//! Resolution (plan.md "Resolution order", highest first): overrides, the language section,
//! the shared settings, the `extends` chain, `.editorconfig`, the provider defaults. Every value
//! keeps the place it came from, for `sudsy config print`.
//!
//! Within one tier the chain's files apply base first, so the file closest to the user wins.
//! Across tiers the order above holds regardless of which file wrote the value: a base file's
//! language section still beats the extending file's shared section.

use crate::chain::ConfigChain;
use crate::editorconfig::EditorConfig;
use crate::error::{ConfigError, unknown_name};
use crate::model::{Level, OptionValue, Options};
use crate::schema::{OptionError, OptionSchema, ResolvedOptions};
use crate::shared;
use serde_saphyr::Spanned;
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Where a resolved value came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    /// The provider's declaration.
    Default,
    EditorConfig {
        file: PathBuf,
        line: usize,
        key: String,
    },
    Preset(String),
    /// A config file; `section` says which part (`csharp.format`, `overrides[0].format`).
    Config {
        file: Option<PathBuf>,
        line: u64,
        column: u64,
        section: String,
    },
}

impl Source {
    /// Like `Display`, with `show` rendering file paths.
    pub fn describe(&self, show: &dyn Fn(&Path) -> String) -> String {
        match self {
            Source::Default => "default".into(),
            Source::EditorConfig { file, line, key } => format!("{}:{line} ({key})", show(file)),
            Source::Preset(name) => format!("preset {name}"),
            Source::Config {
                file,
                line,
                column,
                section,
            } => {
                let name = file.as_deref().map_or("config".into(), show);
                format!("{name}:{line}:{column} ({section})")
            }
        }
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.describe(&|p| p.display().to_string()))
    }
}

/// What resolution needs to know about the file.
#[derive(Clone, Copy, Debug)]
pub struct FileInput<'a> {
    /// The grammar's language name (`csharp`, `json`, `yaml`).
    pub language: &'a str,
    /// Absolute path, for `overrides`. Without one no override applies.
    pub path: Option<&'a Path>,
    pub editorconfig: Option<&'a EditorConfig>,
}

impl<'a> FileInput<'a> {
    pub fn language(language: &'a str) -> Self {
        FileInput {
            language,
            path: None,
            editorconfig: None,
        }
    }

    pub fn with_path(mut self, path: &'a Path) -> Self {
        self.path = Some(path);
        self
    }

    pub fn with_editorconfig(mut self, editorconfig: &'a EditorConfig) -> Self {
        self.editorconfig = Some(editorconfig);
        self
    }
}

/// Format options of one file and where each came from.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub options: ResolvedOptions,
    /// One entry per option the provider declares.
    pub sources: BTreeMap<String, Source>,
}

/// A rule's level and options for one file.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedRule {
    pub level: Level,
    pub options: BTreeMap<String, OptionValue>,
    pub source: Source,
}

fn located(name: &str, at: &Spanned<OptionValue>, e: OptionError) -> ConfigError {
    let _ = name;
    ConfigError::at(at.referenced.line(), at.referenced.column(), e.to_string())
}

/// Applies the entries of `options` to `into`, recording each one.
pub(crate) fn apply_options(
    options: &Options,
    schema: &OptionSchema,
    into: &mut ResolvedOptions,
    mut record: impl FnMut(&str, &Spanned<OptionValue>),
) -> Result<(), ConfigError> {
    for (name, value) in options {
        schema
            .apply(into, name, &value.value)
            .map_err(|e| located(name, value, e))?;
        record(name, value);
    }
    Ok(())
}

/// Applies the shared options (`line_width`, ...) that the language declares; the others are
/// not this language's business.
pub(crate) fn apply_shared(
    options: &Options,
    schema: &OptionSchema,
    into: &mut ResolvedOptions,
    mut record: impl FnMut(&str, &Spanned<OptionValue>),
) -> Result<(), ConfigError> {
    for (name, value) in options {
        if schema.get(name).is_some() && shared::NAMES.contains(&name.as_str()) {
            schema
                .apply(into, name, &value.value)
                .map_err(|e| located(name, value, e))?;
            record(name, value);
        }
    }
    Ok(())
}

/// Only the shared options may appear in a `format:` section outside a language.
pub(crate) fn check_shared(options: &Options) -> Result<(), ConfigError> {
    for (name, value) in options {
        if !shared::NAMES.contains(&name.as_str()) {
            let mut message = unknown_name("shared option", name, shared::NAMES);
            message.push_str(
                "; options of one language go under its section (`csharp:`, `json:`, `yaml:`)",
            );
            return Err(ConfigError::at(
                value.referenced.line(),
                value.referenced.column(),
                message,
            ));
        }
    }
    Ok(())
}

fn language_of(preset: &str) -> &str {
    preset.split('/').next().unwrap_or_default()
}

impl ConfigChain {
    /// Format options for one file: defaults, `.editorconfig`, presets, shared settings, the
    /// language section, matching overrides.
    pub fn format_options(
        &self,
        input: &FileInput<'_>,
        schema: &OptionSchema,
    ) -> Result<Resolved, ConfigError> {
        let mut options = schema.defaults();
        let mut sources: BTreeMap<String, Source> = schema
            .defs()
            .iter()
            .map(|d| (d.name.to_string(), Source::Default))
            .collect();

        if let Some(editorconfig) = input.editorconfig {
            for v in &editorconfig.values {
                let Some(def) = schema.get(v.option) else {
                    continue;
                };
                if def.check(&v.value).is_ok() {
                    options.set(v.option, v.value.clone());
                    sources.insert(
                        v.option.into(),
                        Source::EditorConfig {
                            file: v.file.clone(),
                            line: v.line,
                            key: v.key.clone(),
                        },
                    );
                }
            }
        }

        for preset_ref in &self.presets {
            if language_of(&preset_ref.name) != input.language {
                continue;
            }
            let preset = schema.find_preset(&preset_ref.name).ok_or_else(|| {
                ConfigError::at(
                    preset_ref.line,
                    preset_ref.column,
                    format!("unknown preset `{}`", preset_ref.name),
                )
                .in_file(preset_ref.file.as_ref())
            })?;
            for (option, value) in &preset.values {
                if schema.get(option).is_some() {
                    schema
                        .apply(&mut options, option, value)
                        .map_err(|e| ConfigError::new(format!("preset {}: {e}", preset.name)))?;
                    sources.insert((*option).into(), Source::Preset(preset.name.into()));
                } else {
                    options.set(option, value.clone());
                }
            }
        }

        for loaded in &self.files {
            let source = |at: &Spanned<OptionValue>, section: &str| Source::Config {
                file: loaded.file.clone(),
                line: at.referenced.line(),
                column: at.referenced.column(),
                section: section.into(),
            };
            apply_shared(&loaded.config.format, schema, &mut options, |name, at| {
                sources.insert(name.into(), source(at, "format"));
            })
            .map_err(|e| e.in_file(loaded.file.as_ref()))?;
        }
        for loaded in &self.files {
            let Some(section) = loaded.config.language_section(input.language) else {
                continue;
            };
            let label = format!("{}.format", input.language);
            apply_options(&section.format, schema, &mut options, |name, at| {
                sources.insert(
                    name.into(),
                    Source::Config {
                        file: loaded.file.clone(),
                        line: at.referenced.line(),
                        column: at.referenced.column(),
                        section: label.clone(),
                    },
                );
            })
            .map_err(|e| e.in_file(loaded.file.as_ref()))?;
        }
        for (loaded, index, o) in self.matching_overrides(input) {
            let record = |label: String| {
                let file = loaded.file.clone();
                move |at: &Spanned<OptionValue>| Source::Config {
                    file: file.clone(),
                    line: at.referenced.line(),
                    column: at.referenced.column(),
                    section: label.clone(),
                }
            };
            let shared_source = record(format!("overrides[{index}].format"));
            apply_shared(&o.format, schema, &mut options, |name, at| {
                sources.insert(name.into(), shared_source(at));
            })
            .map_err(|e| e.in_file(loaded.file.as_ref()))?;
            if let Some(section) = o.language_section(input.language) {
                let language_source =
                    record(format!("overrides[{index}].{}.format", input.language));
                apply_options(&section.format, schema, &mut options, |name, at| {
                    sources.insert(name.into(), language_source(at));
                })
                .map_err(|e| e.in_file(loaded.file.as_ref()))?;
            }
        }
        Ok(Resolved { options, sources })
    }

    /// The overrides whose `files` match the file, in application order, with their position in
    /// their file.
    fn matching_overrides<'a>(
        &'a self,
        input: &FileInput<'_>,
    ) -> Vec<(&'a crate::chain::Loaded, usize, &'a crate::model::Override)> {
        let Some(path) = input.path else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for loaded in &self.files {
            let Some(relative) = loaded.relative(path) else {
                continue;
            };
            for (index, o) in loaded.config.overrides.iter().enumerate() {
                if loaded.globs.overrides[index].is_match(&relative) {
                    out.push((loaded, index, o));
                }
            }
        }
        out
    }

    /// Rule levels and options for one file (stored as data; the rule engine checks them): the
    /// shared `lint:` section, the language section, matching overrides.
    pub fn rules(&self, input: &FileInput<'_>) -> BTreeMap<String, ResolvedRule> {
        let mut out: BTreeMap<String, ResolvedRule> = BTreeMap::new();
        let mut put =
            |lint: &crate::model::LintSection, loaded: &crate::chain::Loaded, section: &str| {
                for (id, rule) in &lint.rules {
                    let previous = out.remove(id);
                    let options = if rule.value.options.is_empty() {
                        previous.map(|p| p.options).unwrap_or_default()
                    } else {
                        rule.value.options.clone()
                    };
                    out.insert(
                        id.clone(),
                        ResolvedRule {
                            level: rule.value.level,
                            options,
                            source: Source::Config {
                                file: loaded.file.clone(),
                                line: rule.referenced.line(),
                                column: rule.referenced.column(),
                                section: section.into(),
                            },
                        },
                    );
                }
            };
        for loaded in &self.files {
            put(&loaded.config.lint, loaded, "lint.rules");
        }
        for loaded in &self.files {
            if let Some(section) = loaded.config.language_section(input.language) {
                put(
                    &section.lint,
                    loaded,
                    &format!("{}.lint.rules", input.language),
                );
            }
        }
        for (loaded, index, o) in self.matching_overrides(input) {
            put(&o.lint, loaded, &format!("overrides[{index}].lint.rules"));
            if let Some(section) = o.language_section(input.language) {
                put(
                    &section.lint,
                    loaded,
                    &format!("overrides[{index}].{}.lint.rules", input.language),
                );
            }
        }
        out
    }
}

//! Resolution (plan.md "Resolution order", highest first): overrides, the language section,
//! the shared settings, the `extends` chain, `.editorconfig`, the provider defaults. Every value
//! keeps the place it came from, for `sudsy config print`.
//!
//! Within one tier the chain's files apply base first, so the file closest to the user wins.
//! Across tiers the order above holds regardless of which file wrote the value: a base file's
//! language section still beats the extending file's shared section.

use crate::chain::{ConfigChain, Loaded};
use crate::editorconfig::EditorConfig;
use crate::error::{ConfigError, unknown_name};
use crate::model::{Level, OptionValue, Options, Semantics};
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

        let presets = self.preset_files(input.language, schema)?;
        // Presets are a base layer: all their tiers sit below those of the chain's files.
        let label = format!("{}.format", input.language);
        for files in [
            presets.iter().collect::<Vec<_>>(),
            self.files.iter().collect(),
        ] {
            for loaded in &files {
                apply_shared(&loaded.config.format, schema, &mut options, |name, at| {
                    let (line, column) = (at.referenced.line(), at.referenced.column());
                    sources.insert(name.into(), loaded.source(line, column, "format"));
                })
                .map_err(|e| loaded.attribute(e))?;
            }
            for loaded in &files {
                let Some(section) = loaded.config.language_section(input.language) else {
                    continue;
                };
                apply_options(&section.format, schema, &mut options, |name, at| {
                    let (line, column) = (at.referenced.line(), at.referenced.column());
                    sources.insert(name.into(), loaded.source(line, column, &label));
                })
                .map_err(|e| loaded.attribute(e))?;
            }
            for (loaded, index, o) in Self::matching_overrides(&files, input) {
                let shared_label = format!("overrides[{index}].format");
                apply_shared(&o.format, schema, &mut options, |name, at| {
                    let (line, column) = (at.referenced.line(), at.referenced.column());
                    sources.insert(name.into(), loaded.source(line, column, &shared_label));
                })
                .map_err(|e| loaded.attribute(e))?;
                if let Some(section) = o.language_section(input.language) {
                    let label = format!("overrides[{index}].{}.format", input.language);
                    apply_options(&section.format, schema, &mut options, |name, at| {
                        let (line, column) = (at.referenced.line(), at.referenced.column());
                        sources.insert(name.into(), loaded.source(line, column, &label));
                    })
                    .map_err(|e| loaded.attribute(e))?;
                }
            }
        }
        Ok(Resolved { options, sources })
    }

    /// The overrides whose `files` match the file, in application order, with their position in
    /// their file.
    fn matching_overrides<'a>(
        files: &[&'a Loaded],
        input: &FileInput<'_>,
    ) -> Vec<(&'a Loaded, usize, &'a crate::model::Override)> {
        let Some(path) = input.path else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for &loaded in files {
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

    /// The `semantics:` facts that apply to a file: its language section in every config file and
    /// in matching overrides, added together.
    pub fn semantics(&self, input: &FileInput<'_>) -> Semantics {
        let mut out = Semantics::default();
        let mut add = |s: &Semantics| {
            out.does_not_return
                .extend(s.does_not_return.iter().cloned());
            out.pure.extend(s.pure.iter().cloned());
        };
        let files: Vec<&Loaded> = self.files.iter().collect();
        for loaded in &files {
            if let Some(section) = loaded.config.language_section(input.language) {
                add(&section.semantics);
            }
        }
        for (_, _, o) in Self::matching_overrides(&files, input) {
            if let Some(section) = o.language_section(input.language) {
                add(&section.semantics);
            }
        }
        out
    }

    /// Rule levels and options for one file (stored as data; the rule engine checks them): the
    /// shared `lint:` section, the language section, matching overrides.
    /// language count as configs below the chain's files.
    pub fn rules(
        &self,
        input: &FileInput<'_>,
        schema: &OptionSchema,
    ) -> Result<BTreeMap<String, ResolvedRule>, ConfigError> {
        let presets = self.preset_files(input.language, schema)?;
        let mut out: BTreeMap<String, ResolvedRule> = BTreeMap::new();
        let mut put = |lint: &crate::model::LintSection, loaded: &Loaded, section: &str| {
            for (id, rule) in &lint.rules {
                let previous = out.remove(id);
                let options = if rule.value.options.is_empty() {
                    previous.map(|p| p.options).unwrap_or_default()
                } else {
                    rule.value.options.clone()
                };
                let (line, column) = (rule.referenced.line(), rule.referenced.column());
                out.insert(
                    id.clone(),
                    ResolvedRule {
                        level: rule.value.level,
                        options,
                        source: loaded.source(line, column, section),
                    },
                );
            }
        };
        for files in [
            presets.iter().collect::<Vec<_>>(),
            self.files.iter().collect(),
        ] {
            for loaded in &files {
                put(&loaded.config.lint, loaded, "lint.rules");
            }
            for loaded in &files {
                if let Some(section) = loaded.config.language_section(input.language) {
                    put(
                        &section.lint,
                        loaded,
                        &format!("{}.lint.rules", input.language),
                    );
                }
            }
            for (loaded, index, o) in Self::matching_overrides(&files, input) {
                put(&o.lint, loaded, &format!("overrides[{index}].lint.rules"));
                if let Some(section) = o.language_section(input.language) {
                    put(
                        &section.lint,
                        loaded,
                        &format!("overrides[{index}].{}.lint.rules", input.language),
                    );
                }
            }
        }
        Ok(out)
    }
}

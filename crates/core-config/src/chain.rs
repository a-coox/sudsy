//! The config files that apply: the one found by [`crate::discover`] and everything it extends.

use crate::PRESET_PREFIX;
use crate::error::ConfigError;
use crate::glob::Matcher;
use crate::load::{load_file, parse_str};
use crate::model::{Config, Depth, LANGUAGES};
use crate::schema::OptionSchema;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Longest chain of config files extending each other.
const MAX_DEPTH: usize = 16;

/// A `preset:` entry of an `extends` list, with where it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetRef {
    /// `csharp/google@1`, as written (the version may be missing).
    pub name: String,
    pub file: Option<PathBuf>,
    pub line: u64,
    pub column: u64,
}

/// One config file of the chain.
#[derive(Clone, Debug, PartialEq)]
pub struct Loaded {
    /// `None` for a config given as text.
    pub file: Option<PathBuf>,
    /// Directory of the file: the globs in it are relative to this.
    pub dir: Option<PathBuf>,
    pub config: Config,
    pub(crate) globs: Arc<Globs>,
}

/// The compiled globs of one file. Derived from the config, so they never make two equal configs
/// unequal.
#[derive(Debug)]
pub(crate) struct Globs {
    pub overrides: Vec<Matcher>,
    pub include: Matcher,
    pub exclude: Matcher,
}

impl PartialEq for Globs {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Loaded {
    fn new(file: Option<PathBuf>, config: Config) -> Result<Loaded, ConfigError> {
        let compile = || -> Result<Globs, ConfigError> {
            Ok(Globs {
                overrides: config
                    .overrides
                    .iter()
                    .map(|o| Matcher::new(&o.files.value))
                    .collect::<Result<_, _>>()?,
                include: Matcher::new(&config.include)?,
                exclude: Matcher::new(&config.exclude)?,
            })
        };
        let globs = compile().map_err(|e| e.in_file(file.as_ref()))?;
        let dir = file
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf);
        Ok(Loaded {
            file,
            dir,
            config,
            globs: Arc::new(globs),
        })
    }

    /// `path` as the globs of this file see it: relative to its directory, `/` separated.
    /// `None` for a path outside the directory.
    pub(crate) fn relative(&self, path: &Path) -> Option<String> {
        let relative = match &self.dir {
            Some(dir) => path.strip_prefix(dir).ok()?,
            None => path,
        };
        Some(
            relative
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/"),
        )
    }
}

/// The config files that apply, base files first: for `extends: [preset:A, base.yaml]` the chain
/// is `base.yaml`, then the file itself, and the presets are `A` and whatever `base.yaml` names.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ConfigChain {
    pub files: Vec<Loaded>,
    /// Every `preset:` entry, in the order the chain reads them.
    pub presets: Vec<PresetRef>,
}

impl ConfigChain {
    /// A chain from the text of a config that is not a file. It may extend presets but no files.
    pub fn parse(text: &str) -> Result<ConfigChain, ConfigError> {
        ConfigChain::from_config(parse_str(text)?)
    }

    /// A chain from a parsed config that is not a file.
    pub fn from_config(config: Config) -> Result<ConfigChain, ConfigError> {
        let mut chain = ConfigChain::default();
        for entry in &config.extends {
            match entry.value.strip_prefix(PRESET_PREFIX) {
                Some(name) => chain.presets.push(PresetRef {
                    name: name.to_string(),
                    file: None,
                    line: entry.referenced.line(),
                    column: entry.referenced.column(),
                }),
                None => {
                    return Err(ConfigError::at(
                        entry.referenced.line(),
                        entry.referenced.column(),
                        format!("cannot extend `{}`: the config has no file", entry.value),
                    ));
                }
            }
        }
        chain.files.push(Loaded::new(None, config)?);
        Ok(chain)
    }

    /// Reads `path` and the files it extends (paths relative to the file that names them).
    pub fn load(path: &Path) -> Result<ConfigChain, ConfigError> {
        let mut chain = ConfigChain::default();
        let path = std::path::absolute(path)
            .map_err(|e| ConfigError::new(e.to_string()).with_path(path.to_path_buf()))?;
        chain.read(&path, &mut vec![path.clone()])?;
        Ok(chain)
    }

    fn read(&mut self, path: &Path, stack: &mut Vec<PathBuf>) -> Result<(), ConfigError> {
        let config = load_file(path)?;
        let dir = path.parent().unwrap_or(Path::new(""));
        for entry in &config.extends {
            let (line, column) = (entry.referenced.line(), entry.referenced.column());
            let here = |message: String| {
                ConfigError::at(line, column, message).with_path(path.to_path_buf())
            };
            if let Some(name) = entry.value.strip_prefix(PRESET_PREFIX) {
                self.presets.push(PresetRef {
                    name: name.to_string(),
                    file: Some(path.to_path_buf()),
                    line,
                    column,
                });
                continue;
            }
            let target =
                std::path::absolute(dir.join(&entry.value)).map_err(|e| here(e.to_string()))?;
            if stack.contains(&target) {
                return Err(here(format!(
                    "`{}` extends itself (through {})",
                    entry.value,
                    stack
                        .iter()
                        .filter_map(|p| p.file_name())
                        .map(|n| n.to_string_lossy())
                        .collect::<Vec<_>>()
                        .join(" -> ")
                )));
            }
            if stack.len() >= MAX_DEPTH {
                return Err(here(format!(
                    "more than {MAX_DEPTH} config files extend each other"
                )));
            }
            if !target.is_file() {
                return Err(here(format!("extended config `{}` not found", entry.value)));
            }
            stack.push(target.clone());
            self.read(&target, stack)?;
            stack.pop();
        }
        self.files
            .push(Loaded::new(Some(path.to_path_buf()), config)?);
        Ok(())
    }

    /// The file the chain starts from (the last one: it extends all the others).
    pub fn root(&self) -> Option<&Loaded> {
        self.files.last()
    }

    /// Directory of the root config file.
    pub fn root_dir(&self) -> Option<&Path> {
        self.root().and_then(|l| l.dir.as_deref())
    }

    /// Analysis depth: the last file that sets it, else `standard`.
    pub fn depth(&self) -> Depth {
        self.files
            .iter()
            .rev()
            .find_map(|l| l.config.analysis)
            .unwrap_or_default()
    }

    /// Does an `exclude` glob of any file in the chain match `path` (absolute)? Applies to
    /// directories too: nothing below an excluded directory is visited.
    pub fn is_excluded(&self, path: &Path) -> bool {
        self.files.iter().any(|loaded| {
            loaded
                .relative(path)
                .is_some_and(|relative| loaded.globs.exclude.is_match(&relative))
        })
    }

    /// Is `path` (absolute) a file the `include` lists let through? True when no file in the chain
    /// has an `include`.
    pub fn is_included(&self, path: &Path) -> bool {
        let mut has_include = false;
        for loaded in &self.files {
            if loaded.globs.include.is_empty() {
                continue;
            }
            has_include = true;
            if loaded
                .relative(path)
                .is_some_and(|relative| loaded.globs.include.is_match(&relative))
            {
                return true;
            }
        }
        !has_include
    }

    /// Is the file left out by `include` / `exclude`?
    pub fn excludes(&self, path: &Path) -> bool {
        self.is_excluded(path) || !self.is_included(path)
    }

    /// Plugin files and patterns of every file in the chain: the file they were written in, then
    /// the entry as written (relative to that file's directory).
    pub fn plugin_entries(&self) -> Vec<PluginEntry> {
        let mut out = Vec::new();
        for loaded in &self.files {
            let plugins = &loaded.config.plugins;
            for (kind, list) in [
                (PluginKind::Pattern, &plugins.patterns),
                (PluginKind::Wasm, &plugins.wasm),
            ] {
                for entry in list {
                    out.push(PluginEntry {
                        kind,
                        entry: entry.value.clone(),
                        dir: loaded.dir.clone(),
                    });
                }
            }
        }
        out
    }

    /// Checks everything that needs the providers' option declarations: preset names, the names
    /// and values of format options in every section and override. `schemas` are the languages
    /// the caller knows (`(language, schema)`); sections of other languages are not checked.
    pub fn validate(&self, schemas: &[(&str, &OptionSchema)]) -> Result<(), ConfigError> {
        for preset in &self.presets {
            self.check_preset(preset, schemas)
                .map_err(|e| e.in_file(preset.file.as_ref()))?;
        }
        for loaded in &self.files {
            let check = |e: ConfigError| e.in_file(loaded.file.as_ref());
            crate::resolve::check_shared(&loaded.config.format).map_err(check)?;
            for o in &loaded.config.overrides {
                crate::resolve::check_shared(&o.format).map_err(check)?;
            }
            for (language, schema) in schemas {
                let mut scratch = schema.defaults();
                let sections = loaded.config.language_section(language).into_iter().chain(
                    loaded
                        .config
                        .overrides
                        .iter()
                        .filter_map(|o| o.language_section(language)),
                );
                for section in sections {
                    crate::resolve::apply_options(&section.format, schema, &mut scratch, |_, _| {})
                        .map_err(check)?;
                }
                // Shared options the language declares must hold valid values too.
                crate::resolve::apply_shared(
                    &loaded.config.format,
                    schema,
                    &mut scratch,
                    |_, _| {},
                )
                .map_err(check)?;
                for o in &loaded.config.overrides {
                    crate::resolve::apply_shared(&o.format, schema, &mut scratch, |_, _| {})
                        .map_err(check)?;
                }
            }
        }
        Ok(())
    }

    fn check_preset(
        &self,
        preset: &PresetRef,
        schemas: &[(&str, &OptionSchema)],
    ) -> Result<(), ConfigError> {
        let language = preset.name.split('/').next().unwrap_or_default();
        let at = |message: String| ConfigError::at(preset.line, preset.column, message);
        match schemas.iter().find(|(l, _)| *l == language) {
            Some((_, schema)) => {
                if schema.find_preset(&preset.name).is_none() {
                    return Err(at(format!(
                        "unknown preset `{}`; known presets: {}",
                        preset.name,
                        schema
                            .presets()
                            .iter()
                            .map(|p| p.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
            }
            None => {
                if !LANGUAGES.contains(&language) {
                    return Err(at(crate::error::unknown_name(
                        "preset language",
                        language,
                        LANGUAGES,
                    )));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PluginKind {
    Pattern,
    Wasm,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginEntry {
    pub kind: PluginKind,
    pub entry: String,
    /// Directory of the config file that names it.
    pub dir: Option<PathBuf>,
}

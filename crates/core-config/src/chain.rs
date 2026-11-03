//! The config files that apply: the one found by [`crate::discover`] and everything it extends.

use crate::PRESET_PREFIX;
use crate::error::ConfigError;
use crate::glob::Matcher;
use crate::load::{parse_str, read_source};
use crate::model::{Config, Depth, LANGUAGES, Plugins};
use crate::presets::language_of;
use crate::resolve::Source;
use crate::schema::OptionSchema;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Longest chain of config files extending each other.
const MAX_DEPTH: usize = 16;

/// Most config files one chain reads in total.
const MAX_FILES: usize = 64;

fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// A `preset:` entry of an `extends` list, with where it was written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresetRef {
    /// `csharp/google`, as written.
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
    /// `Some(name)` when this is a preset (`csharp/google`), whose text stands in for a file.
    pub preset: Option<String>,
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
            preset: None,
            globs: Arc::new(globs),
        })
    }

    /// Where a value written at `line`/`column` in `section` of this config came from.
    pub(crate) fn source(&self, line: u64, column: u64, section: &str) -> Source {
        match &self.preset {
            Some(name) => Source::Preset(name.clone()),
            None => Source::Config {
                file: self.file.clone(),
                line,
                column,
                section: section.into(),
            },
        }
    }

    /// `e` as an error of this config: pointing at its file, or at the preset's name.
    pub(crate) fn attribute(&self, e: ConfigError) -> ConfigError {
        match &self.preset {
            Some(name) => ConfigError::new(format!("preset {name}: {e}")),
            None => e.in_file(self.file.as_ref()),
        }
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
        let mut seen = HashSet::from([canonical(&path)]);
        let root = canonical(path.parent().unwrap_or(Path::new("")));
        chain.read(&path, &root, &mut vec![path.clone()], &mut seen)?;
        Ok(chain)
    }

    /// `root` is the directory of the first file: a parse error of a file outside it is reported
    /// without the parser's message, which quotes the file's text. `seen` holds the files already
    /// read, so a file extended along several paths is read once.
    fn read(
        &mut self,
        path: &Path,
        root: &Path,
        stack: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) -> Result<(), ConfigError> {
        let source = read_source(path)?;
        let config = parse_str(&source).map_err(|e| {
            // Compared canonically: `..` is not resolved in a lexical path on every platform.
            if canonical(path).starts_with(root) {
                return e.with_path(path.to_path_buf());
            }
            let (line, column) = (e.line.unwrap_or(0), e.column.unwrap_or(0));
            ConfigError::at(
                line,
                column,
                "invalid config (details withheld: the file is outside the project)",
            )
            .with_path(path.to_path_buf())
        })?;
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
            if !seen.insert(canonical(&target)) {
                continue;
            }
            if seen.len() > MAX_FILES {
                return Err(here(format!(
                    "more than {MAX_FILES} config files in one extends chain"
                )));
            }
            stack.push(target.clone());
            self.read(&target, root, stack, seen)?;
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

    /// The same chain with analysis depth `depth`: the command line overrides the config files.
    pub fn with_depth(mut self, depth: Depth) -> Self {
        match self.files.last_mut() {
            Some(root) => root.config.analysis = Some(depth),
            None => {
                let config = Config {
                    analysis: Some(depth),
                    ..Config::default()
                };
                self.files
                    .push(Loaded::new(None, config).expect("a config without globs compiles"));
            }
        }
        self
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
            check_loaded(loaded, schemas)?;
        }
        Ok(())
    }

    /// The presets of `language` the chain names, as configs of their own (a preset is read
    /// like an `extends` file whose text is embedded), each after the presets it extends. They
    /// sit below every file of the chain; their globs are relative to the root config's directory.
    pub(crate) fn preset_files(
        &self,
        language: &str,
        schema: &OptionSchema,
    ) -> Result<Vec<Loaded>, ConfigError> {
        let mut out = Vec::new();
        for r in self
            .presets
            .iter()
            .filter(|r| language_of(&r.name) == language)
        {
            if schema.find_preset(&r.name).is_none() {
                return Err(ConfigError::at(
                    r.line,
                    r.column,
                    format!("unknown preset `{}`", r.name),
                )
                .in_file(r.file.as_ref()));
            }
            self.read_preset(&r.name, schema, language, &mut Vec::new(), &mut out)
                .map_err(|e| e.in_file(r.file.as_ref()))?;
        }
        Ok(out)
    }

    /// Reads preset `name` and the presets it extends into `out`. `stack` holds the presets being
    /// read, to catch cycles. A preset can extend presets only, not files, and cannot set the
    /// keys that are about the project (`include`, `exclude`, `analysis`, `plugins`).
    fn read_preset(
        &self,
        name: &str,
        schema: &OptionSchema,
        language: &str,
        stack: &mut Vec<String>,
        out: &mut Vec<Loaded>,
    ) -> Result<(), ConfigError> {
        if stack.iter().any(|n| n == name) {
            return Err(ConfigError::new(format!(
                "preset {name} extends itself (through {})",
                stack.join(" -> ")
            )));
        }
        let in_preset = |e: ConfigError| ConfigError::new(format!("preset {name}: {e}"));
        let preset = schema
            .find_preset(name)
            .ok_or_else(|| ConfigError::new(format!("unknown preset `{name}`")))?;
        let config = parse_str(preset.text).map_err(in_preset)?;
        if !config.include.is_empty()
            || !config.exclude.is_empty()
            || config.analysis.is_some()
            || config.plugins != Plugins::default()
        {
            return Err(in_preset(ConfigError::new(
                "a preset cannot set include, exclude, analysis or plugins",
            )));
        }
        stack.push(name.to_string());
        for entry in &config.extends {
            match entry.value.strip_prefix(PRESET_PREFIX) {
                Some(n) if language_of(n) == language => {
                    self.read_preset(n, schema, language, stack, out)?;
                }
                Some(_) => {}
                None => {
                    return Err(in_preset(ConfigError::new(
                        "a preset can only extend presets",
                    )));
                }
            }
        }
        stack.pop();
        let mut loaded = Loaded::new(None, config).map_err(in_preset)?;
        loaded.preset = Some(name.to_string());
        loaded.dir = self.root_dir().map(Path::to_path_buf);
        out.push(loaded);
        Ok(())
    }

    fn check_preset(
        &self,
        preset: &PresetRef,
        schemas: &[(&str, &OptionSchema)],
    ) -> Result<(), ConfigError> {
        let language = language_of(&preset.name);
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
                            .map(|p| p.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                }
                let mut files = Vec::new();
                self.read_preset(&preset.name, schema, language, &mut Vec::new(), &mut files)
                    .map_err(|e| at(e.to_string()))?;
                for loaded in &files {
                    check_loaded(loaded, schemas).map_err(|e| at(e.to_string()))?;
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

/// Checks one config's format options against the declarations of the languages in `schemas`.
fn check_loaded(loaded: &Loaded, schemas: &[(&str, &OptionSchema)]) -> Result<(), ConfigError> {
    let check = |e: ConfigError| loaded.attribute(e);
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
        crate::resolve::apply_shared(&loaded.config.format, schema, &mut scratch, |_, _| {})
            .map_err(check)?;
        for o in &loaded.config.overrides {
            crate::resolve::apply_shared(&o.format, schema, &mut scratch, |_, _| {})
                .map_err(check)?;
        }
    }
    Ok(())
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

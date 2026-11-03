use crate::error::{ConfigError, suggest};
use crate::glob::Matcher;
use crate::model::{Config, Depth, LANGUAGES, LintSection};
use serde_saphyr::Spanned;
use std::path::{Path, PathBuf};

/// Accepted config file names. Having more than one in a directory is an error.
pub const CONFIG_FILE_NAMES: [&str; 2] = ["sudsy.yaml", "sudsy.yml"];

/// Parses and checks one config file's text: structure, unknown keys (with a suggestion for a
/// likely typo), `extends` entries, globs and rule ids. Format option names and values are
/// checked against the providers' declarations later ([`crate::ConfigChain::validate`]).
pub fn parse_str(source: &str) -> Result<Config, ConfigError> {
    if source
        .lines()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
    {
        return Ok(Config::default());
    }
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true; // YAML 1.2: only true/false are booleans
    let config: Config = serde_saphyr::from_str_with_options(source, options).map_err(|e| {
        let message = improve(&e.to_string());
        match e.location() {
            Some(loc) => ConfigError::at(loc.line(), loc.column(), message),
            None => ConfigError::new(message),
        }
    })?;
    validate(&config)?;
    Ok(config)
}

/// The first line of serde-saphyr's message without its position prefix; an unknown field
/// with a close known name gets a suggestion.
fn improve(message: &str) -> String {
    let first = message.lines().next().unwrap_or_default();
    let text = match first.strip_prefix("error: ") {
        Some(rest) => rest.split_once(": ").map_or(rest, |(_, m)| m),
        None => first,
    };
    let Some(rest) = text.strip_prefix("unknown field `") else {
        return text.to_string();
    };
    let Some((name, tail)) = rest.split_once('`') else {
        return text.to_string();
    };
    let expected: Vec<&str> = tail
        .split_once("expected one of ")
        .map_or("", |(_, list)| list)
        .split(", ")
        .collect();
    let guess = suggest(name, expected.iter().copied())
        .map(|s| format!("; did you mean `{s}`?"))
        .unwrap_or_default();
    format!(
        "unknown key `{name}`{guess} (known keys: {})",
        expected.join(", ")
    )
}

fn at<T>(s: &Spanned<T>, message: impl Into<String>) -> ConfigError {
    ConfigError::at(s.referenced.line(), s.referenced.column(), message)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    for entry in &config.extends {
        if entry.value.trim().is_empty() {
            return Err(at(entry, "`extends` entries must not be empty"));
        }
        if let Some(name) = entry.value.strip_prefix(crate::PRESET_PREFIX) {
            check_preset_name(entry, name)?;
        }
    }
    Matcher::new(&config.include)?;
    Matcher::new(&config.exclude)?;
    for p in config.plugins.patterns.iter().chain(&config.plugins.wasm) {
        if p.value.trim().is_empty() {
            return Err(at(p, "plugin paths must not be empty"));
        }
    }
    check_lint(&config.lint)?;
    for section in LANGUAGES.iter().filter_map(|l| config.language_section(l)) {
        check_lint(&section.lint)?;
    }
    for o in &config.overrides {
        if o.files.value.is_empty() {
            return Err(at(&o.files, "`files` must list at least one glob"));
        }
        if let Some(blank) = o.files.value.iter().find(|g| g.value.trim().is_empty()) {
            return Err(at(blank, "`files` globs must not be empty"));
        }
        Matcher::new(&o.files.value)?;
        check_lint(&o.lint)?;
        for section in LANGUAGES.iter().filter_map(|l| o.language_section(l)) {
            check_lint(&section.lint)?;
        }
    }
    Ok(())
}

/// `language/name`; presets are unversioned, so an `@version` is an error.
fn check_preset_name(entry: &Spanned<String>, name: &str) -> Result<(), ConfigError> {
    let valid = matches!(name.split_once('/'), Some((l, n))
        if !l.is_empty() && !n.is_empty() && !n.contains(['/', '@']));
    if valid {
        Ok(())
    } else {
        Err(at(
            entry,
            format!(
                "`{name}` is not a preset name; expected `preset:<language>/<name>` (presets have no versions)"
            ),
        ))
    }
}

/// Rule ids are short names (`no-empty-catch`) or `plugin/rule`: lowercase letters, digits and `-`.
pub fn is_rule_id(id: &str) -> bool {
    let part = |s: &str| {
        s.chars().next().is_some_and(|c| c.is_ascii_lowercase())
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    };
    match id.split_once('/') {
        Some((plugin, rule)) => part(plugin) && part(rule),
        None => part(id),
    }
}

fn check_lint(lint: &LintSection) -> Result<(), ConfigError> {
    for (id, rule) in &lint.rules {
        if !is_rule_id(id) {
            return Err(at(
                rule,
                format!(
                    "`{id}` is not a rule id; ids are lowercase words joined by `-`, \
                     optionally `plugin/rule`"
                ),
            ));
        }
    }
    Ok(())
}

pub fn load_file(path: &Path) -> Result<Config, ConfigError> {
    let source = read_source(path)?;
    parse_str(&source).map_err(|e| e.with_path(path.to_path_buf()))
}

/// The text of a config file, size-capped.
pub(crate) fn read_source(path: &Path) -> Result<String, ConfigError> {
    let bytes = read_capped(path, MAX_CONFIG_BYTES)
        .map_err(|e| ConfigError::new(e).with_path(path.to_path_buf()))?;
    String::from_utf8(bytes)
        .map_err(|_| ConfigError::new("not valid UTF-8").with_path(path.to_path_buf()))
}

/// Largest config or pattern file read (bytes); a repo-supplied file is never read whole beyond it.
pub const MAX_CONFIG_BYTES: u64 = 1 << 20;

/// Reads `path`, refusing before the read when it is larger than `max` bytes.
pub fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    // Read one byte past the cap so a file that grows (or lies about its size) is still caught.
    file.take(max + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > max {
        return Err(format!("larger than the {} KiB limit", max / 1024));
    }
    Ok(bytes)
}

/// Looks for the config file in `dir` and its ancestors. `Ok(None)` when there is none.
pub fn discover(dir: &Path) -> Result<Option<PathBuf>, ConfigError> {
    for d in dir.ancestors() {
        let found: Vec<PathBuf> = CONFIG_FILE_NAMES
            .iter()
            .map(|n| d.join(n))
            .filter(|p| p.is_file())
            .collect();
        match found.as_slice() {
            [] => {}
            [one] => return Ok(Some(one.clone())),
            [first, ..] => {
                return Err(
                    ConfigError::new("both sudsy.yaml and sudsy.yml exist; keep only one")
                        .with_path(first.clone()),
                );
            }
        }
    }
    Ok(None)
}

impl Config {
    /// The analysis depth, `standard` unless set.
    pub fn depth(&self) -> Depth {
        self.analysis.unwrap_or_default()
    }
}

use crate::error::ConfigError;
use crate::model::Config;
use std::path::{Path, PathBuf};

/// Accepted config file names. Having more than one in a directory is an error.
pub const CONFIG_FILE_NAMES: [&str; 2] = ["sudsy.yaml", "sudsy.yml"];

pub fn parse_str(source: &str) -> Result<Config, ConfigError> {
    if source
        .lines()
        .all(|l| l.trim().is_empty() || l.trim_start().starts_with('#'))
    {
        return Ok(Config::default());
    }
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true; // YAML 1.2: only true/false are booleans
    let config: Config =
        serde_saphyr::from_str_with_options(source, options).map_err(|e| match e.location() {
            Some(loc) => ConfigError::at(loc.line(), loc.column(), e.to_string()),
            None => ConfigError {
                path: None,
                line: None,
                column: None,
                message: e.to_string(),
            },
        })?;
    validate(&config)?;
    Ok(config)
}

fn validate(config: &Config) -> Result<(), ConfigError> {
    for entry in &config.extends {
        if entry.value.trim().is_empty() {
            let at = &entry.referenced;
            return Err(ConfigError::at(
                at.line(),
                at.column(),
                "`extends` entries must not be empty",
            ));
        }
    }
    for o in &config.overrides {
        if o.files.value.is_empty() || o.files.value.iter().any(|g| g.trim().is_empty()) {
            let at = &o.files.referenced;
            return Err(ConfigError::at(
                at.line(),
                at.column(),
                "`files` must list at least one non-empty glob pattern",
            ));
        }
    }
    Ok(())
}

pub fn load_file(path: &Path) -> Result<Config, ConfigError> {
    let fail = |message: String| ConfigError {
        path: Some(path.to_path_buf()),
        line: None,
        column: None,
        message,
    };
    let source = std::fs::read_to_string(path).map_err(|e| fail(e.to_string()))?;
    parse_str(&source).map_err(|e| e.with_path(path.to_path_buf()))
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
                return Err(ConfigError {
                    path: Some(first.clone()),
                    line: None,
                    column: None,
                    message: "both sudsy.yaml and sudsy.yml exist; keep only one".into(),
                });
            }
        }
    }
    Ok(None)
}

//! What every command that reads files needs: the providers, the config chain found from the
//! working directory, and how paths are printed (relative to the working directory, with `/`).

use anyhow::{Result, anyhow};
use core_config::{ConfigChain, ConfigError, OptionSchema};
use provider_api::LanguageProvider;
use std::path::{Path, PathBuf};

/// The providers of the languages Sudsy handles. The C# worker is started on the first C# file
/// only; when it cannot be found, the C# files fail with the reason and the rest are processed.
pub fn providers() -> Vec<Box<dyn LanguageProvider>> {
    let worker = lang_csharp::locate_worker()
        .unwrap_or_else(|_| PathBuf::from("csharp-worker/Sudsy.Worker.dll"));
    vec![
        Box::new(lang_json::JsonProvider::new()),
        Box::new(lang_yaml::YamlProvider::new()),
        Box::new(lang_csharp::provider(&worker)),
    ]
}

/// `(language, option schema)` of every provider.
pub fn schemas(providers: &[Box<dyn LanguageProvider>]) -> Vec<(&str, &OptionSchema)> {
    providers
        .iter()
        .map(|p| (p.grammar().language.as_str(), p.option_schema()))
        .collect()
}

/// The ids of every rule of every language, shared ones included.
pub fn rule_ids(providers: &[Box<dyn LanguageProvider>]) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = providers
        .iter()
        .flat_map(|p| p.rules())
        .chain(core_rules::shared::RULES)
        .map(|r| r.meta().id)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

pub struct Project {
    pub cwd: PathBuf,
    pub chain: ConfigChain,
}

impl Project {
    /// The config found from the working directory (the default config when there is none),
    /// checked against the providers' option declarations.
    pub fn load(providers: &[Box<dyn LanguageProvider>]) -> Result<Project> {
        let cwd = core_workspace::absolute(&std::env::current_dir()?)?;
        let fail = |e: ConfigError| anyhow!("{}", describe_error(&e, &cwd));
        let chain = match core_config::discover(&cwd).map_err(fail)? {
            Some(path) => ConfigChain::load(&path).map_err(fail)?,
            None => ConfigChain::default(),
        };
        chain.validate(&schemas(providers)).map_err(fail)?;
        chain
            .validate_rule_ids(&rule_ids(providers))
            .map_err(fail)?;
        Ok(Project { cwd, chain })
    }

    /// A path as printed to the user.
    pub fn show(&self, path: &Path) -> String {
        core_workspace::display_path(path, &self.cwd)
    }
}

/// `sudsy.yaml:3:5: message`, with the path printed like every other path.
pub fn describe_error(e: &ConfigError, cwd: &Path) -> String {
    let mut out = String::new();
    if let Some(path) = &e.path {
        out.push_str(&core_workspace::display_path(path, cwd));
        if let (Some(line), Some(column)) = (e.line, e.column) {
            out.push_str(&format!(":{line}:{column}"));
        }
        out.push_str(": ");
    } else if let (Some(line), Some(column)) = (e.line, e.column) {
        out.push_str(&format!("{line}:{column}: "));
    }
    out.push_str(&e.message);
    out
}

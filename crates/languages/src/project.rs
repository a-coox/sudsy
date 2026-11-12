//! The project a front end works on: its root directory and the config chain found from there.

use core_config::{ConfigChain, ConfigError, Loaded};
use core_rules::Extensions;
use provider_api::LanguageProvider;
use std::path::{Path, PathBuf};

/// A config that cannot be used, described as `sudsy.yaml:3:5: message`.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ProjectError(pub String);

pub struct Project {
    pub root: PathBuf,
    pub chain: ConfigChain,
    /// The pattern and plugin rules the config names.
    pub extensions: Extensions,
    /// The presets the config names, as configs of their own: their `exclude` applies.
    preset_files: Vec<Loaded>,
}

impl Project {
    /// The config's `include` and `exclude` as a file discovery filter.
    pub fn filter(&self) -> core_workspace::Filter {
        let chain = std::sync::Arc::new(self.chain.clone());
        let presets = std::sync::Arc::new(self.preset_files.clone());
        core_workspace::Filter {
            exclude: std::sync::Arc::new({
                let chain = chain.clone();
                move |path| is_excluded(&chain, &presets, path)
            }),
            include: std::sync::Arc::new(move |path| chain.is_included(path)),
        }
    }

    /// Does the config's `exclude` (its own or a preset's) leave `path` out?
    pub fn is_excluded(&self, path: &Path) -> bool {
        is_excluded(&self.chain, &self.preset_files, path)
    }

    /// The config found from `root` (the default config when there is none), checked against the
    /// providers' option declarations and rule ids.
    pub fn load(
        root: &Path,
        providers: &[Box<dyn LanguageProvider>],
    ) -> Result<Project, ProjectError> {
        let root = core_workspace::absolute(root).map_err(|e| ProjectError(e.to_string()))?;
        let fail = |e: ConfigError| ProjectError(describe_error(&e, &root));
        let chain = match core_config::discover(&root).map_err(fail)? {
            Some(path) => ConfigChain::load(&path).map_err(fail)?,
            None => ConfigChain::default(),
        };
        let schemas = crate::schemas(providers);
        chain.validate(&schemas).map_err(fail)?;
        let mut preset_files = Vec::new();
        for (language, schema) in &schemas {
            preset_files.extend(chain.preset_files(language, schema).map_err(fail)?);
        }
        let extensions = crate::load_extensions(&chain, providers, &root).map_err(fail)?;
        chain
            .validate_rule_ids(|| {
                let mut ids = crate::rule_ids(providers);
                ids.extend(extensions.ids());
                ids
            })
            .map_err(fail)?;
        Ok(Project {
            root,
            chain,
            extensions,
            preset_files,
        })
    }

    /// A path as printed to the user.
    pub fn show(&self, path: &Path) -> String {
        core_workspace::display_path(path, &self.root)
    }
}

fn is_excluded(chain: &ConfigChain, presets: &[Loaded], path: &Path) -> bool {
    chain.is_excluded(path) || presets.iter().any(|p| p.excludes(path))
}

/// `sudsy.yaml:3:5: message`, with the path printed relative to `root`.
pub fn describe_error(e: &ConfigError, root: &Path) -> String {
    let mut out = String::new();
    if let Some(path) = &e.path {
        out.push_str(&core_workspace::display_path(path, root));
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

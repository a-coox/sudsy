//! The database: salsa storage plus the (immutable) provider list.

use crate::inputs::{ConfigFile, ExtensionSet, File, WorkspaceRevision};
use core_config::{ConfigChain, ConfigError, EditorConfig};
use provider_api::{LanguageProvider, select_provider_index};
use salsa::Setter;
use std::path::Path;
use std::sync::Arc;

pub type Providers = Arc<[Box<dyn LanguageProvider>]>;

/// What queries need besides inputs. The providers are part of the environment rather than an
/// input: the set is fixed for the life of a database. Pattern and plugin rules, which change
/// with the config, are the [`ExtensionSet`] input.
#[salsa::db]
pub trait Db: salsa::Database {
    fn providers(&self) -> &[Box<dyn LanguageProvider>];
}

/// Callback for salsa's events (`WillExecute`, `DidValidateMemoizedValue`, ...). Tests use it to
/// count query executions.
pub type EventHook = Box<dyn Fn(salsa::Event) + Send + Sync>;

/// One database per run (CLI) or per workspace (LSP). `Clone` makes a handle that shares the
/// memos; each thread works through its own handle. Setting an input needs `&mut` access to a
/// handle and waits until every other handle is dropped, so inputs are set while a single
/// handle exists.
#[salsa::db]
#[derive(Clone)]
pub struct Database {
    storage: salsa::Storage<Self>,
    providers: Providers,
}

#[salsa::db]
impl salsa::Database for Database {}

#[salsa::db]
impl Db for Database {
    fn providers(&self) -> &[Box<dyn LanguageProvider>] {
        &self.providers
    }
}

impl Database {
    pub fn new(providers: Vec<Box<dyn LanguageProvider>>) -> Self {
        Self::build(providers, None)
    }

    pub fn with_event_hook(providers: Vec<Box<dyn LanguageProvider>>, hook: EventHook) -> Self {
        Self::build(providers, Some(hook))
    }

    fn build(providers: Vec<Box<dyn LanguageProvider>>, hook: Option<EventHook>) -> Self {
        let db = Database {
            storage: salsa::Storage::new(hook),
            providers: providers.into(),
        };
        ConfigFile::new(&db, Ok(ConfigChain::default()));
        ExtensionSet::new(&db, core_rules::Extensions::default());
        WorkspaceRevision::new(&db, 0);
        db
    }

    /// Whether some provider handles `path`. Lets callers skip unsupported files without
    /// reading them.
    pub fn supports(&self, path: &Path) -> bool {
        select_provider_index(&self.providers, path).is_some()
    }

    /// Lets the providers start their background processes ahead of the first request.
    pub fn prestart(&self) {
        for provider in self.providers.iter() {
            provider.prestart();
        }
    }

    /// Tells the providers with a semantic model which directories, projects or solutions make up
    /// the project; their files are part of every file's binding.
    pub fn open_workspace(&self, paths: &[std::path::PathBuf]) {
        for provider in self.providers.iter() {
            provider.open_workspace(paths);
        }
    }

    /// [`Database::open_workspace`] for the files a walk found: `found`, the files discovered
    /// under `roots` (so without what the ignore files and `exclude` leave out), those the
    /// provider handles, and the files named in `roots` (a project or solution file).
    pub fn open_project(&self, roots: &[std::path::PathBuf], found: &[std::path::PathBuf]) {
        for provider in self.providers.iter() {
            let matchers = provider.file_matchers();
            let paths: std::collections::BTreeSet<&std::path::PathBuf> = roots
                .iter()
                .filter(|r| r.is_file())
                .chain(
                    found
                        .iter()
                        .filter(|f| matchers.iter().any(|m| m.matches(f))),
                )
                .collect();
            provider.open_workspace(&paths.into_iter().cloned().collect::<Vec<_>>());
        }
    }

    /// An editor edit: the new text of `file` at the editor's document `version`.
    pub fn set_file_text(&mut self, file: File, version: u64, text: String) {
        file.set_text(self).to(text);
        file.set_version(self).to(version);
    }

    /// Some document opened, changed or closed: other files' semantic answers are stale.
    pub fn bump_workspace_revision(&mut self) {
        let input = WorkspaceRevision::get(self);
        let next = input.revision(self) + 1;
        input.set_revision(self).to(next);
    }

    /// The `.editorconfig` values of `file` changed.
    pub fn set_file_editorconfig(&mut self, file: File, editorconfig: EditorConfig) {
        file.set_editorconfig(self).to(editorconfig);
    }

    /// Replaces the pattern and plugin rules (see `core_rules::Extensions`).
    /// An unchanged set (loaders reuse unchanged rule objects) invalidates nothing.
    pub fn set_extensions(&mut self, extensions: core_rules::Extensions) {
        if *ExtensionSet::get(self).extensions(self) == extensions {
            return;
        }
        let _previous = ExtensionSet::get(self).set_extensions(self).to(extensions);
    }

    /// Replaces the config files.
    pub fn set_config(&mut self, chain: Result<ConfigChain, ConfigError>) {
        let _previous = ConfigFile::get(self).set_chain(self).to(chain);
    }
}

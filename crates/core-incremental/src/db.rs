//! The database: salsa storage plus the (immutable) provider list.

use crate::inputs::ConfigFile;
use provider_api::{LanguageProvider, select_provider_index};
use salsa::Setter;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub type Providers = Arc<[Box<dyn LanguageProvider>]>;

/// What queries need besides inputs. The providers are part of the environment rather than an
/// input: the set is fixed for the life of a database (the Wasm plugin host of M6 will make it
/// dynamic, and then it becomes an input).
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
/// handle exists (see `docs/decisions.md`, "Incremental engine").
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
        ConfigFile::new(&db, None, None);
        db
    }

    /// Whether some provider handles `path`. Lets callers skip unsupported files without
    /// reading them.
    pub fn supports(&self, path: &Path) -> bool {
        select_provider_index(&self.providers, path).is_some()
    }

    /// Replaces the config contents.
    pub fn set_config(&mut self, path: Option<PathBuf>, text: Option<String>) {
        let config = ConfigFile::get(self);
        config.set_path(self).to(path);
        config.set_text(self).to(text);
    }
}

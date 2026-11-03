//! The state behind the server: the incremental database with the project's config and the open
//! documents. Knows nothing about the transport, so the handlers in the sibling modules and the
//! tests use it directly.

use crate::position::apply_change;
use core_config::{ConfigError, EditorConfig};
use core_incremental::{Database, File};
use languages::Project;
use lsp_types::TextDocumentContentChangeEvent;
use provider_api::{LanguageProvider, SourceFile};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

struct Doc {
    file: File,
}

pub struct Workspace {
    pub(crate) db: Database,
    root: PathBuf,
    /// The project of the last successful config load.
    project: Option<Project>,
    docs: HashMap<PathBuf, Doc>,
}

impl Workspace {
    /// A workspace rooted at `root`. The second value is why the config could not be loaded, if
    /// it could not: every file then fails with that reason until the config is fixed.
    pub fn new(
        root: PathBuf,
        providers: Vec<Box<dyn LanguageProvider>>,
    ) -> (Workspace, Option<String>) {
        let mut workspace = Workspace {
            db: Database::new(providers),
            root,
            project: None,
            docs: HashMap::new(),
        };
        let error = workspace.reload_config();
        (workspace, error)
    }

    /// Reads the config and the `.editorconfig` values again, for every open document. Returns
    /// the reason when the config is invalid.
    pub fn reload_config(&mut self) -> Option<String> {
        let loaded = Project::load(&self.root, core_incremental::Db::providers(&self.db));
        let error = match loaded {
            Ok(project) => {
                self.db.set_config(Ok(project.chain.clone()));
                self.project = Some(project);
                None
            }
            Err(e) => {
                self.project = None;
                self.db.set_config(Err(ConfigError::new(e.0.clone())));
                Some(e.0)
            }
        };
        for (path, doc) in &self.docs {
            self.db
                .set_file_editorconfig(doc.file, EditorConfig::for_file(path));
        }
        error
    }

    /// Does a change to this file affect the config: a config file name, `.editorconfig`, or a
    /// file the current config extends.
    pub fn is_config_file(&self, path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        core_config::CONFIG_FILE_NAMES.contains(&name)
            || name == ".editorconfig"
            || self.project.as_ref().is_some_and(|p| {
                p.chain
                    .files
                    .iter()
                    .any(|f| f.file.as_deref() == Some(path))
            })
    }

    /// Starts tracking a document; false when no provider handles the file.
    pub fn open(&mut self, path: PathBuf, version: i32, text: String) -> bool {
        if !self.db.supports(&path) {
            return false;
        }
        let Ok(source) = SourceFile::from_bytes(path.to_string_lossy(), 1, text.into_bytes())
        else {
            return false;
        };
        let file = File::new(
            &self.db,
            path.clone(),
            version as u64,
            source.text,
            source.had_bom,
            EditorConfig::for_file(&path),
        );
        self.docs.insert(path, Doc { file });
        true
    }

    /// Applies the changes of one `didChange`, in order.
    pub fn change(
        &mut self,
        path: &Path,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
    ) {
        let Some(doc) = self.docs.get(path) else {
            return;
        };
        let mut text = doc.file.text(&self.db).clone();
        for change in changes {
            apply_change(&mut text, change.range, &change.text);
        }
        let file = doc.file;
        self.db.set_file_text(file, version as u64, text);
    }

    pub fn close(&mut self, path: &Path) {
        self.docs.remove(path);
    }

    /// The input of an open document.
    pub(crate) fn file(&self, path: &Path) -> Option<File> {
        self.docs.get(path).map(|d| d.file)
    }

    pub fn is_open(&self, path: &Path) -> bool {
        self.docs.contains_key(path)
    }

    pub fn text(&self, path: &Path) -> Option<String> {
        self.file(path).map(|f| f.text(&self.db).clone())
    }
}

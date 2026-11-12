//! The state behind the server: the incremental database with the project's config and the open
//! documents. Knows nothing about the transport, so the handlers in the sibling modules and the
//! tests use it directly.

use crate::position::apply_change;
use core_config::{ConfigError, EditorConfig, Need};
use core_incremental::{Database, Db, File};
use languages::{Project, installed::Checker};
use lsp_types::TextDocumentContentChangeEvent;
use provider_api::LanguageProvider;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
struct Doc {
    file: File,
    /// Open in the editor but its language is not installed: kept (and kept in sync) so that
    /// installing the language can start serving it, invisible to every handler until then.
    skipped: bool,
}

/// `Clone` makes a snapshot for a job on the analysis thread: it shares the database's memos and
/// the project; the documents are the inputs as they were.
#[derive(Clone)]
pub struct Workspace {
    pub(crate) db: Database,
    root: PathBuf,
    /// The project of the last successful config load.
    project: Option<Arc<Project>>,
    docs: HashMap<PathBuf, Doc>,
    /// The project's other files, read from disk when the first `deep` diagnostics are asked for
    /// (`None` before that). They give the project-scope rules every file; an edit in the editor
    /// takes a file out of here and into `docs`.
    background: Option<Arc<HashMap<PathBuf, File>>>,
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
            background: None,
        };
        workspace.db.prestart();
        let error = workspace.reload_config();
        (workspace, error)
    }

    /// Tells the semantic providers which files are the project: those the walk visits, so not
    /// what `.gitignore` or `exclude` leave out.
    fn open_semantic_workspace(&self, project: &Project) {
        let roots = [self.root.clone()];
        let found = core_workspace::discover_files(&roots, &project.filter()).unwrap_or_default();
        self.db.open_workspace(&roots, &found);
    }

    /// Reads the config and the `.editorconfig` values again, for every open document. Returns
    /// the reason when the config is invalid.
    pub fn reload_config(&mut self) -> Option<String> {
        let loaded = Project::load(&self.root, core_incremental::Db::providers(&self.db));
        let error = match loaded {
            Ok(project) => {
                self.open_semantic_workspace(&project);
                self.db.set_config(Ok(project.chain.clone()));
                self.db.set_extensions(project.extensions.clone());
                self.project = Some(Arc::new(project));
                None
            }
            Err(e) => {
                let roots = [self.root.clone()];
                let all = core_workspace::Filter::none();
                let found = core_workspace::discover_files(&roots, &all).unwrap_or_default();
                self.db.open_workspace(&roots, &found);
                self.project = None;
                self.db.set_config(Err(ConfigError::new(e.0.clone())));
                self.db.set_extensions(Default::default());
                Some(e.0)
            }
        };
        self.background = None;
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
                    || p.extensions.files.iter().any(|f| f == path)
            })
    }

    /// `(language, why)` when `path` is a known language that is not installed.
    pub fn missing_language(&self, path: &Path) -> Option<(String, String)> {
        let (language, why) = Checker::new(self.db.providers()).missing(path)?;
        Some((language.to_string(), why))
    }

    /// Whether the project's config says files of `language` are skipped without a word.
    pub fn ignores_language(&self, language: &str) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.chain.needs().get(language) == Some(&Need::Ignored))
    }

    /// Serves the documents that were waiting for a language that is installed now.
    pub fn activate_installed(&mut self) {
        let waiting: Vec<PathBuf> = (self.docs.iter())
            .filter(|(_, d)| d.skipped)
            .map(|(p, _)| p.clone())
            .collect();
        let mut checker = Checker::new(self.db.providers());
        let ready: Vec<PathBuf> = (waiting.into_iter())
            .filter(|p| checker.missing(p).is_none())
            .collect();
        for path in ready {
            if let Some(doc) = self.docs.get_mut(&path) {
                doc.skipped = false;
            }
        }
    }

    /// Starts tracking a document; false when no provider handles the file or its language is
    /// not installed (the document is then only kept for [`Workspace::activate_installed`]).
    pub fn open(&mut self, path: PathBuf, version: i32, text: String) -> bool {
        // A file the config excludes (a package cache, generated code) gets no diagnostics and no
        // formatting, though the editor opened it.
        if !self.db.supports(&path) || self.project.as_ref().is_some_and(|p| p.is_excluded(&path)) {
            return false;
        }
        let skipped = self.missing_language(&path).is_some();
        let label = path.to_string_lossy().into_owned();
        let Ok(file) = File::from_bytes(&self.db, path.clone(), label, version as u64, text.into())
        else {
            return false;
        };
        if let Some(background) = &mut self.background {
            Arc::make_mut(background).remove(&path);
        }
        self.docs.insert(path, Doc { file, skipped });
        !skipped
    }

    /// Applies the changes of one `didChange`, in order. `shared`: other threads hold handles of
    /// the database. Setting an input would then wait for them (a semantic request can take
    /// seconds), so the document gets a new input instead; the old one stays for them.
    pub fn change(
        &mut self,
        path: &Path,
        version: i32,
        changes: &[TextDocumentContentChangeEvent],
        shared: bool,
    ) {
        let Some(doc) = self.docs.get(path) else {
            return;
        };
        let mut text = doc.file.text(&self.db).clone();
        for change in changes {
            apply_change(&mut text, change.range, &change.text);
        }
        let (file, skipped) = (doc.file, doc.skipped);
        if shared {
            let new = File::new(
                &self.db,
                path.to_path_buf(),
                version as u64,
                text,
                file.had_bom(&self.db),
                file.editorconfig(&self.db).clone(),
            );
            self.docs
                .insert(path.to_path_buf(), Doc { file: new, skipped });
        } else {
            self.db.set_file_text(file, version as u64, text);
        }
    }

    pub fn close(&mut self, path: &Path) {
        if self.docs.remove(path).is_some_and(|d| !d.skipped)
            && let Some(file) = self.background.is_some().then(|| self.read(path)).flatten()
            && let Some(background) = &mut self.background
        {
            Arc::make_mut(background).insert(path.to_path_buf(), file);
        }
    }

    /// Makes the files' semantic answers depend on the edits so far. Setting the input cancels the
    /// queries of other handles: call it while none runs.
    pub fn bump_revision(&mut self) {
        self.db.bump_workspace_revision();
    }

    /// Does the config ask for `analysis: deep`?
    pub fn is_deep(&self) -> bool {
        self.project
            .as_ref()
            .is_some_and(|p| p.chain.depth() == core_config::Depth::Deep)
    }

    /// Reads the project's files that are not open, once, for the project-scope rules.
    pub fn load_project(&mut self) {
        if self.background.is_some() {
            return;
        }
        let Some(project) = &self.project else { return };
        let paths =
            core_workspace::discover_files(std::slice::from_ref(&self.root), &project.filter())
                .unwrap_or_default();
        let mut background = HashMap::new();
        let mut checker = Checker::new(self.db.providers());
        for path in paths {
            if self.db.supports(&path)
                && checker.missing(&path).is_none()
                && !self.docs.contains_key(&path)
                && let Some(file) = self.read(&path)
            {
                background.insert(path, file);
            }
        }
        self.background = Some(Arc::new(background));
    }

    fn read(&self, path: &Path) -> Option<File> {
        let bytes = std::fs::read(path).ok()?;
        File::from_bytes(
            &self.db,
            path.to_path_buf(),
            path.to_string_lossy(),
            1,
            bytes,
        )
        .ok()
    }

    /// Every file the project-scope rules see: the open documents and the loaded rest.
    pub(crate) fn project_files(&self) -> Vec<File> {
        self.docs
            .values()
            .filter(|d| !d.skipped)
            .map(|d| d.file)
            .chain(self.background.iter().flat_map(|b| b.values().copied()))
            .collect()
    }

    /// The input of an open document.
    pub(crate) fn file(&self, path: &Path) -> Option<File> {
        self.docs.get(path).filter(|d| !d.skipped).map(|d| d.file)
    }

    pub fn text(&self, path: &Path) -> Option<String> {
        self.file(path).map(|f| f.text(&self.db).clone())
    }
}

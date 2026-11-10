//! The inputs of the database: the only values that are set from outside. Everything else is a
//! query over them.

use core_config::{ConfigChain, ConfigError, EditorConfig};
use std::path::PathBuf;

/// One file's text. An editor edit is `file.set_text(&mut db).to(..)` (plus a new `version`);
/// the CLI creates a file once per run.
#[salsa::input(debug)]
pub struct File {
    #[returns(ref)]
    pub path: PathBuf,
    /// The editor's document version, or 1. The worker keys its per-file state on it, so the
    /// parse query depends on it. Setting a file to the same text and a new version re-parses,
    /// which is also how a failed parse is retried.
    #[returns(copy)]
    pub version: u64,
    /// Without a leading byte-order mark.
    #[returns(ref)]
    pub text: String,
    #[returns(copy)]
    pub had_bom: bool,
    /// The `.editorconfig` values that apply to the file; the caller reads them (`EditorConfig::for_file`).
    #[returns(ref)]
    pub editorconfig: EditorConfig,
}

impl File {
    /// A file from its bytes as read from disk or sent by an editor: UTF-8 without the BOM, and
    /// the `.editorconfig` values that apply to `path`. `label` names the file in the error.
    pub fn from_bytes(
        db: &dyn crate::Db,
        path: PathBuf,
        label: impl Into<String>,
        version: u64,
        bytes: Vec<u8>,
    ) -> Result<File, provider_api::SourceError> {
        let source = provider_api::SourceFile::from_bytes(label, version, bytes)?;
        let editorconfig = EditorConfig::for_file(&path);
        Ok(File::new(
            db,
            path,
            version,
            source.text,
            source.had_bom,
            editorconfig,
        ))
    }
}

/// The config files that apply: the `sudsy.yaml` found by the caller and the files it extends, or
/// the error from reading them. An empty chain is the default config.
#[salsa::input(singleton, debug)]
pub struct ConfigFile {
    #[returns(ref)]
    pub chain: Result<ConfigChain, ConfigError>,
}

/// Bumped by the editor whenever any document opens, changes or closes. Semantic answers read it,
/// since another file's edit can change them; format and syntax-only lint do not.
#[salsa::input(singleton, debug)]
pub struct WorkspaceRevision {
    #[returns(copy)]
    pub revision: u64,
}

/// The rules that pattern files and plugins add to the providers' own (empty by default).
#[salsa::input(singleton, debug)]
pub struct ExtensionSet {
    #[returns(ref)]
    pub extensions: core_rules::Extensions,
}

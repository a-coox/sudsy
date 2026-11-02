//! The inputs of the database: the only values that are set from outside. Everything else is a
//! query over them.

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
}

/// The contents of the `sudsy.yaml` that applies, or `None` text when there is no such file.
/// Which config file applies (the walk up from the working directory) is the caller's business;
/// the engine only sees its contents.
#[salsa::input(singleton, debug)]
pub struct ConfigFile {
    #[returns(ref)]
    pub path: Option<PathBuf>,
    #[returns(ref)]
    pub text: Option<String>,
}

//! The derived queries. Dependencies (an arrow reads "reads"):
//!
//! ```text
//! format ──> parse ──> parse_settings ──> options ──> config ──> ConfigFile
//!    │          │                              │
//!    │          └──────────────────────────> selection ──> File.path
//!    └──> options, File.text/version/had_bom
//! ```
//!
//! Rule results are two more queries over `parse` (see [`crate::rules`]): `lint_settings(file)`
//! reads the config and the selection, and `lint(file)` reads `parse` and `lint_settings`. Semantic
//! requests will read the worker session as a versioned input, and `flow_graph(file, body)` those
//! answers (M5); neither exists yet.

use crate::db::Db;
use crate::failure::{Failure, Stage};
use crate::inputs::{ConfigFile, File};
use core_config::{ConfigChain, FileInput, ResolvedOptions};
use core_format::DirectiveTable;
use core_syntax::{GreenNode, SyntaxNode};
use provider_api::{
    LanguageProvider, ParseError, ParseResult, SourceFile, format_parsed, resolve_dialect,
    select_provider_index,
};

/// Which provider handles a file and the dialect its matcher selects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// Position in [`Db::providers`].
    pub provider: usize,
    pub matcher_dialect: Option<String>,
}

/// Format options of one file, with the dialect fixed (see [`resolve_dialect`]).
#[derive(Clone, Debug, PartialEq)]
pub struct FileOptions {
    pub resolved: ResolvedOptions,
    pub language_version: Option<String>,
}

/// The part of the options that parsing reads: a separate value so that changing, say,
/// `indent_size` leaves the parse valid.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseSettings {
    pub language_version: Option<String>,
    /// Conditional compilation symbols (C#), from the `preprocessor_symbols` option.
    pub preprocessor_symbols: Vec<String>,
}

/// A parse as a query value. The tree is a green node: immutable, reference counted (cloning is
/// a pointer copy) and `Send + Sync`, unlike the red `SyntaxNode` the formatter wants, which is
/// rebuilt from it in O(1) wherever it is needed. Equality is structural and, because the tree
/// is lossless, equal trees mean equal text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    pub green: GreenNode,
    pub errors: Vec<ParseError>,
}

impl Parsed {
    pub fn root(&self) -> SyntaxNode {
        SyntaxNode::new_root(self.green.clone())
    }
}

fn unsupported() -> Failure {
    Failure::message(Stage::Unsupported, "no provider handles this file")
}

/// The config files, checked against the providers' option declarations (preset names, option
/// names and values in every section). Errors carry the path and position of the offending value.
#[salsa::tracked(returns(ref))]
pub fn config(db: &dyn Db) -> Result<ConfigChain, Failure> {
    let chain = ConfigFile::get(db)
        .chain(db)
        .as_ref()
        .map_err(|e| Failure::new(Stage::Config, e))?;
    let schemas: Vec<(&str, &core_config::OptionSchema)> = db
        .providers()
        .iter()
        .map(|p| (p.grammar().language.as_str(), p.option_schema()))
        .collect();
    chain
        .validate(&schemas)
        .map_err(|e| Failure::new(Stage::Config, &e))?;
    Ok(chain.clone())
}

/// Language and dialect selection: depends only on the file's path.
#[salsa::tracked(returns(ref))]
pub fn selection(db: &dyn Db, file: File) -> Option<Selection> {
    select_provider_index(db.providers(), file.path(db)).map(|(provider, matcher_dialect)| {
        Selection {
            provider,
            matcher_dialect,
        }
    })
}

/// Resolved format options of a file. Depends on the config and the selection, so a config edit
/// re-resolves it, but a file whose options come out equal does not disturb its dependents.
#[salsa::tracked(returns(ref))]
pub fn options(db: &dyn Db, file: File) -> Result<FileOptions, Failure> {
    let Some(sel) = selection(db, file) else {
        return Err(unsupported());
    };
    let config = config(db).as_ref().map_err(Clone::clone)?;
    let provider = &db.providers()[sel.provider];
    let input = FileInput::language(&provider.grammar().language)
        .with_path(file.path(db))
        .with_editorconfig(file.editorconfig(db));
    let mut resolved = config
        .format_options(&input, provider.option_schema())
        .map_err(|e| Failure::new(Stage::Options, &e))?
        .options;
    let language_version = resolve_dialect(&mut resolved, sel.matcher_dialect.clone());
    Ok(FileOptions {
        resolved,
        language_version,
    })
}

/// The only part of the options that parsing reads.
#[salsa::tracked(returns(clone))]
pub fn parse_settings(db: &dyn Db, file: File) -> Option<ParseSettings> {
    options(db, file).as_ref().ok().map(|o| ParseSettings {
        language_version: o.language_version.clone(),
        preprocessor_symbols: o.resolved.strings("preprocessor_symbols"),
    })
}

/// The file as the provider sees it.
fn source_file(db: &dyn Db, file: File, settings: Option<ParseSettings>) -> SourceFile {
    let (language_version, preprocessor_symbols) = settings
        .map(|s| (s.language_version, s.preprocessor_symbols))
        .unwrap_or_default();
    SourceFile {
        id: file.path(db).to_string_lossy().into_owned(),
        version: file.version(db),
        text: file.text(db).clone(),
        had_bom: file.had_bom(db),
        language_version,
        preprocessor_symbols,
    }
}

pub(crate) fn provider_of(db: &dyn Db, file: File) -> Result<&dyn LanguageProvider, Failure> {
    let sel = selection(db, file).as_ref().ok_or_else(unsupported)?;
    Ok(db.providers()[sel.provider].as_ref())
}

/// Parse tree and parse errors.
///
/// Not a pure function for C#: the provider talks to a worker process. It is still a sound
/// query because its answer is determined by the inputs it reads (path, version, text, dialect)
/// as long as the worker is deterministic, which the protocol requires. A worker that is
/// missing, crashed or refusing is an `Err(Failure)` value like any other result; it is
/// memoized, so one run does not retry a worker that is gone. Setting the file again (same text,
/// new version) retries.
#[salsa::tracked(returns(ref))]
pub fn parse(db: &dyn Db, file: File) -> Result<Parsed, Failure> {
    let provider = provider_of(db, file)?;
    let source = source_file(db, file, parse_settings(db, file));
    match provider.parse(&source) {
        Ok(result) => Ok(Parsed {
            green: result.tree.green().into_owned(),
            errors: result.errors,
        }),
        Err(e) => Err(Failure::new(Stage::Parse, &e)),
    }
}

/// The formatted text, or why the file is left alone: a worker failure, syntax errors, a
/// formatter error, or a failed safety check (lossless, meaning-preserving; see
/// `core_format::safety`). The safety check's re-parse of the output goes straight to the
/// provider: the output is not a file, so there is nothing to key a query on.
#[salsa::tracked(returns(ref))]
pub fn format(db: &dyn Db, file: File) -> Result<String, Failure> {
    let provider = provider_of(db, file)?;
    let parsed = parse(db, file).as_ref().map_err(Clone::clone)?;
    let options = options(db, file).as_ref().map_err(Clone::clone)?;
    let source = source_file(db, file, parse_settings(db, file));
    let result = ParseResult {
        tree: parsed.root(),
        errors: parsed.errors.clone(),
    };
    format_parsed(
        provider,
        &source,
        &result,
        &options.resolved,
        &DirectiveTable::default(),
    )
    .map_err(|e| Failure::new(Stage::Format, &e))
}

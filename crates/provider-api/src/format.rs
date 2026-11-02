//! Formatting a file through a provider: parse, format, safety check.

use crate::provider::{LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile};
use core_config::ResolvedOptions;
use core_format::{DirectiveTable, FormatCtx, FormatError, format_tree, safety};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum FileFormatError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    /// Files with syntax errors are not formatted.
    #[error("{line}:{column}: {message}{}", more(*.count))]
    Syntax {
        line: usize,
        column: usize,
        message: String,
        count: usize,
    },
    #[error(transparent)]
    Format(#[from] FormatError),
}

fn more(count: usize) -> String {
    match count {
        0 | 1 => String::new(),
        n => format!(" (and {} more)", n - 1),
    }
}

/// 1-based line and column (in characters) of a byte offset.
fn position(text: &str, offset: u32) -> (usize, usize) {
    let end = (offset as usize).min(text.len());
    let before = &text[..text.floor_char_boundary(end)];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
    (line, column)
}

fn syntax_error(text: &str, errors: &[ParseError]) -> FileFormatError {
    let first = &errors[0];
    let (line, column) = position(text, first.start);
    FileFormatError::Syntax {
        line,
        column,
        message: first.message.clone(),
        count: errors.len(),
    }
}

/// Formats `file` with the provider and checks the result before returning it. A file with
/// syntax errors, or whose output fails the safety check, is an error and is left alone.
pub fn format_file(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
) -> Result<String, FileFormatError> {
    let parsed = provider.parse(file)?;
    format_parsed(provider, file, &parsed, options, directives)
}

/// [`format_file`] for a tree that is already parsed (the incremental engine memoizes parsing).
/// `parsed` must be the result of parsing `file`.
pub fn format_parsed(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
) -> Result<String, FileFormatError> {
    if !parsed.errors.is_empty() {
        return Err(syntax_error(&file.text, &parsed.errors));
    }
    let grammar = provider.grammar();
    let cx = FormatCtx::new(grammar, options, directives, provider.formatter());
    let output = format_tree(&parsed.tree, &cx)?;
    let reparse = |text: &str| {
        let again = SourceFile {
            text: text.to_string(),
            version: file.version + 1,
            ..file.clone()
        };
        let result = provider.parse(&again).map_err(|e| e.to_string())?;
        match result.errors.first() {
            Some(e) => Err(format!("{}: {}", position_str(text, e), e.message)),
            None => Ok(result.tree),
        }
    };
    safety::verify(
        grammar,
        provider.formatter(),
        &parsed.tree,
        &file.text,
        &output,
        reparse,
    )?;
    Ok(output)
}

fn position_str(text: &str, e: &ParseError) -> String {
    let (line, column) = position(text, e.start);
    format!("{line}:{column}")
}

/// The provider responsible for `path` and the dialect its matcher selects. The most specific
/// matcher wins (file name, then directory, then extension).
pub fn select_provider<'a>(
    providers: &'a [Box<dyn LanguageProvider>],
    path: &Path,
) -> Option<(&'a dyn LanguageProvider, Option<String>)> {
    select_provider_index(providers, path)
        .map(|(index, dialect)| (providers[index].as_ref(), dialect))
}

/// [`select_provider`], naming the provider by its position in `providers`.
pub fn select_provider_index(
    providers: &[Box<dyn LanguageProvider>],
    path: &Path,
) -> Option<(usize, Option<String>)> {
    providers
        .iter()
        .enumerate()
        .flat_map(|(i, p)| p.file_matchers().iter().map(move |m| (i, m)))
        .filter(|(_, m)| m.matches(path))
        .max_by_key(|(_, m)| m.specificity())
        .map(|(i, m)| (i, m.dialect.clone()))
}

/// Fixes the dialect of a file for parsing and formatting. A provider whose schema has a
/// `dialect` option gets the option's value, or the matcher's dialect when it is `auto`; the
/// effective dialect is written back to `options` so formatting code can read it. Returns the
/// value for [`SourceFile::language_version`].
pub fn resolve_dialect(
    options: &mut ResolvedOptions,
    matcher_dialect: Option<String>,
) -> Option<String> {
    let chosen = match options.get("dialect")? {
        core_config::OptionValue::String(s) if s != "auto" => Some(s.clone()),
        _ => matcher_dialect,
    }?;
    options.set("dialect", core_config::OptionValue::String(chosen.clone()));
    Some(chosen)
}

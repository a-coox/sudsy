//! Formatting a file through a provider: parse, format (and, in debug builds, the safety check).

use crate::provider::{LanguageProvider, ParseError, ParseResult, ProviderError, SourceFile};
use core_config::ResolvedOptions;
use core_format::{
    DirectiveTable, Embedder, FormatCtx, FormatError, Variants, format_tree, safety,
};
use core_syntax::SyntaxNode;
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
    core_rules::LineIndex::new(text).line_col(text, offset.into())
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

/// Formats `file` with the provider. A file with syntax errors is an error and is left alone;
/// debug builds (every test) also run the safety check on the output.
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
    format_parsed_with(provider, file, parsed, options, directives, None)
}

/// [`format_parsed`] for a host whose formatter embeds code of other languages through
/// `embedder`.
pub fn format_parsed_with(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
    embedder: Option<&dyn Embedder>,
) -> Result<String, FileFormatError> {
    if !parsed.errors.is_empty() {
        return Err(syntax_error(&file.text, &parsed.errors));
    }
    let grammar = provider.grammar();
    // A rewrite can unlock the next (`((T))` loses one pair of parentheses per round): repeat
    // until the text is stable, so that formatting the output changes nothing more.
    let mut current: Option<(SourceFile, ParseResult)> = None;
    for _ in 0..4 {
        let (text, tree) = match &current {
            Some((f, p)) => (&f.text, &p.tree),
            None => (&file.text, &parsed.tree),
        };
        let Some(next) = provider
            .formatter()
            .rewrite_source(tree, text, grammar, options)
            .filter(|t| t != text)
        else {
            break;
        };
        let prev_version = current.as_ref().map_or(file.version, |(f, _)| f.version);
        let again = SourceFile {
            id: file.id.clone(),
            version: prev_version + 1,
            text: next,
            had_bom: file.had_bom,
            language_version: file.language_version.clone(),
            preprocessor_symbols: file.preprocessor_symbols.clone(),
        };
        let reparsed = provider.parse(&again)?;
        if let Some(e) = reparsed.errors.first() {
            return Err(FormatError::Safety(format!(
                "the rewritten source does not parse: {}",
                e.message
            ))
            .into());
        }
        current = Some((again, reparsed));
    }
    if let Some((again, reparsed)) = &current {
        return format_checked(provider, again, reparsed, options, directives, embedder);
    }
    format_checked(provider, file, parsed, options, directives, embedder)
}

/// Formats a tree that has no syntax errors and checks the output against it. Code that the
/// parse leaves out (inactive `#if` branches) is formatted from other parses when the provider
/// asks for that (see [`core_format::NodeFormatter::merge_variants`]); every parse it used checks the result.
fn format_checked(
    provider: &dyn LanguageProvider,
    file: &SourceFile,
    parsed: &ParseResult,
    options: &ResolvedOptions,
    directives: &DirectiveTable,
    embedder: Option<&dyn Embedder>,
) -> Result<String, FileFormatError> {
    let grammar = provider.grammar();
    let cx =
        FormatCtx::new(grammar, options, directives, provider.formatter()).with_embedder(embedder);
    let output = format_tree(&parsed.tree, &cx)?;
    let check = |symbols: &[String], tree: &SyntaxNode, text: &str| {
        let reparse = |text: &str| {
            // Not `..file.clone()`: that would copy the whole source text just to replace it.
            let again = SourceFile {
                id: file.id.clone(),
                version: file.version + 1,
                text: text.to_string(),
                had_bom: file.had_bom,
                language_version: file.language_version.clone(),
                preprocessor_symbols: symbols.to_vec(),
            };
            let result = provider.parse(&again).map_err(|e| e.to_string())?;
            match result.errors.first() {
                Some(e) => Err(format!("{}: {}", position_str(text, e), e.message)),
                None => Ok(result.tree),
            }
        };
        safety::verify(grammar, provider.formatter(), tree, text, reparse)
    };
    let mut variants = Reparses {
        provider,
        file,
        options,
        directives,
        parses: vec![(file.preprocessor_symbols.clone(), Some(parsed.tree.clone()))],
    };
    let merged = provider
        .formatter()
        .merge_variants(&parsed.tree, &output, options, &mut variants)
        .filter(|(text, _)| *text != output);
    if let Some((text, sets)) = merged {
        let trees: Option<Vec<_>> = sets.iter().map(|s| variants.parse(s)).collect();
        let sound = trees.is_some_and(|trees| {
            check(&file.preprocessor_symbols, &parsed.tree, &text).is_ok()
                && sets
                    .iter()
                    .zip(&trees)
                    .all(|(s, t)| check(s, t, &text).is_ok())
        });
        // Any doubt: the inactive branches stay as they were.
        if sound {
            return Ok(text);
        }
    }
    // Release builds trust the formatter; tests and the corpus runs keep checking it.
    if cfg!(debug_assertions) {
        check(&file.preprocessor_symbols, &parsed.tree, &output)?;
    }
    Ok(output)
}

/// The parses of one file under other symbol sets, memoized (see [`Variants`]).
struct Reparses<'a> {
    provider: &'a dyn LanguageProvider,
    file: &'a SourceFile,
    options: &'a ResolvedOptions,
    directives: &'a DirectiveTable,
    parses: Vec<(Vec<String>, Option<SyntaxNode>)>,
}

impl Variants for Reparses<'_> {
    fn parse(&mut self, symbols: &[String]) -> Option<SyntaxNode> {
        if let Some((_, tree)) = self.parses.iter().find(|(s, _)| s == symbols) {
            return tree.clone();
        }
        let again = SourceFile {
            id: self.file.id.clone(),
            version: self.file.version + 1,
            text: self.file.text.clone(),
            had_bom: self.file.had_bom,
            language_version: self.file.language_version.clone(),
            preprocessor_symbols: symbols.to_vec(),
        };
        let tree = self
            .provider
            .parse(&again)
            .ok()
            .filter(|r| r.errors.is_empty())
            .map(|r| r.tree);
        self.parses.push((symbols.to_vec(), tree.clone()));
        tree
    }

    fn format(&mut self, tree: &SyntaxNode) -> Option<String> {
        let cx = FormatCtx::new(
            self.provider.grammar(),
            self.options,
            self.directives,
            self.provider.formatter(),
        );
        format_tree(tree, &cx).ok()
    }
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

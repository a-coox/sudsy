//! Provider-backed parsing of pattern snippets (the parse function core-patterns asks for).

use crate::provider::{LanguageProvider, ParseResult, SourceFile};
use core_patterns::SnippetParser;
use core_rules::{Family, Rule};
use core_syntax::{Grammar, SyntaxNode};
use std::sync::Arc;

/// A language provider as a [`SnippetParser`]: a snippet parses when its provider reports no
/// syntax errors.
pub struct ProviderSnippets<'a>(pub &'a dyn LanguageProvider);

impl SnippetParser for ProviderSnippets<'_> {
    fn grammar(&self) -> &Grammar {
        self.0.grammar()
    }

    fn parse(&self, text: &str) -> Result<Option<SyntaxNode>, String> {
        let parsed = self
            .0
            .parse(&SourceFile::new("pattern", 1, text))
            .map_err(|e| e.to_string())?;
        Ok(parsed.errors.is_empty().then_some(parsed.tree))
    }
}

/// Snippets of an in-process language, parsed by a function of the text.
pub struct FnSnippets(
    &'static Grammar,
    Box<dyn Fn(&str) -> ParseResult + Send + Sync>,
);

impl FnSnippets {
    pub fn new(
        grammar: &'static Grammar,
        parse: impl Fn(&str) -> ParseResult + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(FnSnippets(grammar, Box::new(parse)))
    }
}

impl SnippetParser for FnSnippets {
    fn grammar(&self) -> &Grammar {
        self.0
    }

    fn parse(&self, text: &str) -> Result<Option<SyntaxNode>, String> {
        let parsed = (self.1)(text);
        Ok(parsed.errors.is_empty().then_some(parsed.tree))
    }
}

/// `own` plus the built-in pattern files `sources` (see [`pattern_files!`]). The patterns compile
/// when a rule first runs, through `parser`: a worker language starts no worker to be listed.
pub fn with_pattern_rules(
    own: &'static [&'static dyn Rule],
    language: &str,
    sources: &[&str],
    parser: Arc<dyn SnippetParser + Send + Sync>,
) -> &'static [&'static dyn Rule] {
    with_pattern_families(own, language, sources, &[], parser)
}

/// [`with_pattern_rules`] where the files may also name one of `families`.
pub fn with_pattern_families(
    own: &'static [&'static dyn Rule],
    language: &str,
    sources: &[&str],
    families: &[Family],
    parser: Arc<dyn SnippetParser + Send + Sync>,
) -> &'static [&'static dyn Rule] {
    if sources.is_empty() {
        return own;
    }
    let mut rules = own.to_vec();
    for source in sources {
        let loaded = core_patterns::load_lazy(source, language, &parser, families)
            .unwrap_or_else(|e| panic!("built-in pattern file: {e}"));
        rules.extend(loaded.into_iter().map(|r| &*Box::leak(r)));
    }
    Box::leak(rules.into_boxed_slice())
}

/// The text of the crate's `rules/*.yaml`, listed by `build/embed-rules.rs` (set it as the
/// crate's `build` script).
#[macro_export]
macro_rules! pattern_files {
    () => {
        include!(concat!(env!("OUT_DIR"), "/pattern_rules.rs"))
    };
}

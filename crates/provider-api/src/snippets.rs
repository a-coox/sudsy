//! Provider-backed parsing of pattern snippets (the parse function core-patterns asks for).

use crate::provider::{LanguageProvider, SourceFile};
use core_patterns::SnippetParser;
use core_syntax::{Grammar, SyntaxNode};

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

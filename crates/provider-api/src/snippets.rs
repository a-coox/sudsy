//! Provider-backed parsing of pattern snippets (the parse function core-patterns asks for).

use crate::provider::{LanguageProvider, ParseResult, SourceFile};
use core_patterns::SnippetParser;
use core_rules::{Family, Rule};
use core_syntax::grammars::LazyGrammar;
use core_syntax::{Grammar, SyntaxNode};
use std::sync::{Arc, OnceLock};

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
pub struct FnSnippets(LazyGrammar, Box<dyn Fn(&str) -> ParseResult + Send + Sync>);

impl FnSnippets {
    pub fn new(
        grammar: LazyGrammar,
        parse: impl Fn(&str) -> ParseResult + Send + Sync + 'static,
    ) -> Arc<Self> {
        Arc::new(FnSnippets(grammar, Box::new(parse)))
    }
}

impl SnippetParser for FnSnippets {
    fn grammar(&self) -> &Grammar {
        self.0.get()
    }

    fn parse(&self, text: &str) -> Result<Option<SyntaxNode>, String> {
        let parsed = (self.1)(text);
        Ok(parsed.errors.is_empty().then_some(parsed.tree))
    }
}

/// A provider's rules: its own, plus the built-in pattern files (see [`pattern_files!`]), which
/// are read (and leaked) the first time [`LazyRules::all`] is asked, so a process that never lints
/// the language does not pay for them. The patterns compile when a rule first runs, through
/// `parser`: a worker language starts no worker to be listed.
pub struct LazyRules {
    own: &'static [&'static dyn Rule],
    files: Option<Files>,
    all: OnceLock<&'static [&'static dyn Rule]>,
}

struct Files {
    language: String,
    sources: &'static [&'static str],
    families: &'static [Family],
    parser: Arc<dyn SnippetParser + Send + Sync>,
}

impl LazyRules {
    /// No pattern files.
    pub fn own(own: &'static [&'static dyn Rule]) -> Self {
        LazyRules {
            own,
            files: None,
            all: OnceLock::new(),
        }
    }

    /// `own` plus the pattern files `sources`, which may also name one of `families`.
    pub fn new(
        own: &'static [&'static dyn Rule],
        language: &str,
        sources: &'static [&'static str],
        families: &'static [Family],
        parser: Arc<dyn SnippetParser + Send + Sync>,
    ) -> Self {
        LazyRules {
            files: (!sources.is_empty()).then(|| Files {
                language: language.into(),
                sources,
                families,
                parser,
            }),
            ..Self::own(own)
        }
    }

    /// The Rust rules alone: they can lay out, the pattern files cannot.
    pub fn rust(&self) -> &'static [&'static dyn Rule] {
        self.own
    }

    /// Every rule; reads the pattern files on the first call.
    pub fn all(&self) -> &'static [&'static dyn Rule] {
        let Some(files) = &self.files else {
            return self.own;
        };
        self.all.get_or_init(|| {
            let mut rules = self.own.to_vec();
            for source in files.sources {
                let loaded = core_patterns::load_lazy(
                    source,
                    &files.language,
                    &files.parser,
                    files.families,
                )
                .unwrap_or_else(|e| panic!("built-in pattern file: {e}"));
                rules.extend(loaded.into_iter().map(|r| &*Box::leak(r)));
            }
            Box::leak(rules.into_boxed_slice())
        })
    }
}

/// The text of the crate's `rules/*.yaml`, listed by `build/lang-crate.rs` (set it as the
/// crate's `build` script).
#[macro_export]
macro_rules! pattern_files {
    () => {
        include!(concat!(env!("OUT_DIR"), "/pattern_rules.rs"))
    };
}

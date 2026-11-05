//! A [`LanguageProvider`] for a language parsed by tree-sitter, configured by data.

use crate::{Engine, Mapping};
use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{Rule, RuleHooks};
use core_syntax::{Grammar, LanguageId, SyntaxNode};
use provider_api::{
    Capabilities, FileMatcher, FnSnippets, LanguageProvider, ParseError, ParseResult,
    ProviderError, SourceFile,
};
use std::sync::Arc;

/// What makes a tree-sitter language a provider.
pub struct Spec {
    pub language: tree_sitter::Language,
    pub grammar: &'static Grammar,
    pub mapping: &'static Mapping,
    /// File extensions without the dot.
    pub extensions: &'static [&'static str],
    pub schema: OptionSchema,
    pub formatter: &'static dyn NodeFormatter,
    pub hooks: &'static dyn RuleHooks,
    /// The language's own Rust rules.
    pub rules: &'static [&'static dyn Rule],
    /// The text of the built-in pattern files (`provider_api::pattern_files!()`).
    pub pattern_files: &'static [&'static str],
    /// Semantic checks the grammar cannot express; run on a tree without recovery nodes, and
    /// reported as parse errors.
    pub validate: Option<fn(&SyntaxNode, &str) -> Vec<ParseError>>,
}

pub struct TreeSitterProvider {
    grammar: &'static Grammar,
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    engine: Arc<Engine>,
    formatter: &'static dyn NodeFormatter,
    hooks: &'static dyn RuleHooks,
    rules: &'static [&'static dyn Rule],
    validate: Option<fn(&SyntaxNode, &str) -> Vec<ParseError>>,
}

impl TreeSitterProvider {
    pub fn new(spec: Spec) -> Self {
        let engine = Arc::new(Engine::new(spec.language, spec.grammar, spec.mapping));
        let parser = engine.clone();
        TreeSitterProvider {
            grammar: spec.grammar,
            matchers: spec
                .extensions
                .iter()
                .map(|e| FileMatcher::extension(e))
                .collect(),
            schema: spec.schema,
            rules: provider_api::with_pattern_rules(
                spec.rules,
                &spec.grammar.language,
                spec.pattern_files,
                FnSnippets::new(spec.grammar, move |text| parser.parse(text)),
            ),
            engine,
            formatter: spec.formatter,
            hooks: spec.hooks,
            validate: spec.validate,
        }
    }

    /// The parse engine, for benchmarks that time tree-sitter and the conversion apart.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl LanguageProvider for TreeSitterProvider {
    fn id(&self) -> LanguageId {
        self.grammar.language_id
    }

    fn grammar(&self) -> &Grammar {
        self.grammar
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        let mut parsed = self.engine.parse(&file.text);
        if let (true, Some(validate)) = (parsed.errors.is_empty(), self.validate) {
            parsed.errors = validate(&parsed.tree, &file.text);
        }
        Ok(parsed)
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        self.formatter
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.rules
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        self.hooks
    }
}

//! A [`LanguageProvider`] for a language parsed by tree-sitter, configured by data.

use crate::{Composite, Engine, Mapping};
use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{Family, Rule, RuleHooks};
use core_syntax::grammars::LazyGrammar;
use core_syntax::{Embedded, Grammar, LanguageId, SyntaxNode};
use provider_api::{
    Capabilities, FileMatcher, FilePattern, FnSnippets, LanguageProvider, LazyRules, ParseError,
    ParseResult, ProviderError, SourceFile,
};
use std::sync::Arc;

/// What makes a tree-sitter language a provider.
pub struct Spec {
    /// `env!("CARGO_PKG_NAME")` of the language's crate.
    pub crate_name: &'static str,
    /// Resolved on the first parse: a linked-in grammar, or the language's installed pack.
    pub language: fn() -> tree_sitter::Language,
    pub grammar: LazyGrammar,
    pub mapping: &'static Mapping,
    /// File extensions without the dot.
    pub extensions: &'static [&'static str],
    /// Exact file names (`.editorconfig`).
    pub file_names: &'static [&'static str],
    pub schema: OptionSchema,
    pub formatter: &'static dyn NodeFormatter,
    pub hooks: &'static dyn RuleHooks,
    /// The language's own Rust rules.
    pub rules: &'static [&'static dyn Rule],
    /// The text of the built-in pattern files (`provider_api::pattern_files!()`).
    pub pattern_files: &'static [&'static str],
    /// The families the pattern files may name.
    pub families: &'static [Family],
    /// Semantic checks the grammar cannot express; run on a tree without recovery nodes, and
    /// reported as parse errors.
    pub validate: Option<fn(&SyntaxNode, &str) -> Vec<ParseError>>,
}

pub struct TreeSitterProvider {
    crate_name: &'static str,
    grammar: LazyGrammar,
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    engine: Arc<Engine>,
    formatter: &'static dyn NodeFormatter,
    hooks: &'static dyn RuleHooks,
    rules: LazyRules,
    validate: Option<fn(&SyntaxNode, &str) -> Vec<ParseError>>,
    embedded: Option<fn(&SyntaxNode, &str) -> Vec<Embedded>>,
}

impl TreeSitterProvider {
    /// Finds the code of other languages in a tree (a function of the language crate).
    pub fn with_embedded(mut self, find: fn(&SyntaxNode, &str) -> Vec<Embedded>) -> Self {
        self.embedded = Some(find);
        self
    }

    pub fn new(spec: Spec) -> Self {
        Self::with_composite(spec, None)
    }

    /// A provider whose language is assembled from several grammars.
    pub fn with_composite(spec: Spec, composite: Option<Composite>) -> Self {
        let engine = Arc::new(Engine::with_composite(
            spec.language,
            spec.grammar,
            spec.mapping,
            composite,
        ));
        let parser = engine.clone();
        TreeSitterProvider {
            crate_name: spec.crate_name,
            grammar: spec.grammar,
            matchers: spec
                .extensions
                .iter()
                .map(|e| FileMatcher::extension(e))
                .chain(spec.file_names.iter().map(|n| FileMatcher {
                    pattern: FilePattern::FileName((*n).into()),
                    dialect: None,
                }))
                .collect(),
            schema: spec.schema,
            rules: LazyRules::new(
                spec.rules,
                spec.grammar.language,
                spec.pattern_files,
                spec.families,
                FnSnippets::new(spec.grammar, move |text| parser.parse(text)),
            ),
            engine,
            formatter: spec.formatter,
            hooks: spec.hooks,
            validate: spec.validate,
            embedded: None,
        }
    }

    /// The parse engine, for benchmarks that time tree-sitter and the conversion apart.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl LanguageProvider for TreeSitterProvider {
    fn id(&self) -> LanguageId {
        self.grammar.id
    }

    fn language(&self) -> &str {
        self.grammar.language
    }

    fn crate_name(&self) -> &'static str {
        self.crate_name
    }

    fn grammar(&self) -> &Grammar {
        self.grammar.get()
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

    fn embedded(&self, root: &SyntaxNode, text: &str) -> Vec<Embedded> {
        self.embedded.map_or_else(Vec::new, |find| find(root, text))
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        self.formatter
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.rules.all()
    }

    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        self.rules.rust()
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        self.hooks
    }
}

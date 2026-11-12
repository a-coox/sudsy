//! Vue : HTML plus template syntax. The provider is an HTML one with another
//! tree-sitter grammar mapped onto the same tree (`grammars/html.grammar`), its own file
//! matchers and `vue` / `svelte` config sections on top of `html`.

use crate::MAPPING;
use crate::format::{VUE_FORMATTER, option_schema};
use core_config::{OptionDef, OptionSchema};
use core_format::NodeFormatter;
use core_rules::{Rule, RuleHooks};
use core_syntax::{Embedded, Grammar, LanguageId, SyntaxNode};
use core_treesitter::{Mapping, Spec, TreeSitterProvider};
use provider_api::{
    Capabilities, FileMatcher, LanguageProvider, ParseResult, ProviderError, SourceFile,
};

/// HTML's node table with the nodes of another grammar added.
fn mapping(extra: &[(&'static str, &'static str)]) -> &'static Mapping {
    let mut kinds = MAPPING.kinds.to_vec();
    kinds.extend_from_slice(extra);
    Box::leak(Box::new(Mapping {
        whitespace: MAPPING.whitespace,
        newline: MAPPING.newline,
        eof: MAPPING.eof,
        unknown: MAPPING.unknown,
        kinds: Box::leak(kinds.into_boxed_slice()),
        wrap: MAPPING.wrap,
    }))
}

/// The options of HTML and, in front of the shared ones, `extra`.
fn schema(extra: Vec<OptionDef>) -> OptionSchema {
    let base = option_schema();
    let mut defs = base.defs().to_vec();
    defs.extend(extra);
    OptionSchema::new(defs)
}

pub struct DialectProvider {
    name: &'static str,
    inner: TreeSitterProvider,
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
}

impl DialectProvider {
    fn new(name: &'static str, inner: TreeSitterProvider, schema: OptionSchema) -> Self {
        DialectProvider {
            name,
            inner,
            matchers: vec![FileMatcher::extension(name)],
            schema,
        }
    }
}

macro_rules! spec {
    ($name:literal, $krate:ident, $mapping:expr, $formatter:expr) => {
        Spec {
            crate_name: env!("CARGO_PKG_NAME"),
            language: || core_packs::language!($name, $krate::LANGUAGE),
            grammar: core_syntax::grammars::HTML,
            mapping: $mapping,
            extensions: &[$name],
            file_names: &[],
            schema: option_schema(),
            formatter: $formatter,
            hooks: &core_rules::DEFAULT_HOOKS,
            rules: crate::rules::RULES,
            pattern_files: provider_api::pattern_files!(),
            families: &[],
            validate: None,
        }
    };
}

/// `.vue` single-file components.
pub fn vue_provider() -> DialectProvider {
    let m = mapping(&[
        ("template_element", "Element"),
        ("directive_attribute", "Attribute"),
        ("directive_name", "AttrName"),
        ("directive_value", "AttrName"),
        ("dynamic_directive_value", "AttrName"),
        ("directive_modifier", "AttrName"),
        ("\":\"", "AttrName"),
        ("\".\"", "AttrName"),
        ("\"@\"", "AttrName"),
        ("\"#\"", "AttrName"),
        ("interpolation", "Interpolation"),
    ]);
    let options = vec![OptionDef::boolean(
        crate::format::INDENT_SCRIPT_AND_STYLE,
        false,
        "Indent the content of the root `<script>` and `<style>` blocks of a `.vue` file by one level.",
    )];
    let inner = TreeSitterProvider::new(spec!("vue", tree_sitter_vue, m, &VUE_FORMATTER));
    DialectProvider::new("vue", inner, schema(options))
}

impl LanguageProvider for DialectProvider {
    fn id(&self) -> LanguageId {
        self.inner.id()
    }

    fn language(&self) -> &str {
        self.name
    }

    fn crate_name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn grammar(&self) -> &Grammar {
        self.inner.grammar()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        self.inner.parse(file)
    }

    fn embedded(&self, root: &SyntaxNode, text: &str) -> Vec<Embedded> {
        self.inner.embedded(root, text)
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        self.inner.formatter()
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn config_base(&self) -> Option<&str> {
        Some("html")
    }

    fn base_rules(&self) -> &'static [&'static dyn Rule] {
        self.inner.rules()
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.inner.rules()
    }

    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        self.inner.rust_rules()
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        self.inner.rule_hooks()
    }
}

//! The YAML provider: a hand-written lossless YAML 1.2 parser, the data model used by the
//! safety check, and the formatter.

mod format;
core_syntax::include_generated!();
mod anchor_rules;
mod hooks;
pub mod model;
mod parser;
mod quoted_strings;
mod resolve;
mod rules;
pub mod scalar;
mod tree;
mod value_rules;

pub use format::{
    COMMENT_SPACING, DOCUMENT_START, MAX_BLANK_LINES, QUOTE_STYLE, SEQUENCE_INDENT, YamlFormatter,
    option_schema,
};
pub use parser::{MAX_DEPTH, Parsed, parse};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{Rule, RuleHooks};
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, FnSnippets, LanguageProvider, ParseResult, ProviderError, SourceFile,
};

pub struct YamlProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: YamlFormatter,
    rules: &'static [&'static dyn Rule],
}

impl Default for YamlProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl YamlProvider {
    pub fn new() -> Self {
        YamlProvider {
            matchers: vec![
                FileMatcher::extension("yaml"),
                FileMatcher::extension("yml"),
            ],
            schema: option_schema(),
            formatter: YamlFormatter,
            rules: provider_api::with_pattern_rules(
                rules::RULES,
                &grammars::yaml().language,
                provider_api::pattern_files!(),
                FnSnippets::new(grammars::yaml(), parse_result),
            ),
        }
    }
}

fn parse_result(text: &str) -> ParseResult {
    let parsed = parser::parse(text);
    ParseResult {
        tree: parsed.tree,
        errors: parsed.errors,
    }
}

impl LanguageProvider for YamlProvider {
    fn id(&self) -> LanguageId {
        LanguageId::YAML
    }

    fn grammar(&self) -> &Grammar {
        grammars::yaml()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        Ok(parse_result(&file.text))
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        &self.formatter
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        self.rules
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        &hooks::YamlHooks
    }
}

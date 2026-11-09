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
    Capabilities, FileMatcher, FnSnippets, LanguageProvider, LazyRules, ParseResult, ProviderError,
    SourceFile,
};

pub struct YamlProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: YamlFormatter,
    rules: LazyRules,
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
            rules: LazyRules::new(
                rules::RULES,
                grammars::YAML.language,
                provider_api::pattern_files!(),
                &[],
                FnSnippets::new(grammars::YAML, parse_result),
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

    fn language(&self) -> &str {
        grammars::YAML.language
    }

    fn crate_name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn grammar(&self) -> &Grammar {
        grammars::YAML.get()
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
        self.rules.all()
    }

    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        self.rules.rust()
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        &hooks::YamlHooks
    }
}

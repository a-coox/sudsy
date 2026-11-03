//! The YAML provider: a hand-written lossless YAML 1.2 parser, the data model used by the
//! safety check, and the formatter.

mod format;
#[rustfmt::skip]
pub mod generated;
pub mod model;
mod parser;
mod rules;
pub mod scalar;
mod tree;

pub use format::{
    COMMENT_SPACING, DOCUMENT_START, MAX_BLANK_LINES, QUOTE_STYLE, SEQUENCE_INDENT, YamlFormatter,
    option_schema,
};
pub use parser::{MAX_DEPTH, Parsed, parse};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::Rule;
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, LanguageProvider, ParseResult, ProviderError, SourceFile,
};

pub struct YamlProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: YamlFormatter,
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
        }
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
        let parsed = parser::parse(&file.text);
        Ok(ParseResult {
            tree: parsed.tree,
            errors: parsed.errors,
        })
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        &self.formatter
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rules(&self) -> &'static [&'static dyn Rule] {
        rules::RULES
    }
}

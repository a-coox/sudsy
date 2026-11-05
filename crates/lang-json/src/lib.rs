//! The JSON and JSONC provider: a hand-written lossless parser, and formatting that is almost
//! entirely the shared shapes' tag defaults.

#[rustfmt::skip]
pub mod generated;
mod format;
mod hooks;
mod lexer;
mod parser;
mod rules;
mod string;
mod value_rules;

pub use format::{COLLAPSE, DIALECT, JsonFormatter, TRAILING_COMMAS, option_schema};
pub use parser::{Dialect, MAX_DEPTH};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::{Rule, RuleHooks};
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, FilePattern, FnSnippets, LanguageProvider, ParseResult,
    ProviderError, SourceFile,
};
use std::path::Path;

pub struct JsonProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: JsonFormatter,
    rules: &'static [&'static dyn Rule],
}

impl Default for JsonProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonProvider {
    pub fn new() -> Self {
        let matcher = |pattern, dialect: &str| FileMatcher {
            pattern,
            dialect: Some(dialect.into()),
        };
        let name = |n: &str| FilePattern::FileName(n.into());
        JsonProvider {
            matchers: vec![
                matcher(FilePattern::Extension("json".into()), "json"),
                matcher(FilePattern::Extension("jsonc".into()), "jsonc"),
                matcher(name("tsconfig.json"), "jsonc"),
                matcher(
                    FilePattern::InDirectory {
                        dir: ".vscode".into(),
                        extension: "json".into(),
                    },
                    "jsonc",
                ),
            ],
            schema: option_schema(),
            formatter: JsonFormatter,
            rules: provider_api::with_pattern_rules(
                rules::RULES,
                &grammars::json().language,
                provider_api::pattern_files!(),
                FnSnippets::new(grammars::json(), |text| parse_result(text, Dialect::Json)),
            ),
        }
    }

    /// The dialect a file is parsed as: the one chosen in `file.language_version` (set from the
    /// `dialect` option or the matcher), else decided by the file name.
    fn dialect_of(&self, file: &SourceFile) -> Dialect {
        let named = file
            .language_version
            .as_deref()
            .and_then(Dialect::from_name);
        named.unwrap_or_else(|| {
            let path = Path::new(&file.id);
            self.matchers
                .iter()
                .filter(|m| m.matches(path))
                .max_by_key(|m| m.specificity())
                .and_then(|m| m.dialect.as_deref())
                .and_then(Dialect::from_name)
                .unwrap_or(Dialect::Json)
        })
    }
}

fn parse_result(text: &str, dialect: Dialect) -> ParseResult {
    let parsed = parser::parse(text, dialect);
    ParseResult {
        tree: parsed.tree,
        errors: parsed.errors,
    }
}

impl LanguageProvider for JsonProvider {
    fn id(&self) -> LanguageId {
        LanguageId::JSON
    }

    fn grammar(&self) -> &Grammar {
        grammars::json()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        Ok(parse_result(&file.text, self.dialect_of(file)))
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
        &hooks::JsonHooks
    }
}

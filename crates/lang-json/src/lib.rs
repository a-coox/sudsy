//! The JSON and JSONC provider: a hand-written lossless parser, and formatting that is almost
//! entirely the shared shapes' tag defaults.

mod format;
mod lexer;
mod parser;

pub use format::{COLLAPSE, DIALECT, JsonFormatter, TRAILING_COMMAS, option_schema};
pub use parser::{Dialect, MAX_DEPTH};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, FilePattern, LanguageProvider, ParseResult, ProviderError,
    SourceFile,
};
use std::path::Path;

pub struct JsonProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: JsonFormatter,
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
        let parsed = parser::parse(&file.text, self.dialect_of(file));
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
}

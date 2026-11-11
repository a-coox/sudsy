//! CSS, SCSS and Less: one hand-written lossless parser for the three dialects, and three
//! providers over it. `scss` and `less` take `css` as their config base.

core_syntax::include_generated!();
mod format;
mod lexer;
mod parser;

pub use format::{CssFormatter, option_schema};
pub use parser::MAX_DEPTH;

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, LanguageProvider, ParseResult, ProviderError, SourceFile,
};

/// The syntax a file is lexed with: `//` comments are SCSS and Less only, `$name` and `#{}` SCSS
/// only, `@{}` Less only.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dialect {
    Css,
    Scss,
    Less,
}

impl Dialect {
    fn name(self) -> &'static str {
        match self {
            Dialect::Css => "css",
            Dialect::Scss => "scss",
            Dialect::Less => "less",
        }
    }
}

pub struct CssProvider {
    dialect: Dialect,
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: CssFormatter,
}

impl CssProvider {
    pub fn new(dialect: Dialect) -> Self {
        CssProvider {
            dialect,
            matchers: vec![FileMatcher::extension(dialect.name())],
            schema: option_schema(),
            formatter: CssFormatter,
        }
    }
}

impl LanguageProvider for CssProvider {
    fn id(&self) -> LanguageId {
        grammars::CSS.id
    }

    fn language(&self) -> &str {
        self.dialect.name()
    }

    fn crate_name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn grammar(&self) -> &Grammar {
        grammars::CSS.get()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        let parsed = parser::parse(&file.text, self.dialect);
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

    fn config_base(&self) -> Option<&str> {
        (self.dialect != Dialect::Css).then_some("css")
    }
}

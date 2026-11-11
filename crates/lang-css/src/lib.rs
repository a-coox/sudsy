//! CSS, SCSS and Less: one hand-written lossless parser for the three dialects, and three
//! providers over it. `scss` and `less` take `css` as their config base.

core_syntax::include_generated!();
mod format;
mod lexer;
mod parser;
mod rules;

pub use format::{CssFormatter, option_schema};
pub use parser::MAX_DEPTH;

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::Rule;
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, FnSnippets, LanguageProvider, LazyRules, ParseResult, ProviderError,
    SourceFile,
};
use std::sync::LazyLock;

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

/// The rules of the three languages are the same files; built on first use, for all of them. Pattern
/// files here use no snippets, so one parse function serves.
static RULES: LazyLock<LazyRules> = LazyLock::new(|| {
    LazyRules::new(
        &[],
        grammars::CSS.language,
        provider_api::pattern_files!(),
        rules::FAMILIES,
        FnSnippets::new(grammars::CSS, |text| parse_result(text, Dialect::Css)),
    )
});

impl CssProvider {
    pub fn new(dialect: Dialect) -> Self {
        CssProvider {
            dialect,
            matchers: vec![FileMatcher::extension(dialect.name())],
            schema: option_schema(),
            formatter: CssFormatter::new(dialect),
        }
    }
}

fn parse_result(text: &str, dialect: Dialect) -> ParseResult {
    let parsed = parser::parse(text, dialect);
    ParseResult {
        tree: parsed.tree,
        errors: parsed.errors,
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
        Ok(parse_result(&file.text, self.dialect))
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

    fn rules(&self) -> &'static [&'static dyn Rule] {
        RULES.all()
    }

    fn rust_rules(&self) -> &'static [&'static dyn Rule] {
        RULES.rust()
    }
}

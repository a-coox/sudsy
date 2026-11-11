//! The JavaScript and TypeScript provider (`.js .mjs .cjs .jsx .ts .mts .cts .tsx`): oxc parses,
//! `adapter` turns its AST and tokens into the lossless tree, the formatter prints the file as
//! written.

core_syntax::include_generated!();
mod adapter;
mod format;
mod rules;
mod safety;

pub use adapter::{parse, source_type};
pub use format::{JsFormatter, option_schema};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::Rule;
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, FnSnippets, LanguageProvider, LazyRules, ParseResult, ProviderError,
    SourceFile,
};

const EXTENSIONS: [&str; 8] = ["js", "mjs", "cjs", "jsx", "ts", "mts", "cts", "tsx"];

pub struct JsProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: JsFormatter,
    rules: LazyRules,
}

impl Default for JsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl JsProvider {
    pub fn new() -> Self {
        JsProvider {
            matchers: EXTENSIONS
                .iter()
                .map(|e| FileMatcher::extension(e))
                .collect(),
            schema: option_schema(),
            formatter: JsFormatter,
            rules: LazyRules::new(
                rules::RULES,
                grammars::JS.language,
                provider_api::pattern_files!(),
                &[],
                FnSnippets::new(grammars::JS, |text| parse(text, source_type(""))),
            ),
        }
    }
}

impl LanguageProvider for JsProvider {
    fn id(&self) -> LanguageId {
        grammars::JS.id
    }

    fn language(&self) -> &str {
        grammars::JS.language
    }

    fn crate_name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn grammar(&self) -> &Grammar {
        grammars::JS.get()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        Ok(parse(&file.text, source_type(&file.id)))
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
}

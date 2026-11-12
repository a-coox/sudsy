//! The Python provider (`.py .pyi .pyw`): ruff's parser parses, `adapter` turns its AST and tokens
//! into the lossless tree, the formatter prints the file in Black's style (`ruff format`).

core_syntax::include_generated!();
mod adapter;
mod format;
mod rules;

pub use adapter::parse;
pub use format::{PythonFormatter, option_schema};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::Rule;
use core_syntax::{Grammar, LanguageId, grammars};
use provider_api::{
    Capabilities, FileMatcher, FnSnippets, LanguageProvider, LazyRules, ParseResult, ProviderError,
    SourceFile,
};
use ruff_python_ast::PythonVersion;

const EXTENSIONS: [&str; 3] = ["py", "pyi", "pyw"];

pub struct PythonProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    formatter: PythonFormatter,
    rules: LazyRules,
}

impl Default for PythonProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl PythonProvider {
    pub fn new() -> Self {
        PythonProvider {
            matchers: EXTENSIONS
                .iter()
                .map(|e| FileMatcher::extension(e))
                .collect(),
            schema: option_schema(),
            formatter: PythonFormatter,
            rules: LazyRules::new(
                rules::RULES,
                grammars::PYTHON.language,
                provider_api::pattern_files!(),
                rules::FAMILIES,
                FnSnippets::new(grammars::PYTHON, |text| parse(text, PythonVersion::latest())),
            ),
        }
    }
}

/// The newest stable version unless the file's `target_version` names another.
fn target_version(file: &SourceFile) -> PythonVersion {
    file.language_version
        .as_deref()
        .and_then(|v| v.parse().ok())
        .unwrap_or(PythonVersion::latest())
}

impl LanguageProvider for PythonProvider {
    fn id(&self) -> LanguageId {
        grammars::PYTHON.id
    }

    fn language(&self) -> &str {
        grammars::PYTHON.language
    }

    fn crate_name(&self) -> &'static str {
        env!("CARGO_PKG_NAME")
    }

    fn grammar(&self) -> &Grammar {
        grammars::PYTHON.get()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        Ok(parse(&file.text, target_version(file)))
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

//! The TOML provider: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`),
//! formatting is the shared shapes plus the table layout in `format.rs`.

#[rustfmt::skip]
pub mod generated;
mod align;
mod format;
mod hooks;
mod mapping;
mod validate;

pub use format::{
    ALIGN_COMMENTS, COLLAPSE, MAX_BLANK_LINES, TRAILING_COMMAS, TomlFormatter, option_schema,
};

use core_config::OptionSchema;
use core_format::NodeFormatter;
use core_rules::RuleHooks;
use core_syntax::{Grammar, LanguageId, grammars};
use core_treesitter::Engine;
use provider_api::{
    Capabilities, FileMatcher, LanguageProvider, ParseResult, ProviderError, SourceFile,
};

pub struct TomlProvider {
    matchers: Vec<FileMatcher>,
    schema: OptionSchema,
    engine: Engine,
}

impl Default for TomlProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TomlProvider {
    pub fn new() -> Self {
        TomlProvider {
            matchers: vec![FileMatcher::extension("toml")],
            schema: option_schema(),
            engine: Engine::new(
                tree_sitter_toml_ng::LANGUAGE.into(),
                grammars::toml(),
                &mapping::MAPPING,
            ),
        }
    }

    /// The parse engine, for benchmarks that time tree-sitter and the conversion apart.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }
}

impl LanguageProvider for TomlProvider {
    fn id(&self) -> LanguageId {
        generated::LANGUAGE
    }

    fn grammar(&self) -> &Grammar {
        grammars::toml()
    }

    fn file_matchers(&self) -> &[FileMatcher] {
        &self.matchers
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::NONE
    }

    fn parse(&self, file: &SourceFile) -> Result<ParseResult, ProviderError> {
        let mut parsed = self.engine.parse(&file.text);
        // Semantic checks need a tree without recovery nodes.
        if parsed.errors.is_empty() {
            parsed.errors = validate::validate(&parsed.tree, &file.text);
        }
        Ok(parsed)
    }

    fn formatter(&self) -> &dyn NodeFormatter {
        &TomlFormatter
    }

    fn option_schema(&self) -> &OptionSchema {
        &self.schema
    }

    fn rule_hooks(&self) -> &dyn RuleHooks {
        &hooks::TomlHooks
    }
}

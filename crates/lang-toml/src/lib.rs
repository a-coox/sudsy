//! TOML: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`), formatting is
//! the shared shapes plus the table layout in `format.rs`.

core_syntax::include_generated!();
mod align;
mod format;
mod hooks;
mod mapping;
mod rules;
mod validate;

pub use format::{
    ALIGN_COMMENTS, COLLAPSE, MAX_BLANK_LINES, TRAILING_COMMAS, TomlFormatter, option_schema,
};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec {
        language: tree_sitter_toml_ng::LANGUAGE.into(),
        grammar: grammars::toml(),
        mapping: &mapping::MAPPING,
        extensions: &["toml"],
        schema: option_schema(),
        formatter: &TomlFormatter,
        hooks: &hooks::TomlHooks,
        rules: rules::RULES,
        pattern_files: provider_api::pattern_files!(),
        validate: Some(validate::validate),
    })
}

//! EditorConfig: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`),
//! formatting is the shared sequence shape plus the section layout in `format.rs`.

core_syntax::include_generated!();
mod format;
mod hooks;
mod mapping;
mod rules;

pub use format::{EditorConfigFormatter, option_schema};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec {
        language: konomanoasa_tree_sitter_editorconfig::LANGUAGE.into(),
        grammar: grammars::editorconfig(),
        mapping: &mapping::MAPPING,
        extensions: &[],
        file_names: &[".editorconfig"],
        schema: option_schema(),
        formatter: &EditorConfigFormatter,
        hooks: &hooks::EditorConfigHooks,
        rules: rules::RULES,
        pattern_files: provider_api::pattern_files!(),
        validate: None,
    })
}

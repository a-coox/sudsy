//! POSIX sh and Bash: tree-sitter parses (through `core-treesitter`, mapping in `mapping.rs`),
//! the formatter in `format.rs` lays out the statements and keeps every word as written.

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;

pub use format::{ShellFormatter, option_schema};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(
        Spec {
            language: tree_sitter_bash::LANGUAGE.into(),
            grammar: grammars::shell(),
            mapping: &mapping::MAPPING,
            extensions: &["sh", "bash"],
            file_names: &[".bashrc", ".bash_profile", ".profile"],
            schema: option_schema(),
            formatter: &ShellFormatter,
            hooks: &core_rules::DEFAULT_HOOKS,
            rules: rules::RULES,
            pattern_files: provider_api::pattern_files!(),
            families: &[],
            validate: None,
        },
        Some(mapping::COMPOSITE),
    )
}

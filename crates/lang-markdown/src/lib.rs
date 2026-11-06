//! Markdown: tree-sitter-md's block grammar parses, its inline grammar is injected into the
//! `inline` and `pipe_table_cell` nodes (`core-treesitter` composite, mapping in `mapping.rs`).

core_syntax::include_generated!();
mod format;
mod mapping;
mod rules;

pub use format::{MarkdownFormatter, option_schema};

use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(
        Spec {
            language: || core_packs::language!("markdown", tree_sitter_md::LANGUAGE),
            grammar: grammars::MARKDOWN,
            mapping: &mapping::BLOCK,
            extensions: &["md", "markdown"],
            file_names: &[],
            schema: option_schema(),
            formatter: &MarkdownFormatter,
            hooks: &core_rules::DEFAULT_HOOKS,
            rules: rules::RULES,
            pattern_files: provider_api::pattern_files!(),
            families: &[],
            validate: None,
        },
        Some(mapping::COMPOSITE),
    )
}

// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("markdown", tree_sitter_md::LANGUAGE), grammar: grammars::MARKDOWN, mapping: &mapping::BLOCK, extensions: &["md", "markdown"], file_names: &[], schema: option_schema(), formatter: &MarkdownFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None }, Some(mapping::COMPOSITE)).with_embedded(format::find_embedded)
}

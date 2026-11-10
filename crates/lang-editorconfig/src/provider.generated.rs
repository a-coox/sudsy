// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("editorconfig", konomanoasa_tree_sitter_editorconfig::LANGUAGE), grammar: grammars::EDITORCONFIG, mapping: &mapping::MAPPING, extensions: &[], file_names: &[".editorconfig"], schema: option_schema(), formatter: &EditorConfigFormatter, hooks: &hooks::EditorConfigHooks, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("makefile", tree_sitter_make::LANGUAGE), grammar: grammars::MAKEFILE, mapping: &mapping::MAPPING, extensions: &["mk", "mak"], file_names: &["Makefile", "GNUmakefile", "makefile"], schema: option_schema(), formatter: &MakefileFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None }, Some(mapping::COMPOSITE)).with_embedded(embedded::find)
}

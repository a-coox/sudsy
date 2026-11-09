// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("toml", tree_sitter_toml_ng::LANGUAGE), grammar: grammars::TOML, mapping: &mapping::MAPPING, extensions: &["toml"], file_names: &[], schema: option_schema(), formatter: &TomlFormatter, hooks: &hooks::TomlHooks, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: Some(validate::validate) })
}

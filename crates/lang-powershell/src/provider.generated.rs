// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("powershell", woodruffw_trough_tree_sitter_powershell::LANGUAGE), grammar: grammars::POWERSHELL, mapping: &mapping::MAPPING, extensions: &["ps1", "psm1", "psd1"], file_names: &[], schema: option_schema(), formatter: &PowerShellFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None }, Some(mapping::COMPOSITE))
}

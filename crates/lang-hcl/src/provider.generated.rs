// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("hcl", tree_sitter_hcl::LANGUAGE), grammar: grammars::HCL, mapping: &mapping::MAPPING, extensions: &["hcl", "tf", "tfvars", "nomad"], file_names: &[], schema: option_schema(), formatter: &HclFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("dockerfile", tree_sitter_containerfile::LANGUAGE), grammar: grammars::DOCKERFILE, mapping: &mapping::MAPPING, extensions: &["dockerfile"], file_names: &["Dockerfile", "Containerfile", "Dockerfile.*", "Containerfile.*"], schema: option_schema(), formatter: &DockerfileFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: rules::FAMILIES, validate: None }).with_embedded(embedded::find)
}

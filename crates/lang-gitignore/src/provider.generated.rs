// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("gitignore", konomanoasa_tree_sitter_gitignore::LANGUAGE), grammar: grammars::GITIGNORE, mapping: &mapping::MAPPING, extensions: &["gitignore", "dockerignore"], file_names: &[".gitignore", ".dockerignore", ".npmignore", ".prettierignore", ".eslintignore", ".sudsyignore", ".git/info/exclude"], schema: option_schema(), formatter: &GitignoreFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

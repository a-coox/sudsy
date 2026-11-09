// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::with_composite(Spec { language: || core_packs::language!("shell", tree_sitter_bash::LANGUAGE), grammar: grammars::SHELL, mapping: &mapping::MAPPING, extensions: &["sh", "bash"], file_names: &[".bashrc", ".bash_profile", ".profile"], schema: option_schema(), formatter: &ShellFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None }, Some(mapping::COMPOSITE))
}

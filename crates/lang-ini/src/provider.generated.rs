// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Spec, TreeSitterProvider};

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { language: || core_packs::language!("ini", tree_sitter_ini::LANGUAGE), grammar: grammars::INI, mapping: &mapping::MAPPING, extensions: &["ini", "cfg"], file_names: &[".gitconfig", ".gitmodules"], schema: option_schema(), formatter: &IniFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

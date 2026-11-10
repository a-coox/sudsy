// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Mapping, Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("ini", tree_sitter_ini::LANGUAGE), grammar: grammars::INI, mapping: &MAPPING, extensions: &["ini", "cfg"], file_names: &[".gitconfig", ".gitmodules"], schema: option_schema(), formatter: &IniFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
    ],
    kinds: &[
        ("setting_name", "Key"),
        ("text", "Name"),
        ("section_name", "SectionHeader"),
        ("setting_value", "Value"),
        ("document", "Document"),
        ("section", "Section"),
        ("setting", "Setting"),
        ("comment", "Comment"),
        ("continuation", "Continuation"),
        ("[", "LBracket"),
        ("]", "RBracket"),
        ("=", "Equals"),
    ],
};

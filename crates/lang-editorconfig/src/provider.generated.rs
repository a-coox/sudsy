// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Mapping, Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("editorconfig", konomanoasa_tree_sitter_editorconfig::LANGUAGE), grammar: grammars::EDITORCONFIG, mapping: &MAPPING, extensions: &[], file_names: &[".editorconfig"], schema: option_schema(), formatter: &EditorConfigFormatter, hooks: &hooks::EditorConfigHooks, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
    ],
    kinds: &[
        ("assignment_operator", "Equals"),
        ("pattern", "Glob"),
        ("section_open", "LBracket"),
        ("line_ending", "Newline"),
        ("section_close", "RBracket"),
        ("document", "Document"),
        ("section", "Section"),
        ("section_header", "SectionHeader"),
        ("pair", "Pair"),
        ("comment", "Comment"),
        ("key", "Key"),
        ("value", "Value"),
        ("[", "LBracket"),
        ("]", "RBracket"),
        ("=", "Equals"),
    ],
};

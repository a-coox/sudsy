// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Mapping, Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("gitignore", konomanoasa_tree_sitter_gitignore::LANGUAGE), grammar: grammars::GITIGNORE, mapping: &MAPPING, extensions: &["gitignore", "dockerignore"], file_names: &[".gitignore", ".dockerignore", ".npmignore", ".prettierignore", ".eslintignore", ".sudsyignore", ".git/info/exclude"], schema: option_schema(), formatter: &GitignoreFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
    ],
    kinds: &[
        ("pattern", "Entry"),
        ("line_ending", "Newline"),
        ("negation", "Pattern"),
        ("glob_literal", "Pattern"),
        ("wildcard", "Pattern"),
        ("single_character", "Pattern"),
        ("recursive_wildcard", "Pattern"),
        ("path_separator", "Pattern"),
        ("escape", "Pattern"),
        ("set_open", "Pattern"),
        ("set_close", "Pattern"),
        ("set_negation", "Pattern"),
        ("set_text", "Pattern"),
        ("class_name", "Pattern"),
        ("range_character", "Pattern"),
        ("range_operator", "Pattern"),
        ("[", "Pattern"),
        ("]", "Pattern"),
        (":", "Pattern"),
        ("blank_line", "Whitespace"),
        ("trailing_space", "Whitespace"),
        ("trailing_carriage_return", "Whitespace"),
        ("document", "Document"),
        ("comment", "Comment"),
    ],
};

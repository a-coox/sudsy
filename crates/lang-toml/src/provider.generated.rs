// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Mapping, Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("toml", tree_sitter_toml_ng::LANGUAGE), grammar: grammars::TOML, mapping: &MAPPING, extensions: &["toml"], file_names: &[], schema: option_schema(), formatter: &TomlFormatter, hooks: &hooks::TomlHooks, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: Some(validate::validate) })
}

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
        ("BareKey", "Key"),
        ("Boolean", "Literal"),
        ("DateTime", "Literal"),
        ("Float", "Literal"),
        ("Integer", "Literal"),
        ("QuotedKey", "Key"),
        ("String", "Literal"),
    ],
    kinds: &[
        ("table_array_element", "ArrayTable"),
        ("offset_date_time", "DateTime"),
        ("local_date_time", "DateTime"),
        ("local_date", "DateTime"),
        ("local_time", "DateTime"),
        ("dotted_key", "Key"),
        ("document", "Document"),
        ("table", "Table"),
        ("pair", "Pair"),
        ("array", "Array"),
        ("inline_table", "InlineTable"),
        ("comment", "Comment"),
        ("bare_key", "BareKey"),
        ("quoted_key", "QuotedKey"),
        ("string", "String"),
        ("integer", "Integer"),
        ("float", "Float"),
        ("boolean", "Boolean"),
        ("[", "LBracket"),
        ("]", "RBracket"),
        ("[[", "LBracket2"),
        ("]]", "RBracket2"),
        ("{", "LBrace"),
        ("}", "RBrace"),
        ("=", "Equals"),
        (".", "Dot"),
        (",", "Comma"),
    ],
};

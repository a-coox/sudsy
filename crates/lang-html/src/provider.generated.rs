// Generated from lang.yaml by `cargo xtask codegen`.
use core_syntax::grammars;
use core_treesitter::{Mapping, Spec, TreeSitterProvider};

core_syntax::include_generated!();

#[rustfmt::skip]
pub fn provider() -> TreeSitterProvider {
    TreeSitterProvider::new(Spec { crate_name: env!("CARGO_PKG_NAME"), language: || core_packs::language!("html", tree_sitter_html::LANGUAGE), grammar: grammars::HTML, mapping: &MAPPING, extensions: &["html", "htm"], file_names: &[], schema: option_schema(), formatter: &HtmlFormatter, hooks: &core_rules::DEFAULT_HOOKS, rules: rules::RULES, pattern_files: provider_api::pattern_files!(), families: &[], validate: None })
}

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[
        ("CData", "Chunk"),
        ("Entity", "Chunk"),
        ("RawText", "Chunk"),
        ("Text", "Chunk"),
    ],
    kinds: &[
        ("attribute_name", "AttrName"),
        ("attribute_value", "AttrValue"),
        ("quoted_attribute_value", "AttrValue"),
        ("cdata", "CData"),
        ("doctype", "Doctype"),
        ("doctype_body", "DoctypeBody"),
        ("\"doctype\"", "DoctypeKw"),
        ("element", "Element"),
        ("script_element", "Element"),
        ("style_element", "Element"),
        ("rcdata_element", "Element"),
        ("raw_text", "RawText"),
        ("start_tag", "StartTag"),
        ("self_closing_tag", "StartTag"),
        ("erroneous_end_tag", "StrayEndTag"),
        ("tag_name", "TagName"),
        ("erroneous_end_tag_name", "TagName"),
        ("document", "Document"),
        ("comment", "Comment"),
        ("attribute", "Attribute"),
        ("text", "Text"),
        ("entity", "Entity"),
        ("end_tag", "EndTag"),
        ("<!", "LtBang"),
        ("<", "Lt"),
        ("</", "LtSlash"),
        (">", "Gt"),
        ("/>", "SlashGt"),
        ("=", "Equals"),
    ],
};

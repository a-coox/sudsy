//! tree-sitter-ini node name -> `grammars/ini.grammar` kind. Names and values are tokens, so a
//! comment's `text` child stays inside the comment.

use core_treesitter::Mapping;

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[],
    kinds: &[
        ("document", "Document"),
        ("section", "Section"),
        ("section_name", "SectionHeader"),
        ("setting", "Setting"),
        ("comment", "Comment"),
        ("[", "LBracket"),
        ("]", "RBracket"),
        ("=", "Equals"),
        ("text", "Name"),
        ("setting_name", "Key"),
        ("setting_value", "Value"),
        ("continuation", "Continuation"),
    ],
};

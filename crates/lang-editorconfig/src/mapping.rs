//! tree-sitter-editorconfig node name -> `grammars/editorconfig.grammar` kind. Glob, key and value
//! are tokens: their inner nodes stay inside. Issue nodes are unmapped, so they are errors.

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
        ("section_header", "SectionHeader"),
        ("pair", "Pair"),
        ("comment", "Comment"),
        ("line_ending", "Newline"),
        ("section_open", "LBracket"),
        ("section_close", "RBracket"),
        ("assignment_operator", "Equals"),
        ("pattern", "Glob"),
        ("key", "Key"),
        ("value", "Value"),
    ],
};

//! tree-sitter-gitignore node name -> `grammars/gitignore.grammar` kind. The leaves of a pattern
//! are all `Pattern` tokens; blank lines, trailing spaces and carriage returns are blank text, so
//! they lex as whitespace. Issue leaves (`incomplete_escape`, `missing_set_close`, ...) are unmapped,
//! so a malformed pattern is a parse error.

use core_treesitter::Mapping;

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    wrap: &[],
    kinds: &[
        ("document", "Document"),
        ("pattern", "Entry"),
        ("blank_line", "Whitespace"),
        ("trailing_space", "Whitespace"),
        ("trailing_carriage_return", "Whitespace"),
        ("comment", "Comment"),
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
    ],
};

//! tree-sitter-toml-ng node name -> `grammars/toml.grammar` kind. Anonymous tokens are named by
//! their text. Strings and quoted keys are tokens: their content tokens stay inside.

use core_treesitter::Mapping;

pub const MAPPING: Mapping = Mapping {
    whitespace: "Whitespace",
    newline: "Newline",
    eof: "Eof",
    unknown: "Unknown",
    // Tree-sitter hides one-part keys and values as bare tokens: keys and list items are nodes.
    wrap: &[
        ("BareKey", "Key"),
        ("QuotedKey", "Key"),
        ("String", "Literal"),
        ("Integer", "Literal"),
        ("Float", "Literal"),
        ("Boolean", "Literal"),
        ("DateTime", "Literal"),
    ],
    kinds: &[
        ("document", "Document"),
        ("table", "Table"),
        ("table_array_element", "ArrayTable"),
        ("pair", "Pair"),
        ("dotted_key", "Key"),
        ("array", "Array"),
        ("inline_table", "InlineTable"),
        ("comment", "Comment"),
        ("[", "LBracket"),
        ("]", "RBracket"),
        ("[[", "LBracket2"),
        ("]]", "RBracket2"),
        ("{", "LBrace"),
        ("}", "RBrace"),
        ("=", "Equals"),
        (".", "Dot"),
        (",", "Comma"),
        ("bare_key", "BareKey"),
        ("quoted_key", "QuotedKey"),
        ("string", "String"),
        ("integer", "Integer"),
        ("float", "Float"),
        ("boolean", "Boolean"),
        ("offset_date_time", "DateTime"),
        ("local_date_time", "DateTime"),
        ("local_date", "DateTime"),
        ("local_time", "DateTime"),
    ],
};

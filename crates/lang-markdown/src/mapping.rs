//! tree-sitter-md node name -> `grammars/markdown.grammar` kind, for the block grammar and the
//! inline grammar parsed inside its `inline` and `pipe_table_cell` nodes. Unmapped leaves
//! (escapes, entities, link punctuation) are part of the surrounding `Text`.

use core_treesitter::{Composite, Injected, Mapping};

const fn mapping(
    kinds: &'static [(&'static str, &'static str)],
    wrap: &'static [(&'static str, &'static str)],
) -> Mapping {
    Mapping {
        whitespace: "Whitespace",
        newline: "Newline",
        eof: "Eof",
        unknown: "Unknown",
        wrap,
        kinds,
    }
}

pub static BLOCK: Mapping = mapping(
    &[
        ("document", "Document"),
        ("atx_heading", "Heading"),
        ("setext_heading", "Heading"),
        ("block_quote", "BlockQuote"),
        ("block_quote_marker", "QuoteMarker"),
        ("list", "List"),
        ("list_item", "ListItem"),
        ("list_marker_minus", "ListMarker"),
        ("list_marker_plus", "ListMarker"),
        ("list_marker_star", "ListMarker"),
        ("list_marker_dot", "ListMarker"),
        ("list_marker_parenthesis", "ListMarker"),
        ("task_list_marker_checked", "TaskMarker"),
        ("task_list_marker_unchecked", "TaskMarker"),
        ("fenced_code_block", "FencedCode"),
        ("fenced_code_block_delimiter", "Fence"),
        ("info_string", "Info"),
        ("code_fence_content", "CodeText"),
        ("indented_code_block", "IndentedCode"),
        ("html_block", "HtmlBlock"),
        ("thematic_break", "ThematicBreak"),
        ("link_reference_definition", "LinkDef"),
        ("minus_metadata", "FrontMatter"),
        ("plus_metadata", "FrontMatter"),
        ("pipe_table", "Table"),
        ("pipe_table_header", "TableRow"),
        ("pipe_table_row", "TableRow"),
        ("pipe_table_delimiter_row", "TableDelimiter"),
        ("pipe_table_cell", "Cell"),
        ("|", "Pipe"),
        ("inline", "Inline"),
        ("atx_h1_marker", "Marker"),
        ("atx_h2_marker", "Marker"),
        ("atx_h3_marker", "Marker"),
        ("atx_h4_marker", "Marker"),
        ("atx_h5_marker", "Marker"),
        ("atx_h6_marker", "Marker"),
        ("setext_h1_underline", "Marker"),
        ("setext_h2_underline", "Marker"),
        ("block_continuation", "Prefix"),
    ],
    &[("HtmlBlock", "Html")],
);

pub static INLINE: Mapping = mapping(
    &[
        ("emphasis", "Emphasis"),
        ("strong_emphasis", "Strong"),
        ("strikethrough", "Strikethrough"),
        ("emphasis_delimiter", "EmphasisDelim"),
        ("code_span", "CodeSpan"),
        ("html_tag", "HtmlInline"),
        ("hard_line_break", "HardBreak"),
        ("uri_autolink", "Autolink"),
        ("email_autolink", "Autolink"),
        ("inline_link", "Link"),
        ("full_reference_link", "Link"),
        ("collapsed_reference_link", "Link"),
        ("shortcut_link", "Link"),
        ("image", "Image"),
        ("link_destination", "LinkDest"),
        ("link_title", "LinkTitle"),
        ("link_label", "LinkLabel"),
    ],
    &[("CodeSpan", "Code"), ("HtmlInline", "Html")],
);

pub static COMPOSITE: Composite = Composite {
    text: "Text",
    injected: &[
        Injected {
            host: "inline",
            language: || tree_sitter_md::INLINE_LANGUAGE.into(),
            mapping: &INLINE,
        },
        Injected {
            host: "pipe_table_cell",
            language: || tree_sitter_md::INLINE_LANGUAGE.into(),
            mapping: &INLINE,
        },
    ],
    refine: &[
        ("html_block", "<!--", "HtmlComment"),
        ("html_tag", "<!--", "HtmlComment"),
    ],
};

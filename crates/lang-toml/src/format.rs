//! TOML formatting: the shared shapes with TOML's layout. A table is its header and then its pairs
//! one per line, without indentation (there are no braces); an inline table never breaks (TOML 1.0
//! forbids line breaks in it); arrays follow the list shape.

use crate::align::{self, AlignTable};
use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, Preset, ResolvedOptions, shared};
use core_format::shapes::{
    KeyValueStyle, ListLayout, ListStyle, TrailingComma, delimited_list, key_value,
    statement_sequence,
};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode};

pub const COLLAPSE: &str = "collapse";
pub const TRAILING_COMMAS: &str = "trailing_commas";
pub const MAX_BLANK_LINES: &str = "max_blank_lines";
pub const ALIGN_COMMENTS: &str = "align_comments";

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::line_width(80),
        shared::indent_size(2),
        shared::indent_style(),
        shared::final_newline(true),
        shared::end_of_line(),
        shared::bracket_spacing(true),
        shared::array_bracket_spacing(false),
        OptionDef::choice(
            COLLAPSE,
            &["preserve", "always", "never"],
            "preserve",
            "Arrays on one line: `preserve` keeps one that has a line break between its brackets \
             expanded and puts the others on one line if they fit; `always` puts every one that \
             fits on one line; `never` puts every non-empty one on several lines.",
        ),
        OptionDef::choice(
            TRAILING_COMMAS,
            &["never", "multiline"],
            "multiline",
            "Trailing comma after the last item of a multi-line array.",
        ),
        OptionDef::int(
            MAX_BLANK_LINES,
            1,
            (0, 10),
            "Largest number of blank lines kept between entries, tables and comments.",
        ),
        OptionDef::boolean(
            ALIGN_COMMENTS,
            false,
            "Start the trailing comments of consecutive entries in one column (entries with no              blank line or comment line between them); a run with an entry on several lines is not \n             aligned.",
        ),
    ])
    .with_presets(presets())
}

pub struct TomlFormatter;

fn array_style(options: &ResolvedOptions) -> ListStyle {
    ListStyle {
        padding: options.bool(shared::ARRAY_BRACKET_SPACING),
        trailing_comma: if options.str(TRAILING_COMMAS) == "multiline" {
            TrailingComma::WhenBroken
        } else {
            TrailingComma::Never
        },
        layout: match options.str(COLLAPSE) {
            "always" => ListLayout::Fit,
            "never" => ListLayout::Break,
            _ => ListLayout::PreserveBreak,
        },
        ..ListStyle::default()
    }
}

/// The text of the strings of a tree, with CRLF as LF.
fn strings(tree: &SyntaxNode) -> Vec<String> {
    tree.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::STRING)
        .map(|t| t.text().replace("\r\n", "\n"))
        .collect()
}

impl NodeFormatter for TomlFormatter {
    /// `end_of_line` rewrites the line breaks inside multi-line strings; nothing else may change.
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        if strings(before) == strings(after) {
            Ok(())
        } else {
            Err("a string changed".into())
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::PAIR => pair(cx, node)?,
            k::ARRAY => delimited_list(cx, node, &array_style(cx.options))?,
            k::INLINE_TABLE => delimited_list(
                cx,
                node,
                &ListStyle {
                    padding: cx.options.bool(shared::BRACKET_SPACING),
                    layout: ListLayout::Flat,
                    ..ListStyle::default()
                },
            )?,
            k::TABLE | k::ARRAY_TABLE => table(cx, node)?,
            k::DOCUMENT => document(cx, node)?,
            _ => return Ok(None),
        };
        Ok(Some(doc))
    }
}

/// `key = value`, with its trailing comment padded to the column of its run.
fn pair(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut d = key_value(
        cx,
        node,
        &KeyValueStyle {
            space_before_separator: true,
            ..KeyValueStyle::default()
        },
    )?;
    if cx.options.bool(ALIGN_COMMENTS) {
        let extra = cx.cached(AlignTable::default).extra(node);
        if extra > 0 {
            align::pad_comment(&mut d, extra);
        }
    }
    Ok(d)
}

fn max_blank_lines(cx: &FormatCtx<'_>) -> usize {
    cx.options.int(MAX_BLANK_LINES) as usize
}

/// `[header]` then its pairs, one per line.
fn table(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    for e in node.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if !cx.grammar.is_trivia(t.kind()) => parts.push(cx.token(&t)),
            SyntaxElement::Node(n) if n.kind() == k::KEY => parts.push(cx.format(&n)?),
            _ => {}
        }
    }
    let pairs: Vec<SyntaxNode> = node.children().filter(|n| n.kind() == k::PAIR).collect();
    if !pairs.is_empty() {
        parts.push(doc::hardline());
        parts.push(statement_sequence(cx, &pairs, max_blank_lines(cx))?);
    }
    Ok(doc::concat(parts))
}

/// The items, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    if cx.options.bool(ALIGN_COMMENTS) {
        cx.cached(|| AlignTable::build(cx, node));
    }
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence(cx, &items, max_blank_lines(cx))?];
    let eof = cx
        .last_token(node)
        .or_else(|| node.last_token())
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !items.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

/// The presets are plain config files in `presets/` next to the crate's manifest.
fn presets() -> Vec<Preset> {
    vec![
        Preset::new("toml/default", include_str!("../presets/default.yaml")),
        Preset::new(
            "toml/taplo-compat",
            include_str!("../presets/taplo-compat.yaml"),
        ),
    ]
}

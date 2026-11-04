//! TOML formatting: the shared shapes with TOML's layout. A table is its header and then its pairs
//! one per line, without indentation (there are no braces); an inline table never breaks (TOML 1.0
//! forbids line breaks in it); arrays follow the list shape.

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

impl NodeFormatter for TomlFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::PAIR => key_value(
                cx,
                node,
                &KeyValueStyle {
                    space_before_separator: true,
                    ..KeyValueStyle::default()
                },
            )?,
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
        parts.push(statement_sequence(cx, &pairs, 1)?);
    }
    Ok(doc::concat(parts))
}

/// The items, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence(cx, &items, 1)?];
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
    vec![Preset::new(
        "toml/default",
        include_str!("../presets/default.yaml"),
    )]
}

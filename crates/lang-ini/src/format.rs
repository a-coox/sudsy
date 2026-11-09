//! INI formatting: `[section]` headers, `key = value` settings one per line, a blank line before
//! each section. Names, keys, values and comments are kept as written.

use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, Preset, shared};
use core_format::shapes::statement_sequence_with;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::SyntaxNode;

pub const MAX_BLANK_LINES: &str = "max_blank_lines";
pub const SPACE_AROUND_EQUALS: &str = "space_around_equals";

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::final_newline(true),
        shared::end_of_line(),
        OptionDef::boolean(
            SPACE_AROUND_EQUALS,
            true,
            "Write `key = value` rather than `key=value`.",
        ),
        OptionDef::int(
            MAX_BLANK_LINES,
            1,
            (0, 10),
            "Largest number of blank lines kept between settings, sections and comments.",
        ),
    ])
    .with_presets(vec![Preset::new(
        "ini/default",
        include_str!("../presets/default.yaml"),
    )])
}

pub struct IniFormatter;

/// The values of a tree without the blanks around them (the formatter trims them).
fn values(tree: &SyntaxNode) -> Vec<String> {
    tree.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::VALUE)
        .map(|t| t.text().trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

impl NodeFormatter for IniFormatter {
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        if values(before) == values(after) {
            Ok(())
        } else {
            Err("a value changed".into())
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::SECTION => section(cx, node)?,
            k::SETTING => setting(cx, node),
            _ => return Ok(None),
        }))
    }
}

/// Every section starts after a blank line, the first one after the preamble only.
fn blank_before_section(_: &FormatCtx<'_>, _: &SyntaxNode, next: &SyntaxNode) -> bool {
    next.kind() == k::SECTION
}

fn max_blank_lines(cx: &FormatCtx<'_>) -> usize {
    cx.options.int(MAX_BLANK_LINES) as usize
}

fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence_with(
        cx,
        &items,
        max_blank_lines(cx),
        Some(blank_before_section),
        false,
    )?];
    // The comments at the end of the file belong to the end-of-file token.
    let eof = node
        .last_token()
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

/// `[name]` then its settings, one per line.
fn section(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let mut parts = Vec::new();
    let settings: Vec<SyntaxNode> = node.children().filter(|n| n.kind() == k::SETTING).collect();
    for n in node.children().filter(|n| n.kind() == k::SECTION_HEADER) {
        parts.push(cx.format(&n)?);
    }
    if !settings.is_empty() {
        parts.push(doc::hardline());
        parts.push(statement_sequence_with(
            cx,
            &settings,
            max_blank_lines(cx),
            None,
            false,
        )?);
    }
    Ok(doc::concat(parts))
}

/// `key = value`; a setting without a value is `key =`.
fn setting(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let space = cx.options.bool(SPACE_AROUND_EQUALS);
    let mut parts = Vec::new();
    for t in node
        .children_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| !cx.grammar.is_trivia(t.kind()))
    {
        if space && t.kind() != k::KEY {
            parts.push(doc::lit(" "));
        }
        // The parser's value token keeps the blanks around it.
        parts.push(if t.kind() == k::VALUE {
            doc::text(t.text().trim())
        } else {
            cx.token(&t)
        });
    }
    doc::concat(parts)
}

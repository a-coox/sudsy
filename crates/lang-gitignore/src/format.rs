//! Gitignore-syntax formatting: one pattern or comment per line, blank-line runs capped, trailing
//! spaces removed (the parser keeps an escaped one, `\ `, as part of the pattern).

use crate::generated::kind as k;
use core_config::{OptionDef, OptionSchema, Preset, shared};
use core_format::shapes::statement_sequence;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::SyntaxNode;

pub const MAX_BLANK_LINES: &str = "max_blank_lines";

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::final_newline(true),
        shared::end_of_line(),
        OptionDef::int(
            MAX_BLANK_LINES,
            1,
            (0, 10),
            "Largest number of blank lines kept between patterns and comments.",
        ),
    ])
    .with_presets(vec![Preset::new(
        "gitignore/default",
        include_str!("../presets/default.yaml"),
    )])
}

pub struct GitignoreFormatter;

impl NodeFormatter for GitignoreFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        Ok(Some(match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            // The parts are adjacent: the line is its tokens' text, after the comments in front.
            k::ENTRY => {
                let mut parts = Vec::new();
                for t in node.children_with_tokens().filter_map(|e| e.into_token()) {
                    // Verbatim: an escaped trailing space must survive the printer's trim. Git
                    // strips trailing spaces only, so blanks with a tab after a pattern stay.
                    if t.kind() == k::PATTERN {
                        if parts.is_empty() {
                            parts.push(cx.leading_comments_doc(&t));
                        }
                        parts.push(doc::verbatim(t.text()));
                    } else if t.kind() == k::WHITESPACE
                        && t.text().contains('\t')
                        && !parts.is_empty()
                    {
                        parts.push(doc::verbatim(t.text()));
                    }
                }
                doc::concat(parts)
            }
            _ => return Ok(None),
        }))
    }
}

/// The patterns, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let max = cx.options.int(MAX_BLANK_LINES) as usize;
    let mut parts = vec![statement_sequence(cx, &items, max)?];
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

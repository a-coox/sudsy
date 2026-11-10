//! Gitignore-syntax formatting: one pattern or comment per line, blank-line runs capped, trailing
//! spaces removed (the parser keeps an escaped one, `\ `, as part of the pattern).

use crate::generated::kind as k;
use core_config::{OptionSchema, Preset, shared};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, doc};
use core_syntax::SyntaxNode;
use core_syntax::SyntaxNodeExt;

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::final_newline(true),
        shared::end_of_line(),
        shared::max_blank_lines(
            1,
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
                for t in node.tokens() {
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
    core_format::shapes::document(cx, node, cx.options.max_blank_lines(), None)
}

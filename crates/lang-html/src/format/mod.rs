//! HTML formatting, Prettier's layout (printer-html): `model` builds the nodes with the whitespace
//! flags, `print` lays them out. The safety check compares what the formatter rewrites (tag names,
//! attribute values, the blanks of a text) through `value`.

mod model;
mod print;

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxElement, SyntaxNode};
use model::Mode;
use std::borrow::Cow;

include!("../schema.generated.rs");

pub struct HtmlFormatter;

/// Blanks collapsed.
fn norm(t: &str) -> String {
    t.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl NodeFormatter for HtmlFormatter {
    /// What the formatter rewrites: tag names and the doctype in lowercase, the quotes of an
    /// attribute value, the blanks of a text, the form of the tag's closing `>`.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        let SyntaxElement::Token(t) = item else {
            return Cow::Owned(item.to_string());
        };
        let text = t.text();
        match t.kind() {
            k::TAG_NAME | k::ATTR_NAME | k::DOCTYPE_KW => Cow::Owned(text.to_ascii_lowercase()),
            k::ATTR_VALUE => Cow::Owned(norm(
                &text
                    .trim_matches(['"', '\''])
                    .replace("&quot;", "\"")
                    .replace("&apos;", "'"),
            )),
            k::DOCTYPE_BODY => Cow::Owned(norm(&text.to_ascii_lowercase())),
            k::TEXT | k::RAW_TEXT => Cow::Owned(norm(text)),
            k::GT | k::SLASH_GT => Cow::Borrowed(""),
            _ => Cow::Borrowed(text),
        }
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        if node.kind() != k::DOCUMENT {
            return Ok(None);
        }
        let mode = match cx.options.str(WHITESPACE_SENSITIVITY) {
            "strict" => Mode::Strict,
            "ignore" => Mode::Ignore,
            _ => Mode::Css,
        };
        let tree = model::build(cx.grammar, node, &node.text().to_string(), mode);
        Ok(Some(print::block(
            &tree,
            print::Opts {
                tab: cx.options.int("indent_size") as usize,
                bracket_same_line: cx.options.bool(BRACKET_SAME_LINE),
                single_attribute_per_line: cx.options.bool(SINGLE_ATTRIBUTE_PER_LINE),
            },
        )))
    }
}

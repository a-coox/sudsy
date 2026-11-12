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

/// The value of a `style`, `srcset` or event-handler attribute.
fn embedded_attribute(value: &core_syntax::SyntaxToken) -> bool {
    let name = value.parent().and_then(|p| {
        p.children_with_tokens()
            .find_map(|e| e.into_token().filter(|t| t.kind() == k::ATTR_NAME))
    });
    name.is_some_and(|n| {
        let n = n.text().to_ascii_lowercase();
        matches!(n.as_str(), "style" | "srcset" | "allow") || (n.len() > 2 && n.starts_with("on"))
    })
}

/// The text of a `<script>` or `<style>`.
fn in_script_or_style(text: &core_syntax::SyntaxToken) -> bool {
    let element = text.parent().and_then(|c| c.parent());
    element
        .and_then(|e| e.first_child())
        .and_then(|start| {
            start
                .children_with_tokens()
                .find_map(|e| e.into_token().filter(|t| t.kind() == k::TAG_NAME))
        })
        .is_some_and(|n| matches!(n.text().to_ascii_lowercase().as_str(), "script" | "style"))
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
            // Code the formatter may rewrite is checked by its own language's formatter.
            k::ATTR_VALUE if embedded_attribute(t) => Cow::Borrowed(""),
            k::TEXT | k::RAW_TEXT if in_script_or_style(t) => Cow::Borrowed(""),
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
        let src = node.text().to_string();
        let tree = model::build(cx.grammar, node, &src, mode);
        let embed = |l: &str, m, c: &str, w, q| cx.embed(l, m, c, w, q);
        let auto = cx.options.str(EMBEDDED_LANGUAGE_FORMATTING) == "auto";
        Ok(Some(print::block(
            &tree,
            print::Opts {
                embed: auto.then_some(&embed),
                width: cx.options.int("line_width") as usize,
                tab: cx.options.int("indent_size") as usize,
                bracket_same_line: cx.options.bool(BRACKET_SAME_LINE),
                single_attribute_per_line: cx.options.bool(SINGLE_ATTRIBUTE_PER_LINE),
            },
        )))
    }
}

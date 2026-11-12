//! HTML formatting, Prettier's layout (printer-html): `model` builds the nodes with the whitespace
//! flags, `print` lays them out. The safety check compares what the formatter rewrites (tag names,
//! attribute values, the blanks of a text) through `value`.

mod model;
mod print;

use crate::generated::kind as k;
use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::{SyntaxElement, SyntaxNode};
use model::{Dialect, Mode};
use std::borrow::Cow;

include!("../schema.generated.rs");

/// Vue and Svelte: indent the content of `<script>` and `<style>`.
pub const INDENT_SCRIPT_AND_STYLE: &str = "indent_script_and_style";

pub struct HtmlFormatter;

/// The formatter of one markup language: HTML, Vue or Svelte.
pub struct MarkupFormatter {
    pub dialect: Dialect,
}

pub static SVELTE_FORMATTER: MarkupFormatter = MarkupFormatter {
    dialect: Dialect::Svelte,
};

pub static VUE_FORMATTER: MarkupFormatter = MarkupFormatter {
    dialect: Dialect::Vue,
};

impl NodeFormatter for HtmlFormatter {
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        MarkupFormatter {
            dialect: Dialect::Html,
        }
        .value(item)
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        MarkupFormatter {
            dialect: Dialect::Html,
        }
        .format_node(node, cx)
    }
}

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
        matches!(n.as_str(), "style" | "srcset" | "allow")
            || (n.len() > 2 && n.starts_with("on"))
            || n.starts_with([':', '@', '#'])
            || n.starts_with("v-")
    })
}

/// The text directly in a root block of a file (a Vue custom block may be formatted).
fn in_root_block(text: &core_syntax::SyntaxToken) -> bool {
    text.parent()
        .and_then(|c| c.parent())
        .and_then(|e| e.parent())
        .is_some_and(|d| d.kind() == k::DOCUMENT)
}

/// The expression of a `{{ }}`.
fn in_interpolation(text: &core_syntax::SyntaxToken) -> bool {
    text.parent()
        .and_then(|c| c.parent())
        .is_some_and(|i| i.kind() == k::INTERPOLATION)
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

impl NodeFormatter for MarkupFormatter {
    /// What the formatter rewrites: tag names and the doctype in lowercase, the quotes of an
    /// attribute value, the blanks of a text, the form of the tag's closing `>`.
    fn value<'a>(&self, item: &'a SyntaxElement) -> Cow<'a, str> {
        let SyntaxElement::Token(t) = item else {
            return Cow::Owned(item.to_string());
        };
        let text = t.text();
        match t.kind() {
            // Svelte's `{name}` is `name={name}`.
            k::ATTR_NAME => Cow::Owned(text.trim_matches(['{', '}']).to_ascii_lowercase()),
            k::TAG_NAME | k::DOCTYPE_KW => Cow::Owned(text.to_ascii_lowercase()),
            // Code the formatter may rewrite is checked by its own language's formatter.
            k::ATTR_VALUE if embedded_attribute(t) => Cow::Borrowed(""),
            k::MUSTACHE_TEXT | k::BLOCK_TAG_TEXT => Cow::Borrowed(""),
            k::TEXT | k::RAW_TEXT
                if in_script_or_style(t)
                    || in_interpolation(t)
                    || (self.dialect == Dialect::Vue && in_root_block(t)) =>
            {
                Cow::Borrowed("")
            }
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

    /// Svelte: `name={name}` is `{name}` (`bind:name={name}` is `bind:name`) and `name="{expr}"`
    /// is `name={expr}`.
    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        _grammar: &core_syntax::Grammar,
        _options: &core_config::ResolvedOptions,
    ) -> Option<String> {
        if self.dialect != Dialect::Svelte {
            return None;
        }
        let mut edits: Vec<(std::ops::Range<usize>, String)> = Vec::new();
        for a in tree.descendants().filter(|n| n.kind() == k::ATTRIBUTE) {
            let r = a.text_range();
            // The node carries the blanks around it as trivia.
            let whole = &text[usize::from(r.start())..usize::from(r.end())];
            let start = usize::from(r.start()) + (whole.len() - whole.trim_start().len());
            let end = usize::from(r.start()) + whole.trim_end().len();
            let Some(eq) = a
                .children_with_tokens()
                .filter_map(|e| e.into_token())
                .find(|t| t.kind() == k::EQUALS)
            else {
                continue;
            };
            let eq_at = usize::from(eq.text_range().start());
            let name = text[start..eq_at].trim_end();
            let value = a
                .children_with_tokens()
                .filter_map(|v| match v {
                    SyntaxElement::Node(n) if n.kind() == k::MUSTACHE => {
                        let r = n.text_range();
                        Some((usize::from(r.start())..usize::from(r.end()), false))
                    }
                    SyntaxElement::Token(t) if t.kind() == k::ATTR_VALUE => {
                        let r = t.text_range();
                        Some((usize::from(r.start())..usize::from(r.end()), true))
                    }
                    _ => None,
                })
                .last();
            let Some((range, quoted)) = value else {
                continue;
            };
            let raw = text[range.clone()].trim();
            let braces = if quoted {
                // `"{expr}"`: one mustache and nothing else.
                raw.get(1..raw.len().saturating_sub(1))
                    .filter(|v| raw.len() > 1 && v.starts_with('{') && v.ends_with('}'))
                    .filter(|v| v[1..v.len() - 1].find(['{', '}']).is_none())
            } else {
                Some(raw)
            };
            let Some(braces) = braces else { continue };
            let inner = braces[1..braces.len() - 1].trim();
            let last = name.rsplit(':').next().unwrap_or(name);
            if inner == last {
                let short = if name.contains(':') {
                    name.to_string()
                } else {
                    format!("{{{name}}}")
                };
                edits.push((start..end, short));
            } else if quoted {
                edits.push((range, braces.to_string()));
            }
        }
        edits.sort_by_key(|e| e.0.start);
        // `text[range]` with the edits inside it applied.
        let piece = |range: std::ops::Range<usize>| {
            let mut out = text[range.clone()].to_string();
            for (r, with) in edits.iter().rev() {
                if r.start >= range.start && r.end <= range.end {
                    out.replace_range(r.start - range.start..r.end - range.start, with);
                }
            }
            out
        };
        // prettier-plugin-svelte's default order of the root: `<svelte:options>`, scripts, markup,
        // styles. Blocks apart in the source are joined by a line break, different parts by a blank line.
        let root: Vec<(u8, std::ops::Range<usize>)> = tree
            .children()
            .map(|c| {
                let r = c.text_range();
                let key = c
                    .children()
                    .find(|s| s.kind() == k::START_TAG)
                    .and_then(|t| {
                        t.children_with_tokens()
                            .filter_map(|e| e.into_token())
                            .find(|t| t.kind() == k::TAG_NAME)
                    })
                    .map_or(2, |n| match n.text() {
                        "svelte:options" => 0,
                        "script" => 1,
                        "style" => 3,
                        _ => 2,
                    });
                (key, usize::from(r.start())..usize::from(r.end()))
            })
            .collect();
        if root.windows(2).any(|w| w[0].0 > w[1].0) {
            let mut order: Vec<usize> = (0..root.len()).collect();
            order.sort_by_key(|&i| root[i].0);
            let mut out = String::new();
            for (n, &i) in order.iter().enumerate() {
                if n > 0 {
                    let prev = order[n - 1];
                    if i == prev + 1 {
                        out += &text[root[prev].1.end..root[i].1.start];
                    } else {
                        out += if root[i].0 == root[prev].0 {
                            "\n"
                        } else {
                            "\n\n"
                        };
                    }
                }
                out += piece(root[i].1.clone()).trim();
            }
            return Some(out + "\n");
        }
        if edits.is_empty() {
            return None;
        }
        Some(piece(0..text.len()))
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
        let tree = model::build(cx.grammar, node, &src, mode, self.dialect);
        let embed = |l: &str, m, c: &str, w, q| cx.embed(l, m, c, w, q);
        let auto = cx.options.str(EMBEDDED_LANGUAGE_FORMATTING) == "auto";
        Ok(Some(print::block(
            &tree,
            print::Opts {
                dialect: self.dialect,
                indent_blocks: cx
                    .options
                    .get(INDENT_SCRIPT_AND_STYLE)
                    .is_some_and(|v| *v == core_config::OptionValue::Bool(true)),
                embed: auto.then_some(&embed),
                width: cx.options.int("line_width") as usize,
                tab: cx.options.int("indent_size") as usize,
                bracket_same_line: cx.options.bool(BRACKET_SAME_LINE),
                single_attribute_per_line: cx.options.bool(SINGLE_ATTRIBUTE_PER_LINE),
            },
        )))
    }
}

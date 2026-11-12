//! What the Rust rules read of a start tag: its lowercase name and its attributes.

use crate::generated::kind as k;
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange, TextSize};

pub struct Attr<'a> {
    /// Lowercase.
    pub name: String,
    pub key: TextRange,
    /// The value as written, without its quotes; `None` for an attribute without `=`.
    pub value: Option<&'a str>,
    pub value_range: Option<TextRange>,
}

/// The lowercase name of a `StartTag` and its range.
pub fn tag_name(tag: &SyntaxNode, text: &str) -> Option<(String, TextRange)> {
    let t = tag.token(k::TAG_NAME)?;
    Some((text[t.text_range()].to_ascii_lowercase(), t.text_range()))
}

pub fn attrs<'a>(tag: &SyntaxNode, text: &'a str) -> Vec<Attr<'a>> {
    let mut out = Vec::new();
    for a in tag.children().filter(|c| c.kind() == k::ATTRIBUTE) {
        let Some(key) = a.token(k::ATTR_NAME) else {
            continue;
        };
        let value = a.token(k::ATTR_VALUE).map(|v| {
            let range = v.text_range();
            let raw = &text[range];
            let quoted = raw.len() >= 2
                && (raw.starts_with('"') && raw.ends_with('"')
                    || raw.starts_with('\'') && raw.ends_with('\''));
            if quoted {
                let inner = TextRange::new(
                    range.start() + TextSize::from(1),
                    range.end() - TextSize::from(1),
                );
                (&text[inner], inner)
            } else {
                (raw, range)
            }
        });
        out.push(Attr {
            name: text[key.text_range()].to_ascii_lowercase(),
            key: key.text_range(),
            value: value.map(|v| v.0),
            value_range: value.map(|v| v.1),
        });
    }
    out
}

/// The `StartTag` of every element and its content, in document order.
pub fn start_tags(root: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    root.descendants().filter(|n| n.kind() == k::START_TAG)
}

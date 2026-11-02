use crate::FormatCtx;
use crate::directives::Directive;
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxElement, SyntaxNode};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct KeyValueStyle {
    pub space_before_separator: bool,
    pub space_after_separator: bool,
}

impl Default for KeyValueStyle {
    fn default() -> Self {
        KeyValueStyle {
            space_before_separator: false,
            space_after_separator: true,
        }
    }
}

fn significant<'a>(
    cx: &'a FormatCtx<'_>,
    node: &SyntaxNode,
) -> impl Iterator<Item = SyntaxElement> + use<'a> {
    node.children_with_tokens()
        .filter(|e| e.as_token().is_none_or(|t| !cx.grammar.is_trivia(t.kind())))
}

fn element_doc(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Result<Doc, FormatError> {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) => Ok(cx.token(t)),
    }
}

/// Source width of a key, without its comments.
fn key_width(cx: &FormatCtx<'_>, node: &SyntaxNode) -> usize {
    let key = significant(cx, node).next();
    let text = match key {
        Some(SyntaxElement::Token(t)) => t.text().to_string(),
        Some(SyntaxElement::Node(n)) => cx
            .first_token(&n)
            .zip(cx.last_token(&n))
            .map(|(a, b)| {
                let range =
                    core_syntax::TextRange::new(a.text_range().start(), b.text_range().end());
                n.ancestors()
                    .last()
                    .map(|r| r.text().slice(range - r.text_range().start()).to_string())
                    .unwrap_or_default()
            })
            .unwrap_or_default(),
        None => String::new(),
    };
    text.width()
}

/// `key: value`. With the `Align` directive on the parent node, the separators of sibling
/// key-values line up (when the parent is laid out on several lines).
pub fn key_value(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    style: &KeyValueStyle,
) -> Result<Doc, FormatError> {
    let name = cx.grammar.name(node.kind());
    let mut parts = significant(cx, node);
    let (Some(key), Some(SyntaxElement::Token(sep))) = (parts.next(), parts.next()) else {
        return Err(FormatError::Shape(format!(
            "{name}: expected key, separator token and value"
        )));
    };
    let mut value = parts.peekable();
    if value.peek().is_none() {
        return Err(FormatError::Shape(format!("{name}: missing value")));
    }
    let mut out = vec![element_doc(cx, &key)?];
    if let Some(parent) = node.parent()
        && cx.directives_of(&parent).has(&Directive::Align)
    {
        let tag = |n: &SyntaxNode| {
            cx.grammar
                .def(n.kind())
                .is_some_and(|d| d.has_tag("key-value"))
        };
        let widest = parent
            .children()
            .filter(tag)
            .map(|n| key_width(cx, &n))
            .max()
            .unwrap_or(0);
        let pad = widest - key_width(cx, node);
        if pad > 0 {
            out.push(doc::if_break(doc::text(" ".repeat(pad)), Doc::Nil));
        }
    }
    if style.space_before_separator {
        out.push(doc::lit(" "));
    }
    out.push(cx.token(&sep));
    if style.space_after_separator {
        out.push(doc::lit(" "));
    }
    for v in value {
        out.push(element_doc(cx, &v)?);
    }
    Ok(doc::concat(out))
}

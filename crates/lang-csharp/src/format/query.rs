//! LINQ query expressions: `from x in xs where ... select ...` on one line if it fits, else one
//! clause per line below the `from`.

use super::seq::{R, element_doc, seq, sig};
use core_format::{FormatCtx, doc};
use core_syntax::{SyntaxElement, SyntaxNode};

pub fn query_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [SyntaxElement::Node(from), SyntaxElement::Node(body)] = els.as_slice() else {
        return seq(cx, node);
    };
    Ok(doc::group(doc::concat(vec![
        cx.format(from)?,
        doc::indent(doc::concat(vec![doc::line(), cx.format(body)?])),
    ])))
}

/// The clauses, each after a `line` of the enclosing query group.
pub fn query_body(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut parts = Vec::new();
    for (i, e) in sig(cx, node).iter().enumerate() {
        if i > 0 {
            parts.push(doc::line());
        }
        parts.push(element_doc(cx, e)?);
    }
    Ok(doc::concat(parts))
}

/// `into x`, then the rest of the query.
pub fn query_continuation(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Token(into),
        SyntaxElement::Token(name),
        SyntaxElement::Node(body),
    ] = els.as_slice()
    else {
        return seq(cx, node);
    };
    Ok(doc::concat(vec![
        cx.token(into),
        doc::lit(" "),
        cx.token(name),
        doc::line(),
        cx.format(body)?,
    ]))
}

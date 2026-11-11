//! TypeScript: types, interfaces, type aliases, enums, namespaces, `as`, `satisfies`, `!`.

mod decls;
mod heritage;
mod parens;
mod types;

pub(crate) use heritage::class_header;
pub(crate) use parens::redundant as redundant_type_tokens;

use super::util::{R, elem, is_tok, kids};
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, lit};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match types::format(node, cx)? {
        Some(doc) => Ok(Some(doc)),
        None => decls::format(node, cx),
    }
}

/// Whether no space goes between `a` and `b`: before closers, dots, `?`, `!` and `:`, after
/// openers, and before the parts of a signature that follow its name.
fn tight(a: &SyntaxElement, b: &SyntaxElement) -> bool {
    let before = match b {
        SyntaxElement::Token(t) => {
            matches!(t.text(), "," | ")" | "]" | "." | "?" | "!" | ":" | ";")
        }
        SyntaxElement::Node(n) => match n.kind() {
            k::TS_TYPE_ANNOTATION => true,
            k::TS_TYPE_PARAMETER_DECLARATION
            | k::TS_TYPE_PARAMETER_INSTANTIATION
            | k::FORMAL_PARAMETERS => !is_tok(a, "new"),
            _ => false,
        },
    };
    before || matches!(a, SyntaxElement::Token(t) if matches!(t.text(), "(" | "[" | "." | "..."))
}

/// The children on one line, a space between neighbours (none where [`tight`] says so when
/// `tight_signs`); `;` and `,` tokens are left out.
pub(super) fn words(cx: &FormatCtx<'_>, elems: &[SyntaxElement], tight_signs: bool) -> R {
    let mut parts = Vec::with_capacity(elems.len() * 2);
    let mut prev: Option<&SyntaxElement> = None;
    for e in elems {
        // The separator is the list's to print; its comments stay.
        if let SyntaxElement::Token(t) = e
            && matches!(t.text(), ";" | ",")
        {
            parts.extend([cx.leading_comments_doc(t), cx.trailing_comments_doc(t)]);
            continue;
        }
        if let Some(p) = prev
            && !(tight_signs && tight(p, e))
        {
            parts.push(lit(" "));
        }
        parts.push(elem(cx, e)?);
        prev = Some(e);
    }
    Ok(concat(parts))
}

/// The children with nothing between them.
pub(super) fn glued(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let parts = kids(cx, node)
        .iter()
        .map(|e| elem(cx, e))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(concat(parts))
}

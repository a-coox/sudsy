//! A class header with several heritage clauses (`class A<T> extends B implements C, D {`), which
//! goes on lines of its own, indented, when it is too long, and then puts `{` on a line of its
//! own. A lone `extends` or `implements` with a plain name stays on the line.

use super::types::type_params;
use crate::format::util::{elem, inline, is_tok, node_of, shape_error};
use crate::generated::kind as k;
use core_format::doc::{
    Doc, concat, group, group_with_id, hardline, if_break_in, indent, line, lit, new_group_id,
};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode};

/// A name, or a name with type arguments (which break instead of the line).
fn plain(n: &SyntaxNode) -> bool {
    n.kind() == k::IDENTIFIER_REFERENCE
        || n.children().any(|c| {
            matches!(
                c.kind(),
                k::TS_TYPE_PARAMETER_INSTANTIATION | k::IDENTIFIER_REFERENCE
            )
        })
}

/// The header and body of a class (`elems` are its children after the decorators), or `None`
/// when its heritage is at most one plain name and stays on the line.
pub(crate) fn class_header(
    cx: &FormatCtx<'_>,
    elems: &[SyntaxElement],
    body: &Doc,
    body_empty: bool,
) -> Result<Option<Doc>, FormatError> {
    let imp = elems
        .iter()
        .position(|e| matches!(e, SyntaxElement::Token(t) if t.text() == "implements"));
    let ext = elems.iter().position(|e| is_tok(e, "extends"));
    let Some(last) = elems.len().checked_sub(1) else {
        return Ok(None);
    };
    let implements: Vec<&SyntaxNode> = imp
        .map(|at| {
            elems[at + 1..last]
                .iter()
                .filter_map(node_of)
                .filter(|n| n.kind() == k::TS_CLASS_IMPLEMENTS)
                .collect()
        })
        .unwrap_or_default();
    let superclass = ext.and_then(|at| elems.get(at + 1)).and_then(node_of);
    let super_args = ext
        .and_then(|at| elems.get(at + 2))
        .and_then(node_of)
        .is_some_and(|n| n.kind() == k::TS_TYPE_PARAMETER_INSTANTIATION);
    let simple = ext.is_none() && implements.len() == 1 && plain(implements[0])
        || imp.is_none()
            && (super_args || superclass.is_none_or(|s| s.kind() == k::IDENTIFIER_REFERENCE));
    if simple {
        return Ok(None);
    }
    let heritage_start = ext.or(imp).unwrap_or(last);
    // The name and type parameters; the type parameters' group tells where the clauses go.
    let params = elems[..heritage_start]
        .iter()
        .position(|e| node_of(e).is_some_and(|n| n.kind() == k::TS_TYPE_PARAMETER_DECLARATION));
    let id = new_group_id();
    let head = match params {
        Some(at) => {
            let tp = node_of(&elems[at]).ok_or_else(|| shape_error("class"))?;
            concat(vec![
                inline(cx, &elems[..at])?,
                type_params(tp, cx, Some(id))?,
                inline(cx, &elems[at + 1..heritage_start])?,
            ])
        }
        None => inline(cx, &elems[..heritage_start])?,
    };
    let multiple = usize::from(ext.is_some()) + implements.len() > 1;
    let mut clauses = Vec::new();
    if let Some(at) = ext {
        clauses.extend([line(), inline(cx, &elems[at..imp.unwrap_or(last)])?]);
    }
    if let Some(at) = imp {
        // The comments of a comma stay after it.
        let mut items: Vec<(Doc, Doc)> = Vec::new();
        for e in &elems[at + 1..last] {
            match e {
                SyntaxElement::Node(n) if n.kind() == k::TS_CLASS_IMPLEMENTS => {
                    items.push((cx.format(n)?, Doc::Nil));
                }
                SyntaxElement::Token(t) if t.text() == "," => {
                    if let Some(item) = items.last_mut() {
                        item.1 = concat(vec![
                            cx.leading_comments_doc(t),
                            cx.trailing_comments_doc(t),
                        ]);
                    }
                }
                _ => {}
            }
        }
        let count = items.len();
        let mut list = Vec::new();
        for (i, (d, c)) in items.into_iter().enumerate() {
            list.push(d);
            if i + 1 < count {
                list.extend([lit(","), c, line()]);
            } else {
                list.push(c);
            }
        }
        let sep = match params {
            // With type parameters only, `implements` stays after a broken parameter list.
            Some(_) if !multiple => if_break_in(id, lit(" "), line()),
            _ => line(),
        };
        clauses.extend([
            sep,
            elem(cx, &elems[at])?,
            group(indent(concat(vec![line(), concat(list)]))),
        ]);
    }
    let heritage = concat(clauses);
    let header = if params.is_some() {
        concat(vec![head, indent(heritage)])
    } else {
        indent(concat(vec![head, heritage]))
    };
    let group_id = new_group_id();
    let brace = if body_empty {
        lit(" ")
    } else {
        if_break_in(group_id, hardline(), lit(" "))
    };
    Ok(Some(concat(vec![
        group_with_id(group_id, header),
        brace,
        body.clone(),
    ])))
}

//! Arrow functions: parameters, the body on the arrow's line or the next, chains
//! (`a => b => c`), and the forms used when the function is hugged as a call argument.

use super::ast::{has_comment, is_binaryish, outer, parent, starts_with, un};
use super::util::{R, kids, node_of, remove_lines, shape_error, token_of, will_break};
use crate::generated::kind as k;
use core_format::doc::{
    Doc, concat, group, group_broken, if_break, if_break_in, indent, indent_if_break, join, line,
    lit, new_group_id, softline,
};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

/// The layouts of one function the callers ask for (bits of the `layout` cache flags).
pub(crate) struct Flags;

impl Flags {
    pub const EXPAND_LAST: u32 = 1;
    pub const EXPAND_FIRST: u32 = 2;
    /// The parameters of a test call's function stay on one line.
    pub const TEST: u32 = 4;
    /// The right side of an assignment.
    pub const RHS: u32 = 8;
    /// The tail of an assignment chain whose value is an arrow chain.
    pub const CHAIN_TAIL: u32 = 16;
    pub const EXPAND: u32 = Self::EXPAND_LAST | Self::EXPAND_FIRST;
}

const BAILOUT: &str = "arguments cannot expand";

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::ARROW_FUNCTION_EXPRESSION {
        return Ok(None);
    }
    print_fn(cx, node, 0)
}

/// The number of parameters of a function.
pub(crate) fn param_count(f: &SyntaxNode) -> usize {
    f.children()
        .find(|c| c.kind() == k::FORMAL_PARAMETERS)
        .map_or(0, |p| {
            p.children()
                .filter(|c| matches!(c.kind(), k::FORMAL_PARAMETER | k::FORMAL_PARAMETER_REST))
                .count()
        })
}

/// A function or arrow function in the layout `flags` asks for; `None` when a hugged
/// function's parameters would have to break.
pub(crate) fn print_fn(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    flags: u32,
) -> Result<Option<Doc>, FormatError> {
    let made = cx.layout(node, flags, || match node.kind() {
        k::FUNCTION => super::decl::function(node, cx, flags)?.ok_or_else(|| shape_error(BAILOUT)),
        _ => arrow(node, cx, flags),
    });
    match made {
        Ok(d) => Ok(Some(d)),
        Err(FormatError::Shape(m)) if m == BAILOUT => Ok(None),
        Err(e) => Err(e),
    }
}

/// `async <T>(a, b): R` of one arrow function.
fn signature(cx: &FormatCtx<'_>, node: &SyntaxNode, flags: u32) -> R {
    let elems = kids(cx, node);
    let mut parts = Vec::new();
    let mut inner = Vec::new();
    let mut arrow = lit(" =>");
    for e in &elems {
        match e {
            e if token_of(e).is_some_and(|t| t.text() == "async") => {
                parts.push(super::util::elem(cx, e)?);
                parts.push(lit(" "));
            }
            e if token_of(e).is_some_and(|t| t.kind() == k::ARROW) => {
                if let Some(t) = token_of(e) {
                    arrow = concat(vec![lit(" "), cx.token(t)]);
                }
                break;
            }
            e if node_of(e).is_some_and(|n| n.kind() == k::FORMAL_PARAMETERS) => {
                let fp = node_of(e).ok_or_else(|| shape_error("arrow"))?;
                let elems = kids(cx, fp);
                // `arrow_parens: avoid` leaves a lone name without parentheses.
                let mut params = match elems.first() {
                    Some(e) if token_of(e).is_none() => super::util::elem(cx, e)?,
                    // Grouped with the return type below, so both break together.
                    _ => super::decl::parameters_in(cx, &elems, Some(false))?,
                };
                if flags & (Flags::EXPAND | Flags::TEST) != 0 {
                    if flags & Flags::EXPAND != 0 && will_break(&params) {
                        return Err(shape_error(BAILOUT));
                    }
                    params = remove_lines(&params);
                }
                inner.push(params);
            }
            e => inner.push(super::util::elem(cx, e)?),
        }
    }
    parts.push(group(concat(inner)));
    parts.push(arrow);
    Ok(concat(parts))
}

fn arrow(node: &SyntaxNode, cx: &FormatCtx<'_>, flags: u32) -> R {
    let expand_last = flags & Flags::EXPAND_LAST != 0;
    let outer_node = outer(node);
    let p = parent(node);
    let is_callee = p.as_ref().is_some_and(|p| {
        matches!(p.kind(), k::CALL_EXPRESSION | k::NEW_EXPRESSION)
            && p.first_child().as_ref() == Some(&outer_node)
    });
    let mut sigs = Vec::new();
    let mut break_chain = false;
    let mut cur = node.clone();
    let (tail, body_doc) = loop {
        sigs.push(signature(cx, &cur, flags)?);
        let params_not_plain = cur
            .children()
            .find(|c| c.kind() == k::FORMAL_PARAMETERS)
            .is_some_and(|fp| {
                fp.children().any(|c| {
                    c.kind() != k::FORMAL_PARAMETER
                        || c.children().count() != 1
                        || c.first_child()
                            .is_none_or(|i| i.kind() != k::BINDING_IDENTIFIER)
                })
            });
        let has_return_type = cur.children().any(|c| c.kind() == k::TS_TYPE_ANNOTATION);
        let has_type_params = cur
            .children()
            .any(|c| c.kind() == k::TS_TYPE_PARAMETER_DECLARATION);
        break_chain |= (has_return_type && super::arrow::param_count(&cur) > 0)
            || has_type_params
            || params_not_plain;
        let body = cur
            .last_child()
            .ok_or_else(|| shape_error("arrow without body"))?;
        if !expand_last && un(&body).kind() == k::ARROW_FUNCTION_EXPRESSION && body == un(&body) {
            cur = body;
        } else {
            break (cur.clone(), cx.format(&body)?);
        }
    };
    let body = tail
        .last_child()
        .ok_or_else(|| shape_error("arrow without body"))?;
    let body_u = un(&body);
    let is_chain = sigs.len() > 1;
    let comments = has_comment(cx, &body);
    let same_line = !super::ast::has_leading_comment(cx, &body)
        && (matches!(
            body_u.kind(),
            k::ARRAY_EXPRESSION
                | k::OBJECT_EXPRESSION
                | k::FUNCTION_BODY
                | k::JSX_ELEMENT
                | k::JSX_FRAGMENT
                | k::ARROW_FUNCTION_EXPRESSION
        ) || super::call::template_on_own_line(cx, &body_u));
    let jsx_container = p
        .as_ref()
        .is_some_and(|p| p.kind() == k::JSX_EXPRESSION_CONTAINER);
    let soft_line = (expand_last || jsx_container) && !comments;
    let trailing_comma = if expand_last {
        super::util::trailing_comma(cx, true)
    } else {
        Doc::Nil
    };
    let body_part = if body_u.kind() == k::SEQUENCE_EXPRESSION {
        concat(vec![
            lit(" ("),
            indent(concat(vec![softline(), body_doc])),
            softline(),
            lit(")"),
        ])
    } else if same_line {
        concat(vec![lit(" "), body_doc])
    } else {
        let parens = body_u.kind() == k::CONDITIONAL_EXPRESSION
            && !starts_with(&body_u, &|n| n.kind() == k::OBJECT_EXPRESSION);
        concat(vec![
            indent(concat(vec![
                line(),
                if parens {
                    if_break(Doc::Nil, lit("("))
                } else {
                    Doc::Nil
                },
                body_doc,
                if parens {
                    if_break(Doc::Nil, lit(")"))
                } else {
                    Doc::Nil
                },
            ])),
            if soft_line {
                concat(vec![trailing_comma, softline()])
            } else {
                Doc::Nil
            },
        ])
    };
    if !is_chain {
        // The body breaks on its own: `) => body` stays on the line when only the parameters broke.
        let own_line = body_u.kind() != k::SEQUENCE_EXPRESSION && !same_line;
        let body_part = if own_line {
            group(body_part)
        } else {
            body_part
        };
        return Ok(group(concat(vec![group(sigs.remove(0)), body_part])));
    }
    let assignment = flags & Flags::RHS != 0;
    let chain_tail = flags & Flags::CHAIN_TAIL != 0;
    let signatures = signatures_doc(cx, node, flags, sigs, break_chain, is_callee);
    let (indent_sigs, break_sigs) = if is_callee || assignment {
        (true, chain_tail || (is_callee && !same_line))
    } else {
        (false, false)
    };
    let id = new_group_id();
    let sig_group = {
        let inner = if indent_sigs {
            indent(concat(vec![softline(), signatures]))
        } else {
            signatures
        };
        Doc::Group(Box::new(core_format::doc::Group {
            contents: inner,
            states: Vec::new(),
            broken: break_sigs,
            id: Some(id),
        }))
    };
    Ok(group(concat(vec![
        sig_group,
        indent_if_break(id, body_part),
        if is_callee {
            if_break_in(id, softline(), Doc::Nil)
        } else {
            Doc::Nil
        },
    ])))
}

fn signatures_doc(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    flags: u32,
    sigs: Vec<Doc>,
    should_break: bool,
    is_callee: bool,
) -> Doc {
    let _ = cx;
    let p = parent(node);
    let call_like = |n: &SyntaxNode| {
        matches!(
            n.kind(),
            k::CALL_EXPRESSION | k::NEW_EXPRESSION | k::IMPORT_EXPRESSION
        )
    };
    let arrow_sep = line();
    let wrap = |d: Doc| {
        if should_break {
            group_broken(d)
        } else {
            group(d)
        }
    };
    let p_kind_call = p.as_ref().is_some_and(call_like);
    if (!is_callee && p_kind_call) || p.as_ref().is_some_and(|p| is_binaryish(p.kind())) {
        let mut it = sigs.into_iter();
        let head = it.next().unwrap_or(Doc::Nil);
        return wrap(concat(vec![
            head,
            indent(concat(vec![line(), join(&arrow_sep, it)])),
        ]));
    }
    if is_callee || flags & Flags::RHS != 0 {
        return wrap(join(&arrow_sep, sigs));
    }
    wrap(indent(join(&arrow_sep, sigs)))
}

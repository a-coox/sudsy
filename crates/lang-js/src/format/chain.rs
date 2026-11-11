//! Member expressions and member chains (`a.b().c().d()`): the chain is cut into groups, printed
//! on one line when it fits and has few calls, one group per line otherwise (Prettier's
//! `printMemberChain`).

use super::ast::{Call, is_literal, is_member, name_of, parent, token_comments, un};
use super::call::{arguments, is_simple_call_argument, tab_width};
use super::parens::{Need, needs_parens};
use super::util::{R, kids, shape_error, token_of, will_break};
use crate::generated::kind as k;
use core_format::doc::{
    Doc, break_parent, concat, conditional_group, group, hardline, indent, label, lit, softline,
};
use core_format::{FormatCtx, FormatError, comments};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match node.kind() {
        k::CHAIN_EXPRESSION => match node.first_child() {
            Some(inner) => Ok(Some(cx.format(&inner)?)),
            None => Err(shape_error("chain")),
        },
        k::STATIC_MEMBER_EXPRESSION
        | k::COMPUTED_MEMBER_EXPRESSION
        | k::PRIVATE_FIELD_EXPRESSION => Ok(Some(member(node, cx)?)),
        _ => Ok(None),
    }
}

/// The `.name`, `?.name` or `[expr]` of a member expression.
fn lookup(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let mut parts = Vec::new();
    let mut computed = None;
    for e in elems.iter().skip(1) {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::L_BRACKET => parts.push(cx.token(t)),
            SyntaxElement::Token(t) if t.kind() == k::R_BRACKET => {
                let inner = parts.pop();
                let open = parts.pop();
                let (Some(open), Some(inner)) = (open, inner) else {
                    return Err(shape_error("member"));
                };
                let numeric = computed == Some(k::NUMERIC_LITERAL);
                parts.push(if numeric {
                    concat(vec![open, inner, cx.token(t)])
                } else {
                    group(concat(vec![
                        open,
                        indent(concat(vec![softline(), inner])),
                        softline(),
                        cx.token(t),
                    ]))
                });
            }
            SyntaxElement::Token(t) => parts.push(cx.token(t)),
            SyntaxElement::Node(n) => {
                computed = Some(n.kind());
                parts.push(cx.format(n)?);
            }
        }
    }
    Ok(concat(parts))
}

fn member(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let object = node.first_child().ok_or_else(|| shape_error("member"))?;
    let object_doc = cx.format(&object)?;
    let lookup = lookup(node, cx)?;
    let first_non_member = std::iter::successors(parent(node), |p| {
        matches!(
            p.kind(),
            k::STATIC_MEMBER_EXPRESSION
                | k::COMPUTED_MEMBER_EXPRESSION
                | k::PRIVATE_FIELD_EXPRESSION
                | k::TS_NON_NULL_EXPRESSION
        )
        .then(|| parent(p))
        .flatten()
    })
    .last();
    let simple_names = un(&object).kind() == k::IDENTIFIER_REFERENCE
        && node
            .last_child()
            .is_some_and(|p| p.kind() == k::IDENTIFIER_NAME)
        && parent(node).is_none_or(|p| !is_member(p.kind()));
    let inline = first_non_member.is_some_and(|p| {
        p.kind() == k::NEW_EXPRESSION
            || (p.kind() == k::ASSIGNMENT_EXPRESSION
                && p.first_child()
                    .is_some_and(|l| un(&l).kind() != k::IDENTIFIER_REFERENCE))
    }) || node.kind() == k::COMPUTED_MEMBER_EXPRESSION
        || simple_names;
    Ok(concat(vec![
        object_doc,
        if inline {
            lookup
        } else {
            group(indent(concat(vec![softline(), lookup])))
        },
    ]))
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum Link {
    Base,
    Call,
    /// A member access; `true` for `[0]` or `["a"]` with a literal.
    Member(bool),
    NonNull,
}

struct Part {
    node: SyntaxNode,
    link: Link,
    doc: Doc,
    commented: bool,
}

fn is_factory(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
        || (!name.is_empty() && name.chars().all(|c| c == '$' || c == '_'))
}

/// `a.b().c()` printed as a chain: the call `call_node` is the last link.
pub(crate) fn member_chain(cx: &FormatCtx<'_>, call_node: &SyntaxNode, call: &Call) -> R {
    let mut parts: Vec<Part> = Vec::new();
    let call_doc = |n: &SyntaxNode, c: &Call| -> R {
        let mut v = Vec::new();
        if let Some(o) = &c.optional {
            v.push(cx.token(o));
        }
        if let Some(t) = &c.type_args {
            v.push(cx.format(t)?);
        }
        v.push(arguments(cx, n, c)?);
        // A blank line after a call in the source stays.
        let blank = cx
            .last_token(n)
            .and_then(|l| l.next_token())
            .map(|mut t| {
                while cx.grammar.is_trivia(t.kind()) {
                    match t.next_token() {
                        Some(x) => t = x,
                        None => break,
                    }
                }
                t
            })
            .is_some_and(|t| {
                t.kind() == k::DOT && comments::blank_lines_before(cx.grammar, &t) > 0
            });
        if blank && n != call_node {
            v.push(hardline());
        }
        Ok(concat(v))
    };
    parts.push(Part {
        node: call_node.clone(),
        link: Link::Call,
        doc: call_doc(call_node, call)?,
        commented: false,
    });
    let mut cur_raw = call.callee.clone();
    loop {
        let mut cur = un(&cur_raw);
        // Parentheses that are not needed are not part of the chain's text.
        if cur_raw != cur && !(matches!(cur.kind(), k::CALL_EXPRESSION) || is_member(cur.kind()))
            || (cur_raw != cur
                && (needs_parens(cx, &cur) != Need::No || paren_commented(cx, &cur_raw)))
        {
            cur = cur_raw.clone();
        }
        match cur.kind() {
            k::CALL_EXPRESSION => {
                let Some(c) = Call::of(cx, &cur) else { break };
                let callee_kind = un(&c.callee).kind();
                if !(is_member(callee_kind) || callee_kind == k::CALL_EXPRESSION) {
                    break;
                }
                parts.push(Part {
                    node: cur.clone(),
                    link: Link::Call,
                    doc: call_doc(&cur, &c)?,
                    commented: false,
                });
                cur_raw = c.callee;
            }
            kind if is_member(kind) => {
                let Some(object) = cur.first_child() else {
                    break;
                };
                let literal = kind == k::COMPUTED_MEMBER_EXPRESSION
                    && cur.last_child().is_some_and(|p| is_literal(un(&p).kind()));
                let commented = kids(cx, &cur)
                    .iter()
                    .any(|e| token_of(e).is_some_and(|t| token_comments(cx, t)));
                parts.push(Part {
                    node: cur.clone(),
                    link: Link::Member(literal),
                    doc: lookup(&cur, cx)?,
                    commented,
                });
                cur_raw = object;
            }
            k::TS_NON_NULL_EXPRESSION => {
                let Some(inner) = cur.first_child() else {
                    break;
                };
                let bang = kids(cx, &cur)
                    .iter()
                    .rev()
                    .find_map(|e| token_of(e).map(|t| cx.token(t)))
                    .unwrap_or_else(|| lit("!"));
                parts.push(Part {
                    node: cur.clone(),
                    link: Link::NonNull,
                    doc: bang,
                    commented: false,
                });
                cur_raw = inner;
            }
            _ => break,
        }
    }
    parts.push(Part {
        doc: cx.format(&cur_raw)?,
        node: un(&cur_raw),
        link: Link::Base,
        commented: false,
    });
    parts.reverse();
    Ok(print_chain(cx, call_node, parts))
}

fn paren_commented(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    let mut cur = n.clone();
    while cur.kind() == k::PARENTHESIZED_EXPRESSION {
        if cur
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .any(|t| token_comments(cx, &t))
        {
            return true;
        }
        match cur.first_child() {
            Some(c) => cur = c,
            None => break,
        }
    }
    false
}

fn print_chain(cx: &FormatCtx<'_>, call_node: &SyntaxNode, parts: Vec<Part>) -> Doc {
    let n = parts.len();
    let is_memberish = |i: usize| matches!(parts[i].link, Link::Member(_));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut cur = vec![0];
    let mut i = 1;
    while i < n
        && matches!(
            parts[i].link,
            Link::NonNull | Link::Call | Link::Member(true)
        )
    {
        cur.push(i);
        i += 1;
    }
    if parts[0].node.kind() != k::CALL_EXPRESSION && parts[0].node.kind() != k::IMPORT_EXPRESSION {
        while i + 1 < n && is_memberish(i) && is_memberish(i + 1) {
            cur.push(i);
            i += 1;
        }
    }
    groups.push(std::mem::take(&mut cur));
    let mut seen_call = false;
    while i < n {
        if seen_call && is_memberish(i) {
            if parts[i].link == Link::Member(true) && !parts[i].commented {
                cur.push(i);
                i += 1;
                continue;
            }
            groups.push(std::mem::take(&mut cur));
            seen_call = false;
        }
        if parts[i].link == Link::Call {
            seen_call = true;
        }
        cur.push(i);
        i += 1;
    }
    if !cur.is_empty() {
        groups.push(cur);
    }

    let tab = tab_width(cx);
    let in_statement = parent(call_node).is_some_and(|p| p.kind() == k::EXPRESSION_STATEMENT);
    let should_not_wrap = || {
        let has_computed = groups
            .get(1)
            .and_then(|g| g.first())
            .is_some_and(|&i| parts[i].link == Link::Member(true));
        if groups[0].len() == 1 {
            let first = &parts[groups[0][0]].node;
            return first.kind() == k::THIS_EXPRESSION
                || (first.kind() == k::IDENTIFIER_REFERENCE
                    && name_of(cx, first).is_some_and(|name| {
                        is_factory(&name)
                            || (in_statement && name.chars().count() <= tab)
                            || has_computed
                    }));
        }
        let last = &parts[*groups[0].last().unwrap_or(&0)];
        matches!(last.link, Link::Member(_))
            && last.node.kind() == k::STATIC_MEMBER_EXPRESSION
            && last
                .node
                .last_child()
                .and_then(|p| name_of(cx, &p))
                .is_some_and(|name| is_factory(&name) || has_computed)
    };
    let should_merge = groups.len() >= 2 && !groups[1].is_empty() && should_not_wrap();

    let print_group = |g: &Vec<usize>| concat(g.iter().map(|&i| parts[i].doc.clone()).collect());
    let printed: Vec<Doc> = groups.iter().map(print_group).collect();
    let one_line = concat(printed.clone());
    let cutoff = if should_merge { 3 } else { 2 };
    let flat: Vec<usize> = groups.iter().flatten().copied().collect();
    let node_has_comment = flat[1..flat.len().saturating_sub(1)]
        .iter()
        .any(|&i| parts[i].commented)
        || groups
            .get(cutoff)
            .and_then(|g| g.first())
            .is_some_and(|&i| parts[i].commented);
    if groups.len() <= cutoff
        && !node_has_comment
        && !printed[cutoff.min(printed.len())..].iter().any(will_break)
    {
        return group(one_line);
    }
    let rest_from = if should_merge { 2 } else { 1 };
    let expanded = concat(vec![
        printed[0].clone(),
        if should_merge {
            printed[1].clone()
        } else {
            Doc::Nil
        },
        if groups.len() > rest_from {
            indent(concat(vec![
                hardline(),
                core_format::doc::join(&hardline(), printed[rest_from..].iter().cloned()),
            ]))
        } else {
            Doc::Nil
        },
    ]);
    let calls: Vec<&Part> = parts.iter().filter(|p| p.link == Link::Call).collect();
    let complex_args = calls.len() > 2
        && calls.iter().any(|p| {
            Call::of(cx, &p.node)
                .is_some_and(|c| !c.args.iter().all(|a| is_simple_call_argument(cx, &a.0, 0)))
        });
    let last_group_breaks_and_calls_have_functions = || {
        let last = &parts[*flat.last().unwrap_or(&0)];
        last.link == Link::Call
            && will_break(printed.last().unwrap_or(&Doc::Nil))
            && calls[..calls.len() - 1].iter().any(|p| {
                Call::of(cx, &p.node).is_some_and(|c| {
                    c.args
                        .iter()
                        .any(|a| super::ast::is_function(un(&a.0).kind()))
                })
            })
    };
    if node_has_comment {
        return group(expanded);
    }
    let result = if complex_args
        || printed[..printed.len() - 1].iter().any(will_break)
        || last_group_breaks_and_calls_have_functions()
    {
        group(expanded)
    } else {
        concat(vec![
            if will_break(&one_line) {
                break_parent()
            } else {
                Doc::Nil
            },
            conditional_group(vec![one_line, expanded]),
        ])
    };
    label("member-chain", result)
}

/// Does the call print as a chain that may break (more than a short `a.b.c()` access)?
pub(crate) fn is_chain_label(doc: &Doc) -> bool {
    doc.label_of() == Some("member-chain")
}

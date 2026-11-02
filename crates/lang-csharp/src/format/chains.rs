//! Expressions that lay out as a chain: binary operators, member accesses and calls, assignments
//! and the conditional operator.

use super::seq::{R, Sep, paren_group, sep_doc, sig};
use crate::generated::kind as k;
use core_format::doc::{self, Doc};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

fn shape(msg: &str) -> FormatError {
    FormatError::Shape(msg.into())
}

/// Does a line comment follow this token on its line? Whatever comes next must then start a
/// new line, or the comment would move to the end of a later line (and change owner).
fn ends_in_line_comment(cx: &FormatCtx<'_>, t: &SyntaxToken) -> bool {
    core_format::comments::trailing_comments(cx.grammar, t)
        .iter()
        .any(|c| c.kind != core_format::comments::CommentKind::Block)
}

/// The space after an operator, or a line break when its comment ends the line.
fn after_operator(cx: &FormatCtx<'_>, op: &SyntaxToken) -> Doc {
    if ends_in_line_comment(cx, op) {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

// ---------------------------------------------------------------- assignment

/// Does the right-hand side of `=` move to its own line when it does not fit?
fn breaks_after_operator(cx: &FormatCtx<'_>, rhs: &SyntaxNode) -> bool {
    // A comment on its own line before the value keeps the value on the next line.
    core_format::comments::node_leading(cx.grammar, rhs).has_comment
        || cx.grammar.has_tag(rhs.kind(), "binary-chain")
        || matches!(rhs.kind(), k::CONDITIONAL_EXPRESSION | k::QUERY_EXPRESSION)
}

/// `lead rhs`, where `lead` ends with the operator.
fn with_rhs(cx: &FormatCtx<'_>, lead: Doc, op: &SyntaxToken, rhs: &SyntaxNode) -> R {
    let rhs_doc = cx.format(rhs)?;
    Ok(if ends_in_line_comment(cx, op) {
        doc::concat(vec![
            lead,
            doc::indent(doc::concat(vec![doc::hardline(), rhs_doc])),
        ])
    } else if breaks_after_operator(cx, rhs) {
        doc::concat(vec![
            lead,
            doc::group(doc::indent(doc::concat(vec![doc::line(), rhs_doc]))),
        ])
    } else {
        doc::concat(vec![lead, doc::lit(" "), rhs_doc])
    })
}

pub fn assignment(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Node(left),
        SyntaxElement::Token(op),
        SyntaxElement::Node(right),
    ] = els.as_slice() else {
        return Err(shape("assignment: expected left, operator and right"));
    };
    let lead = doc::concat(vec![cx.format(left)?, doc::lit(" "), cx.token(op)]);
    with_rhs(cx, lead, op, right)
}

fn operator_and_value(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [SyntaxElement::Token(op), SyntaxElement::Node(value)] = els.as_slice() else {
        return Err(shape("expected an operator token and a value"));
    };
    with_rhs(cx, cx.token(op), op, value)
}

pub fn equals_value_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    operator_and_value(cx, node)
}

pub fn arrow_clause(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    operator_and_value(cx, node)
}

// --------------------------------------------------------------- conditional

pub fn conditional(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Node(cond),
        SyntaxElement::Token(q),
        SyntaxElement::Node(yes),
        SyntaxElement::Token(c),
        SyntaxElement::Node(no),
    ] = els.as_slice() else {
        return Err(shape("conditional expression: unexpected children"));
    };
    Ok(doc::group(doc::concat(vec![
        cx.format(cond)?,
        doc::indent(doc::concat(vec![
            doc::line(),
            cx.token(q),
            after_operator(cx, q),
            cx.format(yes)?,
            doc::line(),
            cx.token(c),
            after_operator(cx, c),
            cx.format(no)?,
        ])),
    ])))
}

pub fn parenthesized(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let [
        SyntaxElement::Token(open),
        SyntaxElement::Node(expr),
        SyntaxElement::Token(close),
    ] = els.as_slice() else {
        return Err(shape("parenthesized expression: unexpected children"));
    };
    paren_group(cx, open, expr, close)
}

// ------------------------------------------------------------------- binary

/// Operators of one precedence level chain into one layout; the level of an operator token.
fn level(op: &str) -> u8 {
    match op {
        "*" | "/" | "%" => 1,
        "+" | "-" => 2,
        "<<" | ">>" | ">>>" => 3,
        "<" | ">" | "<=" | ">=" => 4,
        "==" | "!=" => 5,
        "&" => 6,
        "^" => 7,
        "|" => 8,
        "&&" => 9,
        "||" => 10,
        "??" => 11,
        "and" => 12,
        _ => 13,
    }
}

fn parts_of(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<(SyntaxNode, SyntaxToken, SyntaxNode)> {
    match sig(cx, node).as_slice() {
        [
            SyntaxElement::Node(l),
            SyntaxElement::Token(op),
            SyntaxElement::Node(r),
        ] => Some((l.clone(), op.clone(), r.clone())),
        _ => None,
    }
}

/// Is the expression whose parent is `parent` printed inside something that already indents
/// its continuation lines?
fn parent_indents(cx: &FormatCtx<'_>, parent: Option<SyntaxNode>) -> bool {
    let Some(p) = parent else { return false };
    let kind = p.kind();
    matches!(
        kind,
        k::EQUALS_VALUE_CLAUSE
            | k::ARROW_EXPRESSION_CLAUSE
            | k::PARENTHESIZED_EXPRESSION
            | k::IF_STATEMENT
            | k::WHILE_STATEMENT
            | k::DO_STATEMENT
            | k::LOCK_STATEMENT
            | k::SWITCH_STATEMENT
    ) || cx.grammar.has_tag(kind, "assignment")
        || cx.grammar.has_tag(kind, "binary-chain")
}

/// `a + b + c`: the operands of one precedence level, one per line (operator first) when the
/// whole does not fit.
pub fn binary_chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let g = cx.grammar;
    let (_, top_op, _) = parts_of(cx, node).ok_or_else(|| shape("binary expression"))?;
    let lvl = level(top_op.text());
    let same_level = |n: &SyntaxNode| {
        g.has_tag(n.kind(), "binary-chain")
            && parts_of(cx, n).is_some_and(|(_, op, _)| level(op.text()) == lvl)
    };
    if lvl == 11 {
        return coalesce_chain(cx, node);
    }
    // Left to right: walk down the left spine.
    let mut rest: Vec<(SyntaxToken, SyntaxNode)> = Vec::new();
    let mut cur = node.clone();
    let first = loop {
        let (l, op, r) = parts_of(cx, &cur).ok_or_else(|| shape("binary expression"))?;
        rest.push((op, r));
        if same_level(&l) {
            cur = l;
        } else {
            break l;
        }
    };
    rest.reverse();
    let indent = !parent_indents(cx, node.parent());
    layout(cx, &first, &rest, indent)
}

/// `a ?? b ?? c` groups to the right: `a ?? (b ?? c)`.
fn coalesce_chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let g = cx.grammar;
    let mut operands: Vec<SyntaxNode> = Vec::new();
    let mut ops: Vec<SyntaxToken> = Vec::new();
    let mut cur = node.clone();
    loop {
        let (l, op, r) = parts_of(cx, &cur).ok_or_else(|| shape("binary expression"))?;
        operands.push(l);
        ops.push(op);
        let continues = g.has_tag(r.kind(), "binary-chain")
            && parts_of(cx, &r).is_some_and(|(_, o, _)| level(o.text()) == 11);
        if continues {
            cur = r;
        } else {
            operands.push(r);
            break;
        }
    }
    let first = operands.remove(0);
    let rest: Vec<(SyntaxToken, SyntaxNode)> = ops.into_iter().zip(operands).collect();
    let indent = !parent_indents(cx, node.parent());
    layout(cx, &first, &rest, indent)
}

fn layout(
    cx: &FormatCtx<'_>,
    first: &SyntaxNode,
    rest: &[(SyntaxToken, SyntaxNode)],
    indent: bool,
) -> R {
    let mut tail = Vec::with_capacity(rest.len());
    for (op, operand) in rest {
        tail.push(doc::line());
        tail.push(cx.token(op));
        tail.push(after_operator(cx, op));
        tail.push(cx.format(operand)?);
    }
    let tail = doc::concat(tail);
    Ok(doc::group(doc::concat(vec![
        cx.format(first)?,
        if indent { doc::indent(tail) } else { tail },
    ])))
}

// ------------------------------------------------------------------- member

enum Link {
    Dot(SyntaxToken, SyntaxNode),
    Call(SyntaxNode),
    Index(SyntaxNode),
    Bang(SyntaxToken),
    Question(SyntaxToken),
    /// What follows `?` when it is not a chain: `a?.b = c`.
    Tail(SyntaxNode),
}

/// Collects the links of the chain `node` ends with, in source order, and returns the head: the
/// expression the chain starts from (`None` when the chain starts with `.x` or `[i]` after a `?`).
fn walk(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    links: &mut Vec<Link>,
) -> Result<Option<SyntaxNode>, FormatError> {
    let els = sig(cx, node);
    let inner = |cx: &FormatCtx<'_>, n: &SyntaxNode, links: &mut Vec<Link>| {
        if cx.grammar.has_tag(n.kind(), "member-chain") {
            walk(cx, n, links)
        } else {
            Ok(Some(n.clone()))
        }
    };
    match (node.kind(), els.as_slice()) {
        (k::INVOCATION_EXPRESSION, [SyntaxElement::Node(e), SyntaxElement::Node(a)]) => {
            let head = inner(cx, e, links)?;
            links.push(Link::Call(a.clone()));
            Ok(head)
        }
        (k::ELEMENT_ACCESS_EXPRESSION, [SyntaxElement::Node(e), SyntaxElement::Node(a)]) => {
            let head = inner(cx, e, links)?;
            links.push(Link::Index(a.clone()));
            Ok(head)
        }
        (
            k::SIMPLE_MEMBER_ACCESS_EXPRESSION,
            [SyntaxElement::Node(e), SyntaxElement::Token(op), SyntaxElement::Node(name)],
        ) => {
            let head = inner(cx, e, links)?;
            links.push(Link::Dot(op.clone(), name.clone()));
            Ok(head)
        }
        (
            k::SUPPRESS_NULLABLE_WARNING_EXPRESSION,
            [SyntaxElement::Node(e), SyntaxElement::Token(op)],
        ) => {
            let head = inner(cx, e, links)?;
            links.push(Link::Bang(op.clone()));
            Ok(head)
        }
        (
            k::CONDITIONAL_ACCESS_EXPRESSION,
            [SyntaxElement::Node(e), SyntaxElement::Token(q), SyntaxElement::Node(w)],
        ) => {
            let head = inner(cx, e, links)?;
            links.push(Link::Question(q.clone()));
            if cx.grammar.has_tag(w.kind(), "member-chain") {
                walk(cx, w, links)?;
            } else {
                links.push(Link::Tail(w.clone()));
            }
            Ok(head)
        }
        (k::MEMBER_BINDING_EXPRESSION, [SyntaxElement::Token(dot), SyntaxElement::Node(name)]) => {
            links.push(Link::Dot(dot.clone(), name.clone()));
            Ok(None)
        }
        (k::ELEMENT_BINDING_EXPRESSION, [SyntaxElement::Node(a)]) => {
            links.push(Link::Index(a.clone()));
            Ok(None)
        }
        _ => Err(shape("member chain: unexpected children")),
    }
}

fn link_doc(cx: &FormatCtx<'_>, link: &Link) -> R {
    Ok(match link {
        Link::Dot(op, name) => doc::concat(vec![cx.token(op), cx.format(name)?]),
        Link::Call(a) | Link::Index(a) | Link::Tail(a) => cx.format(a)?,
        Link::Bang(t) | Link::Question(t) => cx.token(t),
    })
}

/// Does a line comment sit between the parts of the chain? Then every call goes on its own
/// line, or two comments would end up on one line.
fn has_line_comment(cx: &FormatCtx<'_>, links: &[Link]) -> bool {
    let g = cx.grammar;
    let line = |t: &SyntaxToken| {
        core_format::comments::trailing_comments(g, t)
            .iter()
            .chain(core_format::comments::leading_comments(g, t).iter())
            .any(|c| c.kind != core_format::comments::CommentKind::Block)
    };
    links.iter().any(|l| match l {
        Link::Dot(op, name) => line(op) || cx.last_token(name).is_some_and(|t| line(&t)),
        Link::Call(a) | Link::Index(a) | Link::Tail(a) => {
            cx.last_token(a).is_some_and(|t| line(&t))
        }
        Link::Bang(t) | Link::Question(t) => line(t),
    })
}

/// Is the head short enough, or a name that reads as a type, to stay on the first line with
/// the first call (`this.Foo()`, `Task.Run()`, `x.Foo()`)?
fn merges_first_group(cx: &FormatCtx<'_>, head: &SyntaxNode) -> bool {
    match head.kind() {
        k::THIS_EXPRESSION | k::BASE_EXPRESSION | k::PREDEFINED_TYPE => true,
        k::IDENTIFIER_NAME => {
            let text = head.text().to_string();
            let text = text.trim();
            text.chars().next().is_some_and(char::is_uppercase)
                || text.chars().count() <= cx.options.int("indent_size") as usize
        }
        _ => false,
    }
}

/// `a.B().C().D()`: on one line if it fits, else the head and then one `.Call()` per line. Chains
/// with fewer calls than `member_chain_min_calls` are never broken at the dots.
pub fn member_chain(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let mut links = Vec::new();
    let Some(head) = walk(cx, node, &mut links)? else {
        // `.name` or `[i]` after a `?`: the part of the chain that follows the head.
        let docs = links
            .iter()
            .map(|l| link_doc(cx, l))
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(doc::concat(docs));
    };
    let calls = links.iter().filter(|l| matches!(l, Link::Call(_))).count();

    // Split into the part that stays with the head and the groups that start at a dot.
    let mut prefix: Vec<Doc> = Vec::new();
    let mut groups: Vec<Vec<Doc>> = Vec::new();
    for (i, link) in links.iter().enumerate() {
        let starts = match link {
            Link::Dot(..) => !(i > 0 && matches!(links[i - 1], Link::Question(_))),
            Link::Question(_) => matches!(links.get(i + 1), Some(Link::Dot(..))),
            _ => false,
        };
        let d = link_doc(cx, link)?;
        if starts {
            groups.push(vec![d]);
        } else if let Some(g) = groups.last_mut() {
            g.push(d);
        } else {
            prefix.push(d);
        }
    }
    let mut head_parts = vec![cx.format(&head)?];
    head_parts.extend(prefix);
    let min_calls = cx.options.int(super::options::MEMBER_CHAIN_MIN_CALLS) as usize;
    let commented = has_line_comment(cx, &links);
    if (calls < min_calls && !commented) || groups.is_empty() {
        for g in groups {
            head_parts.extend(g);
        }
        return Ok(doc::concat(head_parts));
    }
    if merges_first_group(cx, &head) {
        head_parts.extend(groups.remove(0));
    }
    let flat = {
        let mut parts = head_parts.clone();
        for g in &groups {
            parts.extend(g.iter().cloned());
        }
        doc::concat(parts)
    };
    if groups.is_empty() {
        return Ok(flat);
    }
    let mut broken = head_parts;
    let mut tail = Vec::new();
    for g in groups {
        tail.push(sep_doc(cx, Sep::Hard));
        tail.extend(g);
    }
    broken.push(doc::indent(doc::concat(tail)));
    if commented {
        return Ok(doc::concat(broken));
    }
    Ok(doc::conditional_group(vec![flat, doc::concat(broken)]))
}

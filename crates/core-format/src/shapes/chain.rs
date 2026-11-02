use crate::FormatCtx;
use crate::comments::{self, CommentKind};
use crate::doc::{self, Doc};
use crate::error::FormatError;
use core_syntax::{SyntaxNode, SyntaxToken};

/// Does a line comment follow this token on its line? Whatever comes next must then start a new
/// line, or the comment would move to the end of a later line and change owner.
pub fn ends_in_line_comment(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> bool {
    comments::trailing_comments(cx.grammar, tok)
        .iter()
        .any(|c| c.kind != CommentKind::Block)
}

/// The space after an operator, or a line break when a line comment ends its line.
pub fn gap_after_operator(cx: &FormatCtx<'_>, op: &SyntaxToken) -> Doc {
    if ends_in_line_comment(cx, op) {
        doc::hardline()
    } else {
        doc::lit(" ")
    }
}

/// `a + b + c`: the operands in one group, the operator first on each continuation line when
/// the whole does not fit. `indent` indents the continuation lines; leave it off when the
/// caller already does.
pub fn binary_chain(
    cx: &FormatCtx<'_>,
    first: &SyntaxNode,
    rest: &[(SyntaxToken, SyntaxNode)],
    indent: bool,
) -> Result<Doc, FormatError> {
    let mut tail = Vec::with_capacity(rest.len() * 4);
    for (op, operand) in rest {
        tail.push(doc::line());
        tail.push(cx.token(op));
        tail.push(gap_after_operator(cx, op));
        tail.push(cx.format(operand)?);
    }
    let tail = doc::concat(tail);
    Ok(doc::group(doc::concat(vec![
        cx.format(first)?,
        if indent { doc::indent(tail) } else { tail },
    ])))
}

/// The pieces of a chain of member accesses and calls, already laid out.
pub struct MemberChain {
    /// The head and whatever stays on its line.
    pub head: Vec<Doc>,
    /// The links from each dot to the next, one group per line when the chain breaks.
    pub groups: Vec<Vec<Doc>>,
    /// Calls in the chain, counted against `min_calls`.
    pub calls: usize,
    /// A chain with fewer calls is never broken at the dots.
    pub min_calls: usize,
    /// Break at the dots whatever the width (a line comment sits between the links).
    pub force_break: bool,
    /// Break at the dots when a group other than the last has a forced break inside it (a call
    /// with a block lambda), as Prettier does.
    pub break_on_inner_break: bool,
}

/// `head.A().B().C()` on one line if it fits, else the head and then one group per line,
/// indented.
pub fn member_chain(chain: MemberChain) -> Doc {
    let MemberChain {
        head,
        groups,
        calls,
        min_calls,
        force_break,
        break_on_inner_break,
    } = chain;
    if groups.is_empty() || (calls < min_calls && !force_break) {
        let mut parts = head;
        parts.extend(groups.into_iter().flatten());
        return doc::concat(parts);
    }
    // Both candidates are built from the same parts: share them, or a chain nested in a chain
    // would copy its parts once more at every level.
    let head = doc::shared(doc::concat(head));
    let groups: Vec<Doc> = groups
        .into_iter()
        .map(|g| doc::shared(doc::concat(g)))
        .collect();
    let force_break = force_break
        || (break_on_inner_break
            && groups
                .split_last()
                .is_some_and(|(_, init)| init.iter().any(|g| g.clone().propagate_breaks())));
    let flat = (!force_break).then(|| {
        let mut parts = vec![head.clone()];
        parts.extend(groups.iter().cloned());
        doc::concat(parts)
    });
    let mut tail = Vec::new();
    for g in groups {
        tail.push(doc::hardline());
        tail.push(g);
    }
    let broken = doc::concat(vec![head, doc::indent(doc::concat(tail))]);
    match flat {
        Some(flat) => doc::conditional_group(vec![flat, broken]),
        None => broken,
    }
}

//! Compilation units and everything with a braced body: types, namespaces, blocks, switch
//! statements and expressions, enums, accessor lists.

use super::lists::source_trailing_comma;
use super::options::{csharpier, trailing_comma};
use super::seq::{R, Sep, direct_tokens, is_empty_braces, sep_doc, separator, seq_of, sig};
use super::tags::{self, Tag};
use crate::generated::kind as k;
use core_format::shapes::{
    BlankRule, BlockStyle, ListLayout, ListStyle, block_between, delimited_list_between,
    statement_sequence, statement_sequence_with,
};
use core_format::{FormatCtx, FormatError, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

fn nodes_of(els: &[SyntaxElement]) -> Vec<SyntaxNode> {
    els.iter()
        .filter_map(|e| match e {
            SyntaxElement::Node(n) => Some(n.clone()),
            SyntaxElement::Token(_) => None,
        })
        .collect()
}

fn position(els: &[SyntaxElement], t: &SyntaxToken) -> Option<usize> {
    els.iter()
        .position(|e| matches!(e, SyntaxElement::Token(x) if x == t))
}

/// The members, then the comments that end the file.
pub fn compilation_unit(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let items: Vec<SyntaxNode> = nodes_of(&els);
    let mut parts = vec![statement_sequence_with(
        cx,
        &items,
        1,
        member_rule(cx, node),
    )?];
    if let Some(SyntaxElement::Token(eof)) = els.last() {
        let trailing = comments::leading_comments(cx.grammar, eof);
        if !trailing.is_empty() {
            if !items.is_empty() {
                parts.push(doc::hardline());
                if comments::blank_lines_before(cx.grammar, eof) > 0 {
                    parts.push(doc::hardline());
                }
            }
            parts.push(core_format::context::leading_comments_doc(&trailing));
        }
    }
    Ok(doc::concat(parts))
}

/// `namespace N;` and then the members, which are not indented.
pub fn file_scoped_namespace(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    let semi = direct_tokens(node, k::SEMICOLON_TOKEN)
        .into_iter()
        .next()
        .and_then(|t| position(&els, &t))
        .ok_or_else(|| FormatError::Shape("FileScopedNamespaceDeclaration without `;`".into()))?;
    let header = seq_of(cx, node, &els[..=semi])?;
    let rest = nodes_of(&els[semi + 1..]);
    let Some(first) = rest.first().and_then(|n| cx.first_token(n)) else {
        return Ok(header);
    };
    let mut parts = vec![header, doc::hardline()];
    if comments::blank_lines_before(cx.grammar, &first) > 0 {
        parts.push(doc::hardline());
    }
    parts.push(statement_sequence_with(
        cx,
        &rest,
        1,
        member_rule(cx, node),
    )?);
    Ok(doc::concat(parts))
}

/// The body in braces of a node: `pre {block} post`, where `pre` and `post` are the other
/// children of the node laid out in sequence.
fn with_braces(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
    body: impl FnOnce(&SyntaxToken, &SyntaxToken) -> R,
) -> R {
    let els = sig(cx, node);
    let open = direct_tokens(node, k::OPEN_BRACE_TOKEN).into_iter().next();
    let close = direct_tokens(node, k::CLOSE_BRACE_TOKEN).into_iter().last();
    let (Some(open), Some(close)) = (open, close) else {
        return seq_of(cx, node, &els);
    };
    let (Some(oi), Some(ci)) = (position(&els, &open), position(&els, &close)) else {
        return seq_of(cx, node, &els);
    };
    let mut parts = Vec::new();
    if oi > 0 {
        parts.push(seq_of(cx, node, &els[..oi])?);
        let sep = if is_empty_braces(cx, &open, &close) {
            Sep::Space
        } else {
            Sep::Brace
        };
        parts.push(sep_doc(cx, sep));
    }
    parts.push(body(&open, &close)?);
    if ci + 1 < els.len() {
        let sep = separator(cx, node, &els[ci], &els[ci + 1]);
        parts.push(sep_doc(cx, sep));
        parts.push(seq_of(cx, node, &els[ci + 1..])?);
    }
    Ok(doc::concat(parts))
}

const BLOCK: BlockStyle = BlockStyle {
    max_blank_lines: 1,
    empty: " ",
    force_blank: None,
};

/// A member that is one line by nature: it may follow its neighbours without a blank line.
fn is_simple_member(cx: &FormatCtx<'_>, member: &SyntaxNode) -> bool {
    match member.kind() {
        k::FIELD_DECLARATION
        | k::EVENT_FIELD_DECLARATION
        | k::USING_DIRECTIVE
        | k::EXTERN_ALIAS_DIRECTIVE
        | k::DELEGATE_DECLARATION
        | k::ENUM_MEMBER_DECLARATION => true,
        k::PROPERTY_DECLARATION => member.children().all(|c| match c.kind() {
            k::ACCESSOR_LIST => accessors_inline(cx, &c),
            k::ARROW_EXPRESSION_CLAUSE => true,
            _ => true,
        }),
        _ => false,
    }
}

/// CSharpier puts a blank line between members, except between two simple ones when the second
/// has no comment, directive or attribute above it.
fn blank_between_members(cx: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
    let commented = cx
        .first_token(next)
        .is_some_and(|t| !comments::leading_comments(cx.grammar, &t).is_empty());
    let decorated = commented || next.children().any(|c| c.kind() == k::ATTRIBUTE_LIST);
    !(is_simple_member(cx, prev) && is_simple_member(cx, next) && !decorated)
}

/// The blank line rule of a body whose children are members, if the preset has one.
fn member_rule(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<BlankRule> {
    let g = cx.grammar;
    let members = tags::has(g, node.kind(), Tag::Namespace)
        || tags::has(g, node.kind(), Tag::TypeDeclaration)
        || node.kind() == k::COMPILATION_UNIT;
    (csharpier(cx.options) && members).then_some(blank_between_members as BlankRule)
}

/// Blocks, namespaces, types and switch statements.
pub fn braced(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let style = BlockStyle {
        force_blank: member_rule(cx, node),
        ..BLOCK
    };
    with_braces(cx, node, |open, close| {
        block_between(cx, node, open, close, &style)
    })
}

/// `{ get; set; }` on one line when nothing in it needs more room.
pub fn accessors_inline(cx: &FormatCtx<'_>, list: &SyntaxNode) -> bool {
    let g = cx.grammar;
    let open = direct_tokens(list, k::OPEN_BRACE_TOKEN).into_iter().next();
    let close = direct_tokens(list, k::CLOSE_BRACE_TOKEN).into_iter().last();
    let (Some(open), Some(close)) = (open, close) else {
        return false;
    };
    let range = TextRange::new(open.text_range().start(), close.text_range().start());
    let simple = list.children().all(|a| {
        g.is_trivia(a.kind())
            || a.children().all(|c| {
                !matches!(
                    c.kind(),
                    k::BLOCK | k::ARROW_EXPRESSION_CLAUSE | k::ATTRIBUTE_LIST
                )
            })
    });
    let has_comment = list
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .any(|t| g.has_tag(t.kind(), "comment"));
    simple && !has_comment && !cx.has_line_break(list, range)
}

pub fn accessor_list(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    if !accessors_inline(cx, node) {
        // CSharpier drops the blank lines between accessors.
        let style = BlockStyle {
            max_blank_lines: if csharpier(cx.options) { 0 } else { 1 },
            ..BLOCK
        };
        return with_braces(cx, node, |open, close| {
            block_between(cx, node, open, close, &style)
        });
    }
    let mut parts = Vec::new();
    for (i, e) in els.iter().enumerate() {
        if i > 0 {
            parts.push(doc::lit(" "));
        }
        parts.push(super::seq::element_doc(cx, e)?);
    }
    Ok(doc::concat(parts))
}

/// `get { return x; }`: CSharpier keeps the body of an accessor on its line when it is a single
/// simple statement without comments and everything fits; otherwise the braces get their own
/// lines. `None` for any accessor this does not apply to.
pub fn accessor_declaration(cx: &FormatCtx<'_>, accessor: &SyntaxNode) -> Option<R> {
    let g = cx.grammar;
    let els = sig(cx, accessor);
    let (SyntaxElement::Node(block), head) = els.split_last()? else {
        return None;
    };
    let body = sig(cx, block);
    let [
        SyntaxElement::Token(open),
        SyntaxElement::Node(statement),
        SyntaxElement::Token(close),
    ] = body.as_slice()
    else {
        return None;
    };
    let simple = block.kind() == k::BLOCK
        && matches!(
            statement.kind(),
            k::RETURN_STATEMENT | k::THROW_STATEMENT | k::EXPRESSION_STATEMENT
        );
    let has_comment = accessor
        .descendants_with_tokens()
        .filter_map(SyntaxElement::into_token)
        .any(|t| g.has_tag(t.kind(), "comment"));
    if !simple || has_comment {
        return None;
    }
    Some((|| {
        let head = seq_of(cx, accessor, head)?;
        let statement = doc::shared(cx.format(statement)?);
        if statement.clone().propagate_breaks() {
            return Ok(doc::concat(vec![head, doc::hardline(), cx.format(block)?]));
        }
        let (open, close) = (cx.token(open), cx.token(close));
        let one_line = doc::concat(vec![
            head.clone(),
            doc::lit(" "),
            open.clone(),
            doc::lit(" "),
            statement.clone(),
            doc::lit(" "),
            close.clone(),
        ]);
        let expanded = doc::concat(vec![
            head,
            doc::hardline(),
            open,
            doc::indent(doc::concat(vec![doc::hardline(), statement])),
            doc::hardline(),
            close,
        ]);
        Ok(doc::conditional_group(vec![one_line, expanded]))
    })())
}

/// `: base(args)` of a constructor, on its own line below the signature.
pub fn constructor_initializer(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let els = sig(cx, node);
    Ok(doc::indent(doc::concat(vec![
        doc::hardline(),
        seq_of(cx, node, &els)?,
    ])))
}

/// Labels one per line, then the statements indented below them.
pub fn switch_section(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    let g = cx.grammar;
    let (labels, statements): (Vec<_>, Vec<_>) = nodes_of(&sig(cx, node))
        .into_iter()
        .partition(|n| tags::has(g, n.kind(), Tag::SwitchLabel));
    let mut parts = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        if i > 0 {
            parts.push(doc::hardline());
        }
        parts.push(cx.format(label)?);
    }
    // CSharpier does not indent a block that is the only statement of a section.
    if csharpier(cx.options) && matches!(statements.as_slice(), [only] if only.kind() == k::BLOCK) {
        parts.push(doc::hardline());
        parts.push(cx.format(&statements[0])?);
    } else if !statements.is_empty() {
        parts.push(doc::indent(doc::concat(vec![
            doc::hardline(),
            statement_sequence(cx, &statements, 1)?,
        ])));
    }
    Ok(doc::concat(parts))
}

fn arms_style(cx: &FormatCtx<'_>, node: &SyntaxNode, close: &SyntaxToken) -> ListStyle {
    ListStyle {
        layout: ListLayout::Break,
        trailing_comma: trailing_comma(cx.options, source_trailing_comma(cx, node, close)),
        empty: " ",
        force_blank: (csharpier(cx.options) && node.kind() == k::ENUM_DECLARATION)
            .then_some(blank_between_members as BlankRule),
        ..ListStyle::default()
    }
}

/// `x switch`, then the arms between braces on their own lines.
pub fn switch_expression(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    with_braces(cx, node, |open, close| {
        delimited_list_between(cx, node, open, close, &arms_style(cx, node, close))
    })
}

pub fn enum_declaration(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    with_braces(cx, node, |open, close| {
        delimited_list_between(cx, node, open, close, &arms_style(cx, node, close))
    })
}

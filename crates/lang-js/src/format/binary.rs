//! Binary and logical expressions: chains of one precedence flattened into one group, the rest
//! indented under the first operand (Prettier's `printBinaryishExpression`).

use super::ast::{is_binaryish, operator, parent, token_comments, un};
use super::util::shape_error;
use crate::generated::kind as k;
use core_format::doc::{Doc, concat, group, indent, line, lit, softline};
use core_format::{FormatCtx, FormatError};
use core_syntax::SyntaxNode;

pub(crate) fn precedence(op: &str) -> u8 {
    match op {
        "??" => 1,
        "||" => 2,
        "&&" => 3,
        "|" => 4,
        "^" => 5,
        "&" => 6,
        "==" | "===" | "!=" | "!==" => 7,
        "<" | ">" | "<=" | ">=" | "in" | "instanceof" => 8,
        ">>" | "<<" | ">>>" => 9,
        "+" | "-" => 10,
        "*" | "/" | "%" => 11,
        "**" => 12,
        _ => 0,
    }
}

pub(crate) fn is_bitwise(op: &str) -> bool {
    matches!(op, ">>" | ">>>" | "<<" | "&" | "|" | "^")
}

fn multiplicative(op: &str) -> bool {
    matches!(op, "*" | "/" | "%")
}

/// May `a op b op2 c` print as one chain without parentheses around `a op b`?
pub(crate) fn should_flatten(parent_op: &str, node_op: &str) -> bool {
    if precedence(node_op) != precedence(parent_op) || parent_op == "**" {
        return false;
    }
    let equality = |o: &str| matches!(o, "==" | "===" | "!=" | "!==");
    let shift = |o: &str| matches!(o, ">>" | ">>>" | "<<");
    !(equality(parent_op) && equality(node_op)
        || (node_op == "%" && multiplicative(parent_op))
        || (parent_op == "%" && multiplicative(node_op))
        || (node_op != parent_op && multiplicative(node_op) && multiplicative(parent_op))
        || (shift(parent_op) && shift(node_op)))
}

/// `a || { ... }`, `a && [...]`: the right side stays on the operator's line.
pub(crate) fn should_inline_logical(node: &SyntaxNode) -> bool {
    if node.kind() != k::LOGICAL_EXPRESSION {
        return false;
    }
    let Some(right) = node.last_child() else {
        return false;
    };
    let right = un(&right);
    match right.kind() {
        k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION => {
            right.children_with_tokens().any(|e| e.as_node().is_some())
        }
        k::JSX_ELEMENT | k::JSX_FRAGMENT => true,
        _ => false,
    }
}

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if !matches!(
        node.kind(),
        k::BINARY_EXPRESSION | k::LOGICAL_EXPRESSION | k::PRIVATE_IN_EXPRESSION
    ) {
        return Ok(None);
    }
    let p = parent(node);
    let pk = p.as_ref().map(|p| p.kind());
    let c = super::ast::outer(node);
    let first_of_parent = p
        .as_ref()
        .is_some_and(|p| p.first_child().as_ref() == Some(&c));
    let last_of_parent = p
        .as_ref()
        .is_some_and(|p| p.last_child().as_ref() == Some(&c));
    let inside_parens = match pk {
        Some(k::IF_STATEMENT | k::WHILE_STATEMENT | k::SWITCH_STATEMENT) => first_of_parent,
        Some(k::DO_WHILE_STATEMENT) => last_of_parent,
        _ => false,
    };
    let parts = parts(node, cx, inside_parens)?;
    if inside_parens {
        return Ok(Some(concat(parts)));
    }
    let Some(p) = p else {
        return Ok(Some(group(concat(parts))));
    };
    let pk = p.kind();
    if (matches!(pk, k::CALL_EXPRESSION | k::NEW_EXPRESSION) && first_of_parent)
        || pk == k::UNARY_EXPRESSION
        || matches!(
            pk,
            k::STATIC_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION
        )
    {
        return Ok(Some(group(concat(vec![
            indent(concat(vec![softline(), concat(parts)])),
            softline(),
        ]))));
    }
    let gp = parent(&p);
    let gk = gp.as_ref().map(|g| g.kind());
    let should_not_indent = matches!(pk, k::RETURN_STATEMENT | k::THROW_STATEMENT)
        || (pk == k::JSX_EXPRESSION_CONTAINER && gk == Some(k::JSX_ATTRIBUTE))
        || (pk == k::ARROW_FUNCTION_EXPRESSION && last_of_parent)
        || (pk == k::FOR_STATEMENT && !last_of_parent)
        || (pk == k::CONDITIONAL_EXPRESSION
            && !matches!(
                gk,
                Some(k::RETURN_STATEMENT | k::THROW_STATEMENT | k::CALL_EXPRESSION)
            ))
        || pk == k::TEMPLATE_LITERAL;
    let indent_if_inlining = matches!(
        pk,
        k::ASSIGNMENT_EXPRESSION
            | k::VARIABLE_DECLARATOR
            | k::PROPERTY_DEFINITION
            | k::OBJECT_PROPERTY
    );
    let op = operator(cx, node)
        .map(|t| t.text().to_string())
        .unwrap_or_default();
    let same_precedence_sub = node.first_child().map(|l| un(&l)).is_some_and(|l| {
        is_binaryish(l.kind()) && should_flatten(&op, &super::ast::operator_text(cx, &l))
    });
    let inline = should_inline_logical(node);
    if should_not_indent || (inline && !same_precedence_sub) || (!inline && indent_if_inlining) {
        return Ok(Some(group(concat(parts))));
    }
    if parts.is_empty() {
        return Ok(Some(Doc::Nil));
    }
    let mut it = parts.into_iter();
    let head = it.next().unwrap_or(Doc::Nil);
    Ok(Some(group(concat(vec![
        head,
        indent(concat(it.collect())),
    ]))))
}

/// The operands and operators of the chain `node` starts, as the printed parts: the first
/// operand, then ` ` and the operator and operand for each step.
fn parts(
    node: &SyntaxNode,
    cx: &FormatCtx<'_>,
    inside_parens: bool,
) -> Result<Vec<Doc>, FormatError> {
    let (Some(left), Some(right)) = (node.first_child(), node.last_child()) else {
        return Err(shape_error("binary expression"));
    };
    let op_tok = operator(cx, node).ok_or_else(|| shape_error("binary expression"))?;
    let op = op_tok.text().to_string();
    let left_u = un(&left);
    // Prettier's tree has no parentheses; only commented ones are in the way of flattening.
    let clean = |n: &SyntaxNode| {
        std::iter::successors(Some(n.clone()), |x| {
            (x.kind() == k::PARENTHESIZED_EXPRESSION)
                .then(|| x.first_child())
                .flatten()
        })
        .all(|x| {
            x.kind() != k::PARENTHESIZED_EXPRESSION
                || !x
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .any(|t| token_comments(cx, &t))
        })
    };
    let mut parts = if is_binaryish(left_u.kind())
        && node.kind() != k::PRIVATE_IN_EXPRESSION
        && should_flatten(&op, &super::ast::operator_text(cx, &left_u))
        && clean(&left)
    {
        parts(&left_u, cx, inside_parens)?
    } else {
        vec![group(cx.format(&left)?)]
    };
    let inline = should_inline_logical(node);
    let op_doc = cx.token(&op_tok);
    let right_doc = cx.format(&right)?;
    let rhs = if inline {
        concat(vec![op_doc, lit(" "), right_doc])
    } else {
        concat(vec![op_doc, line(), right_doc])
    };
    let p = parent(node);
    let right_u = un(&right);
    let should_group = !(inside_parens && node.kind() == k::LOGICAL_EXPRESSION)
        && p.is_none_or(|p| p.kind() != node.kind())
        && left_u.kind() != node.kind()
        && right_u.kind() != node.kind();
    parts.push(lit(" "));
    parts.push(if should_group { group(rhs) } else { rhs });
    Ok(parts)
}

//! Objects, arrays and destructuring patterns (Prettier's `printObject`, `printArray`,
//! `printProperty`): an object stays expanded when the source has a line break after its `{`,
//! numbers fill lines, a pattern that holds a nested pattern always breaks. The quotes of property
//! names (`quote_props`) are decided before formatting, in [`key_edits`].

use super::ast::{parent, un};
use super::util::{R, inline, is_tok, kids, node_of, shape_error, style, token_of, trailing_comma};
use crate::generated::kind as k;
use core_config::{OptionValue, ResolvedOptions};
use core_format::comments;
use core_format::doc::{
    Doc, break_parent, concat, fill, group, group_broken, group_with_id, hardline, if_break_in,
    indent, line, lit, new_group_id, softline,
};
use core_format::{FormatCtx, FormatError};
use core_syntax::{Grammar, SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    let doc = match node.kind() {
        k::OBJECT_EXPRESSION | k::OBJECT_PATTERN | k::OBJECT_ASSIGNMENT_TARGET => object(node, cx)?,
        k::ARRAY_EXPRESSION | k::ARRAY_PATTERN | k::ARRAY_ASSIGNMENT_TARGET => array(node, cx)?,
        k::OBJECT_PROPERTY | k::BINDING_PROPERTY | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY => {
            property(node, cx)?
        }
        k::ASSIGNMENT_TARGET_PROPERTY_IDENTIFIER
        | k::ASSIGNMENT_PATTERN
        | k::ASSIGNMENT_TARGET_WITH_DEFAULT
        | k::BINDING_REST_ELEMENT
        | k::ASSIGNMENT_TARGET_REST
        | k::FORMAL_PARAMETER_REST => inline(cx, &kids(cx, node))?,
        _ => return Ok(None),
    };
    Ok(Some(doc))
}

/// One element of a list: `None` for a hole (`[a, , b]`), and the comma after it.
struct Item {
    node: Option<SyntaxNode>,
    comma: Option<SyntaxToken>,
}

/// The elements between the brackets of `node`, each with its comma.
fn items(
    cx: &FormatCtx<'_>,
    node: &SyntaxNode,
) -> Result<(SyntaxToken, Vec<Item>, SyntaxToken), FormatError> {
    let elems = kids(cx, node);
    let (Some(open), Some(close)) = (
        elems.first().and_then(token_of),
        elems.last().and_then(token_of),
    ) else {
        return Err(shape_error("list without brackets"));
    };
    let mut items: Vec<Item> = Vec::new();
    // The last item still waits for its comma.
    let mut open_item = false;
    for e in &elems[1..elems.len() - 1] {
        match e {
            // A hole of an array expression holds its own comma.
            SyntaxElement::Node(n) if n.kind() == k::ELISION => {
                items.push(Item {
                    node: None,
                    comma: n
                        .children_with_tokens()
                        .filter_map(|c| c.into_token())
                        .find(|t| !cx.grammar.is_trivia(t.kind())),
                });
                open_item = false;
            }
            SyntaxElement::Node(n) => {
                items.push(Item {
                    node: Some(n.clone()),
                    comma: None,
                });
                open_item = true;
            }
            SyntaxElement::Token(t) if open_item => {
                if let Some(last) = items.last_mut() {
                    last.comma = Some(t.clone());
                }
                open_item = false;
            }
            SyntaxElement::Token(t) => items.push(Item {
                node: None,
                comma: Some(t.clone()),
            }),
        }
    }
    Ok((open.clone(), items, close.clone()))
}

/// The printed items, each with the comments of its comma, and whether a blank line follows.
fn printed(cx: &FormatCtx<'_>, items: &[Item]) -> Result<Vec<(Doc, bool)>, FormatError> {
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let mut d = match &item.node {
            Some(n) => cx.format(n)?,
            None => Doc::Nil,
        };
        if let Some(c) = &item.comma {
            d = concat(vec![
                d,
                cx.leading_comments_doc(c),
                cx.trailing_comments_doc(c),
            ]);
        }
        let blank = items
            .get(i + 1)
            .and_then(|n| n.node.as_ref())
            .and_then(|n| cx.first_token(n))
            .is_some_and(|t| comments::blank_lines_before(cx.grammar, &t) > 0);
        out.push((d, blank));
    }
    Ok(out)
}

fn is_rest(item: &Item) -> bool {
    item.node.as_ref().is_some_and(|n| {
        matches!(
            n.kind(),
            k::BINDING_REST_ELEMENT | k::ASSIGNMENT_TARGET_REST
        )
    })
}

/// `{ a, b }` of an object, an object pattern or an assignment target.
fn object(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let (open, items, close) = items(cx, node)?;
    if items.is_empty() {
        return Ok(empty_with_comments(cx, &open, &close));
    }
    let is_pattern = node.kind() != k::OBJECT_EXPRESSION;
    let p = parent(node);
    let pk = p.as_ref().map(SyntaxNode::kind);
    let first_line_break = !is_pattern
        && items[0]
            .node
            .as_ref()
            .and_then(|n| cx.first_token(n))
            .is_some_and(|t| {
                cx.has_line_break(
                    node,
                    TextRange::new(open.text_range().start(), t.text_range().start()),
                )
            });
    let nested_pattern = is_pattern
        && !matches!(
            pk,
            Some(k::FORMAL_PARAMETER | k::CATCH_PARAMETER | k::ASSIGNMENT_PATTERN)
        )
        && items.iter().filter_map(|i| i.node.as_ref()).any(|prop| {
            prop.children_with_tokens()
                .any(|e| e.as_token().is_some_and(|t| t.kind() == k::COLON))
                && prop.last_child().is_some_and(|v| {
                    matches!(
                        v.kind(),
                        k::OBJECT_PATTERN
                            | k::ARRAY_PATTERN
                            | k::OBJECT_ASSIGNMENT_TARGET
                            | k::ARRAY_ASSIGNMENT_TARGET
                    )
                })
        });
    let must_break = first_line_break || nested_pattern;
    let docs = printed(cx, &items)?;
    let mut parts = Vec::new();
    for (i, (d, _)) in docs.iter().enumerate() {
        if i > 0 {
            parts.extend([lit(","), line()]);
            if docs[i - 1].1 {
                parts.push(hardline());
            }
        }
        parts.push(d.clone());
    }
    let edge = if style(cx).bracket_spacing {
        line()
    } else {
        softline()
    };
    let comma = if items.last().is_some_and(is_rest) {
        Doc::Nil
    } else {
        trailing_comma(cx, false)
    };
    // The comments in front of the `{` belong to the whole object, not to its group.
    let lead = cx.leading_comments_doc(&open);
    let body = concat(vec![
        cx.token_text(&open),
        cx.trailing_comments_doc(&open),
        indent(concat(vec![edge.clone(), concat(parts)])),
        comma,
        edge,
        cx.token(&close),
    ]);
    // A lone pattern parameter hugs the parentheses and a pattern on the left of `=` shares the
    // group of the assignment.
    let shared = is_pattern
        && match (&p, pk) {
            (Some(p), Some(k::FORMAL_PARAMETER)) => {
                pattern_with_type_literal(p)
                    || (!p.children_with_tokens().any(|e| is_tok(&e, "="))
                        && hugs_only_param(p)
                        && p.parent().is_some_and(|fp| fp.children().count() == 1))
            }
            (_, Some(k::VARIABLE_DECLARATOR | k::ASSIGNMENT_EXPRESSION)) => !must_break,
            _ => false,
        };
    Ok(concat(vec![
        lead,
        match (shared, must_break) {
            (true, _) => body,
            (false, true) => group_broken(body),
            (false, false) => group(body),
        },
    ]))
}

/// A parameter `{ a, b }: { a: A; b: B }`: the pattern and the type literal break together,
/// as one group of the parameter.
pub(crate) fn pattern_with_type_literal(param: &SyntaxNode) -> bool {
    param.kind() == k::FORMAL_PARAMETER
        && param
            .first_child()
            .is_some_and(|c| c.kind() == k::OBJECT_PATTERN)
        && param
            .children()
            .find(|c| c.kind() == k::TS_TYPE_ANNOTATION)
            .and_then(|a| a.first_child())
            .is_some_and(|t| t.kind() == k::TS_TYPE_LITERAL)
}

/// A lone parameter that is a pattern, kept on the line of the parentheses (Prettier's
/// `shouldHugTheOnlyFunctionParameter`): `({ a, b }) =>`, `([a]: T) =>`, `({ a } = {}) =>`.
pub(crate) fn hugs_only_param(param: &SyntaxNode) -> bool {
    if param.kind() != k::FORMAL_PARAMETER {
        return false;
    }
    let nodes: Vec<SyntaxNode> = param.children().collect();
    let has_eq = param
        .children_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.text() == "="));
    let pattern = nodes
        .first()
        .is_some_and(|n| matches!(n.kind(), k::OBJECT_PATTERN | k::ARRAY_PATTERN));
    pattern
        && match (
            has_eq,
            nodes[1..]
                .iter()
                .filter(|n| n.kind() != k::TS_TYPE_ANNOTATION)
                .collect::<Vec<_>>()
                .as_slice(),
        ) {
            (false, rest) => rest.is_empty(),
            (true, [init]) => match init.kind() {
                k::IDENTIFIER_REFERENCE => true,
                k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION => init.children().next().is_none(),
                _ => false,
            },
            (true, _) => false,
        }
}

/// `[a, b]`: a group, or a fill when it holds only numbers.
fn array(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let (open, items, close) = items(cx, node)?;
    if items.is_empty() {
        return Ok(empty_with_comments(cx, &open, &close));
    }
    let concise = node.kind() == k::ARRAY_EXPRESSION && super::call::is_concise_array(node);
    let id = new_group_id();
    let comma = if items.last().is_some_and(|i| i.node.is_none()) {
        lit(",")
    } else if items.last().is_some_and(is_rest) {
        Doc::Nil
    } else if concise {
        if_break_in(id, lit(","), Doc::Nil)
    } else {
        trailing_comma(cx, false)
    };
    let docs = printed(cx, &items)?;
    let last = docs.len() - 1;
    let body = if concise {
        let mut parts = Vec::new();
        for (i, (d, blank)) in docs.into_iter().enumerate() {
            if i == last {
                parts.push(concat(vec![d, comma.clone()]));
            } else {
                parts.push(concat(vec![d, lit(",")]));
                parts.push(if blank {
                    concat(vec![hardline(), hardline()])
                } else {
                    line()
                });
            }
        }
        fill(parts)
    } else {
        let mut parts = Vec::new();
        for (i, (d, blank)) in docs.into_iter().enumerate() {
            parts.push(d);
            if i != last {
                parts.extend([lit(","), line()]);
                if blank {
                    parts.push(softline());
                }
            }
        }
        parts.push(comma);
        concat(parts)
    };
    let must_break = node.kind() == k::ARRAY_EXPRESSION && items.len() > 1 && {
        let elems: Vec<SyntaxNode> = items
            .iter()
            .filter_map(|i| i.node.as_ref().map(un))
            .collect();
        elems.len() == items.len()
            && elems.iter().enumerate().all(|(i, e)| {
                matches!(e.kind(), k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION)
                    && elems.get(i + 1).is_none_or(|n| n.kind() == e.kind())
                    && e.children().count() > 1
            })
    };
    // The comments in front of the `[` belong to the whole array, not to its group.
    Ok(concat(vec![
        cx.leading_comments_doc(&open),
        group_with_id(
            id,
            concat(vec![
                cx.token_text(&open),
                cx.trailing_comments_doc(&open),
                indent(concat(vec![softline(), body])),
                softline(),
                cx.token(&close),
                if must_break { break_parent() } else { Doc::Nil },
            ]),
        ),
    ]))
}

/// `key: value` laid out as an assignment; a shorthand property or a method is printed as it
/// is written.
fn property(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let Some(colon) = elems.iter().position(|e| is_tok(e, ":")) else {
        return inline(cx, &elems);
    };
    let (Some(tok), Some(value)) = (
        token_of(&elems[colon]),
        elems.get(colon + 1).and_then(node_of),
    ) else {
        return Err(shape_error("property"));
    };
    let computed = elems[..colon].iter().any(|e| is_tok(e, "["));
    let short_key = !computed
        && elems[..colon]
            .iter()
            .rev()
            .find_map(node_of)
            .is_some_and(|key| {
                let width = super::ast::text_of(cx, key).chars().count();
                width < usize::try_from(cx.options.int("indent_size")).unwrap_or(2) + 3
            });
    super::assign::assign_like(
        cx,
        node,
        inline(cx, &elems[..colon])?,
        cx.token(tok),
        Some(value),
        short_key,
    )
}

/// The key of a property that is written with a `:` or as a method, with whether it is
/// quoted. `None` for computed keys, shorthand properties and anything but a name or a string.
fn key_of(prop: &SyntaxNode) -> Option<(SyntaxNode, bool)> {
    let shorthand = match prop.kind() {
        k::OBJECT_PROPERTY => prop.children().count() == 1,
        k::BINDING_PROPERTY | k::ASSIGNMENT_TARGET_PROPERTY_PROPERTY => !prop
            .children_with_tokens()
            .any(|e| e.as_token().is_some_and(|t| t.kind() == k::COLON)),
        _ => return None,
    };
    let computed = prop
        .children_with_tokens()
        .any(|e| e.as_token().is_some_and(|t| t.kind() == k::L_BRACKET));
    if shorthand || computed {
        return None;
    }
    let key = prop.first_child()?;
    match key.kind() {
        k::STRING_LITERAL => Some((key, true)),
        k::IDENTIFIER_NAME => Some((key, false)),
        _ => None,
    }
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    chars
        .next()
        .is_some_and(|c| c == '$' || c == '_' || c.is_alphabetic())
        && chars.all(|c| c == '$' || c == '_' || c.is_alphanumeric())
}

/// The edits of the property names of every object in `tree` for `quote_props`: quotes that are
/// not needed are removed, and with `consistent` the names are quoted when one needs it. A cut
/// is `(offset, None)`, an insertion `(offset, Some(text))`.
pub(crate) fn key_edits(
    tree: &SyntaxNode,
    text: &str,
    grammar: &Grammar,
    options: &ResolvedOptions,
) -> Vec<(usize, Option<&'static str>)> {
    let mode = match options.get("quote_props") {
        Some(OptionValue::String(s)) => s.as_str(),
        _ => "preserve",
    };
    let mut edits = Vec::new();
    if mode == "preserve" {
        return edits;
    }
    let tok = |n: &SyntaxNode| super::rewrite::first_token(grammar, n);
    for object in tree.descendants().filter(|n| {
        matches!(
            n.kind(),
            k::OBJECT_EXPRESSION | k::OBJECT_PATTERN | k::OBJECT_ASSIGNMENT_TARGET
        )
    }) {
        let keys: Vec<(SyntaxNode, bool)> = object.children().filter_map(|p| key_of(&p)).collect();
        // A quoted name that is not a plain identifier (or number) keeps the quotes of all names.
        let needs_quotes = mode == "consistent"
            && keys.iter().any(|(key, quoted)| {
                *quoted
                    && tok(key).is_some_and(|t| {
                        let raw = t.text();
                        let inner = &raw[1..raw.len().saturating_sub(1).max(1)];
                        !is_identifier(inner) && !inner.bytes().all(|b| b.is_ascii_digit())
                    })
            });
        for (key, quoted) in keys {
            let Some(t) = tok(&key) else { continue };
            let start = usize::from(t.text_range().start());
            let end = usize::from(t.text_range().end());
            if quoted && !needs_quotes {
                let inner = &t.text()[1..t.text().len() - 1];
                let glued = text[..start]
                    .chars()
                    .next_back()
                    .is_some_and(|c| c == '$' || c == '_' || c.is_alphanumeric());
                if is_identifier(inner) && !glued {
                    edits.push((start, None));
                    edits.push((end - 1, None));
                }
            } else if !quoted && needs_quotes {
                edits.push((start, Some("\"")));
                edits.push((end, Some("\"")));
            }
        }
    }
    edits
}

/// `[]` or `{}`; comments on lines of their own inside go indented between the brackets.
fn empty_with_comments(cx: &FormatCtx<'_>, open: &SyntaxToken, close: &SyntaxToken) -> Doc {
    use core_format::comments::{self, CommentKind};
    let mut all = comments::trailing_comments(cx.grammar, open);
    all.extend(comments::leading_comments(cx.grammar, close));
    let own_lines = all
        .iter()
        .any(|c| c.kind != CommentKind::Block || c.breaks_after > 0);
    if !own_lines {
        // `{/* c */}`: inline comments sit tight between the brackets.
        let inner = all
            .iter()
            .map(core_format::context::comment_doc)
            .collect::<Vec<_>>();
        return concat(vec![
            cx.leading_comments_doc(open),
            cx.token_text(open),
            core_format::doc::join(&lit(" "), inner),
            cx.token_text(close),
            cx.trailing_comments_doc(close),
        ]);
    }
    concat(vec![
        cx.leading_comments_doc(open),
        cx.token_text(open),
        indent(core_format::shapes::dangling_comments(&all, false)),
        hardline(),
        cx.token_text(close),
        cx.trailing_comments_doc(close),
    ])
}

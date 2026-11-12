//! Calls and `new`: the argument list (hugging a last or first function or object argument),
//! test-call and `require` forms. Member chains are `chain.rs`.

use super::arrow::{self, Flags};
use super::ast::{Call, has_comment, is_function, is_literal, nodes, parent, un};
use super::chain;
use super::util::{R, kids, shape_error, token_of, will_break};
use crate::generated::kind as k;
use core_config::OptionValue;
use core_format::doc::{
    self, Doc, break_parent, concat, conditional_group, group, group_broken, hardline, indent,
    line, lit, shared, softline,
};
use core_format::{FormatCtx, FormatError, comments};
use core_syntax::{SyntaxElement, SyntaxNode};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    match node.kind() {
        k::CALL_EXPRESSION | k::NEW_EXPRESSION | k::IMPORT_EXPRESSION => {
            Ok(Some(cx.layout(node, 0, || call(node, cx))?))
        }
        _ => Ok(None),
    }
}

fn call(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let is_new = node.kind() == k::NEW_EXPRESSION;
    if node.kind() == k::IMPORT_EXPRESSION {
        return import(node, cx);
    }
    let Some(c) = Call::of(cx, node) else {
        return Ok(super::literal::verbatim(cx, node));
    };
    let callee_u = un(&c.callee);
    let args = &c.args;
    let special = !args.is_empty()
        && ((!is_new && is_common_js_or_amd(cx, node, &c))
            || (args.len() == 1 && template_on_own_line(cx, &un(&args[0].0)))
            || (!is_new && is_test_call(cx, &c)));
    if !special
        && !is_new
        && super::ast::is_member(callee_u.kind())
        && !has_inner_comment(cx, &callee_u)
    {
        return chain::member_chain(cx, node, &c);
    }
    let mut parts = Vec::new();
    if is_new {
        parts.push(elem_first(cx, node)?);
        parts.push(lit(" "));
    }
    parts.push(cx.format(&c.callee)?);
    if let Some(o) = &c.optional {
        parts.push(cx.token(o));
    }
    if let Some(t) = &c.type_args {
        parts.push(cx.format(t)?);
    }
    if special {
        let printed = args
            .iter()
            .map(|(a, _)| cx.format(a))
            .collect::<Result<Vec<_>, _>>()?;
        let test = !is_new && is_test_call(cx, &c);
        let printed = if test {
            // The parameters of a test's function stay on the line.
            args.iter()
                .map(|(a, _)| match un(a).kind() {
                    k::ARROW_FUNCTION_EXPRESSION | k::FUNCTION => {
                        Ok(arrow::print_fn(cx, &un(a), Flags::TEST)?.unwrap_or(Doc::Nil))
                    }
                    _ => cx.format(a),
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            printed
        };
        let (open, close) = parens(cx, &c)?;
        parts.push(open);
        parts.push(doc::join(&lit(", "), printed));
        parts.push(close);
        return Ok(concat(parts));
    }
    parts.push(arguments(cx, node, &c)?);
    let contents = concat(parts);
    Ok(if callee_u.kind() == k::CALL_EXPRESSION {
        group(contents)
    } else {
        contents
    })
}

/// A comment after `n`; the ones in front of its first token belong to the whole expression.
fn has_inner_comment(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    super::ast::has_trailing_comment(cx, n)
}

fn elem_first(cx: &FormatCtx<'_>, node: &SyntaxNode) -> R {
    match kids(cx, node).first().and_then(token_of) {
        Some(t) => Ok(cx.token(t)),
        None => Err(shape_error("new")),
    }
}

fn parens(cx: &FormatCtx<'_>, c: &Call) -> Result<(Doc, Doc), FormatError> {
    match (&c.open, &c.close) {
        (Some(o), Some(cl)) => Ok((cx.token(o), cx.token(cl))),
        _ => Ok((lit("("), lit(")"))),
    }
}

/// `import(source, options)`.
fn import(node: &SyntaxNode, cx: &FormatCtx<'_>) -> R {
    let elems = kids(cx, node);
    let open = elems
        .iter()
        .position(|e| token_of(e).is_some_and(|t| t.kind() == k::L_PAREN));
    let Some(open) = open else {
        return Ok(super::literal::verbatim(cx, node));
    };
    let mut head = Vec::new();
    for e in &elems[..open] {
        head.push(super::util::elem(cx, e)?);
    }
    let args: Vec<(SyntaxNode, Option<core_syntax::SyntaxToken>)> = {
        let mut v: Vec<(SyntaxNode, Option<core_syntax::SyntaxToken>)> = Vec::new();
        for e in &elems[open + 1..] {
            match e {
                SyntaxElement::Node(n) => v.push((n.clone(), None)),
                SyntaxElement::Token(t) if t.kind() == k::COMMA => {
                    if let Some(l) = v.last_mut() {
                        l.1 = Some(t.clone());
                    }
                }
                SyntaxElement::Token(_) => {}
            }
        }
        v
    };
    let c = Call {
        callee: node.clone(),
        type_args: None,
        optional: None,
        open: elems[open].as_token().cloned(),
        args,
        close: elems.last().and_then(token_of).cloned(),
    };
    Ok(concat(vec![
        concat(head),
        group(arguments_inner(cx, node, &c, true)?),
    ]))
}

pub(crate) fn arguments(cx: &FormatCtx<'_>, node: &SyntaxNode, c: &Call) -> R {
    arguments_inner(cx, node, c, false)
}

/// The parenthesized argument list (Prettier's `printCallArguments`).
fn arguments_inner(cx: &FormatCtx<'_>, node: &SyntaxNode, c: &Call, dynamic_import: bool) -> R {
    let (open, close) = parens(cx, c)?;
    let args = &c.args;
    if args.is_empty() {
        return Ok(concat(vec![open, close]));
    }
    let uns: Vec<SyntaxNode> = args.iter().map(|a| un(&a.0)).collect();
    if is_react_hook(cx, &uns) {
        return Ok(concat(vec![
            open,
            cx.format(&args[0].0)?,
            lit(", "),
            cx.format(&args[1].0)?,
            close,
        ]));
    }
    let g = cx.grammar;
    let mut any_empty_line = false;
    let mut printed: Vec<Doc> = Vec::with_capacity(args.len());
    let mut docs: Vec<Doc> = Vec::with_capacity(args.len());
    let mut seps: Vec<Doc> = Vec::with_capacity(args.len());
    for (i, (a, comma)) in args.iter().enumerate() {
        let doc = shared(cx.format(a)?);
        let mut parts = vec![doc.clone()];
        let mut sep = Doc::Nil;
        if i + 1 < args.len() {
            sep = comma.as_ref().map_or(lit(","), |t| cx.token(t));
            let blank = cx
                .first_token(&args[i + 1].0)
                .is_some_and(|t| comments::blank_lines_before(g, &t) > 0);
            if blank {
                any_empty_line = true;
                parts.extend([sep.clone(), hardline(), hardline()]);
            } else {
                parts.extend([sep.clone(), line()]);
            }
        }
        docs.push(doc);
        seps.push(sep);
        printed.push(concat(parts));
    }
    // The trailing comma of a broken list; a comma the source has and a comment sits at stays.
    let trailing = match args.last().and_then(|a| a.1.as_ref()) {
        Some(t) if super::ast::token_comments(cx, t) => cx.token(t),
        _ if dynamic_import => Doc::Nil,
        _ => super::util::trailing_comma(cx, true),
    };
    let all_broken_out = |printed: &[Doc]| {
        group_broken(concat(vec![
            open.clone(),
            indent(concat(vec![
                line(),
                concat(printed.to_vec()),
                trailing.clone(),
            ])),
            line(),
            close.clone(),
        ]))
    };
    let decorator = parent(node).is_some_and(|p| p.kind() == k::DECORATOR);
    if any_empty_line || (!decorator && is_function_composition(cx, &uns)) {
        return Ok(all_broken_out(&printed));
    }
    let group_first = could_group_first(cx, &uns);
    let group_last = should_group_last(cx, &uns);
    if group_first || group_last {
        let concise = if group_first {
            printed[1..].iter().any(will_break)
        } else {
            printed[..printed.len() - 1].iter().any(will_break)
        };
        if concise {
            return Ok(all_broken_out(&printed));
        }
        let at = if group_first { 0 } else { args.len() - 1 };
        let flag = if group_first {
            Flags::EXPAND_FIRST
        } else {
            Flags::EXPAND_LAST
        };
        let Some(expanded) = expanded_arg(cx, &args[at].0, flag)? else {
            return Ok(all_broken_out(&printed));
        };
        let expanded = shared(expanded);
        // The candidates print their separators as spaces: they are tried flat.
        let candidate = |hug: bool| {
            let mut parts = vec![open.clone()];
            for i in 0..args.len() {
                let d = if i == at {
                    if hug {
                        group_broken(expanded.clone())
                    } else {
                        expanded.clone()
                    }
                } else {
                    docs[i].clone()
                };
                parts.push(d);
                if i + 1 < args.len() {
                    parts.extend([seps[i].clone(), lit(" ")]);
                }
            }
            parts.push(close.clone());
            concat(parts)
        };
        let (flat, hugged) = (candidate(false), candidate(true));
        return Ok(concat(vec![
            if printed.iter().any(will_break) {
                break_parent()
            } else {
                Doc::Nil
            },
            conditional_group(vec![flat, hugged, all_broken_out(&printed)]),
        ]));
    }
    let contents = concat(vec![
        open,
        indent(concat(vec![softline(), concat(printed.clone())])),
        trailing,
        softline(),
        close,
    ]);
    if is_long_curried(node) {
        return Ok(contents);
    }
    Ok(if printed.iter().any(will_break) {
        group_broken(contents)
    } else {
        group(contents)
    })
}

/// `a(b)(c)(d)`: the inner call has more arguments than the outer one.
fn is_long_curried(node: &SyntaxNode) -> bool {
    let Some(p) = parent(node) else {
        return false;
    };
    let count = |n: &SyntaxNode| n.children().count();
    p.kind() == k::CALL_EXPRESSION
        && node.kind() == k::CALL_EXPRESSION
        && super::ast::outer(node).parent().as_ref() == Some(&p)
        && p.first_child().as_ref() == Some(&super::ast::outer(node))
        && count(&p) > 1
        && count(node) > count(&p)
}

/// The argument printed for hugging; `None` when its parameters cannot be kept on one line.
fn expanded_arg(
    cx: &FormatCtx<'_>,
    arg: &SyntaxNode,
    flags: u32,
) -> Result<Option<Doc>, FormatError> {
    let a = un(arg);
    match a.kind() {
        k::ARROW_FUNCTION_EXPRESSION | k::FUNCTION => arrow::print_fn(cx, &a, flags),
        _ => Ok(Some(cx.format(arg)?)),
    }
}

/// `useEffect(() => { ... }, [a, b])`.
fn is_react_hook(cx: &FormatCtx<'_>, a: &[SyntaxNode]) -> bool {
    a.len() == 2
        && a[0].kind() == k::ARROW_FUNCTION_EXPRESSION
        && arrow::param_count(&a[0]) == 0
        && a[0]
            .last_child()
            .is_some_and(|b| b.kind() == k::FUNCTION_BODY)
        && a[1].kind() == k::ARRAY_EXPRESSION
        && !a.iter().any(|n| has_comment(cx, n))
}

fn is_function_composition(cx: &FormatCtx<'_>, a: &[SyntaxNode]) -> bool {
    if a.len() <= 1 {
        return false;
    }
    let mut count = 0;
    for n in a {
        let body_block = n.last_child().is_some_and(|b| b.kind() == k::FUNCTION_BODY);
        if n.kind() == k::FUNCTION || (n.kind() == k::ARROW_FUNCTION_EXPRESSION && body_block) {
            count += 1;
            if count > 1 {
                return true;
            }
        } else if matches!(
            n.kind(),
            k::CALL_EXPRESSION | k::NEW_EXPRESSION | k::IMPORT_EXPRESSION
        ) && Call::of(cx, n)
            .is_some_and(|c| c.args.iter().any(|x| is_function(un(&x.0).kind())))
        {
            return true;
        }
    }
    false
}

fn could_group_first(cx: &FormatCtx<'_>, a: &[SyntaxNode]) -> bool {
    let [first, second] = a else {
        return false;
    };
    !has_comment(cx, first)
        && (first.kind() == k::FUNCTION
            || (first.kind() == k::ARROW_FUNCTION_EXPRESSION
                && first
                    .last_child()
                    .is_some_and(|b| b.kind() == k::FUNCTION_BODY)))
        && !matches!(
            second.kind(),
            k::FUNCTION | k::ARROW_FUNCTION_EXPRESSION | k::CONDITIONAL_EXPRESSION
        )
        && is_hopefully_short(cx, second)
        && !could_expand(cx, second, false)
}

fn is_hopefully_short(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    let n = un(n);
    match n.kind() {
        k::BINARY_EXPRESSION | k::LOGICAL_EXPRESSION => {
            n.children().all(|c| is_hopefully_short(cx, &c))
        }
        k::REG_EXP_LITERAL => true,
        _ => is_simple_call_argument(cx, &n, 0),
    }
}

fn should_group_last(cx: &FormatCtx<'_>, a: &[SyntaxNode]) -> bool {
    let Some(last) = a.last() else {
        return false;
    };
    let penultimate = a.len().checked_sub(2).map(|i| &a[i]);
    !has_comment(cx, last)
        && could_expand(cx, last, false)
        && penultimate.is_none_or(|p| p.kind() != last.kind())
        && (a.len() != 2
            || penultimate.is_none_or(|p| {
                p.kind() != k::ARROW_FUNCTION_EXPRESSION || last.kind() != k::ARRAY_EXPRESSION
            }))
        && !(a.len() > 1 && is_concise_array(last))
}

/// An array of two or more numbers, which prints as a fill.
pub(crate) fn is_concise_array(n: &SyntaxNode) -> bool {
    if n.kind() != k::ARRAY_EXPRESSION {
        return false;
    }
    let items = nodes(n);
    items.len() > 1
        && items.iter().all(|i| {
            i.kind() == k::NUMERIC_LITERAL
                || (i.kind() == k::UNARY_EXPRESSION
                    && i.last_child()
                        .is_some_and(|c| c.kind() == k::NUMERIC_LITERAL))
        })
}

#[allow(clippy::only_used_in_recursion)]
pub(crate) fn could_expand(cx: &FormatCtx<'_>, n: &SyntaxNode, arrow_chain: bool) -> bool {
    match n.kind() {
        k::OBJECT_EXPRESSION | k::ARRAY_EXPRESSION => n.children().next().is_some(),
        k::TS_AS_EXPRESSION | k::TS_SATISFIES_EXPRESSION => n
            .first_child()
            .is_some_and(|c| could_expand(cx, &un(&c), false)),
        k::TS_TYPE_ASSERTION => n
            .last_child()
            .is_some_and(|c| could_expand(cx, &un(&c), false)),
        k::FUNCTION => true,
        k::ARROW_FUNCTION_EXPRESSION => {
            let Some(body) = n.last_child() else {
                return false;
            };
            let body_u = un(&body);
            let returns_reference = n.children().any(|c| {
                c.kind() == k::TS_TYPE_ANNOTATION
                    && c.first_child()
                        .is_some_and(|t| t.kind() == k::TS_TYPE_REFERENCE)
            });
            let nonempty_block =
                body_u.kind() == k::FUNCTION_BODY && body_u.children().next().is_some();
            (!returns_reference || nonempty_block)
                && match body_u.kind() {
                    k::FUNCTION_BODY
                    | k::ARROW_FUNCTION_EXPRESSION
                    | k::OBJECT_EXPRESSION
                    | k::ARRAY_EXPRESSION
                    | k::TS_AS_EXPRESSION
                    | k::TS_SATISFIES_EXPRESSION
                    | k::JSX_ELEMENT
                    | k::JSX_FRAGMENT => true,
                    k::CALL_EXPRESSION | k::CONDITIONAL_EXPRESSION => !arrow_chain,
                    _ => false,
                }
        }
        _ => false,
    }
}

/// Prettier's `isSimpleCallArgument`: literals, names, and shallow objects, arrays, calls and
/// member accesses of those.
#[allow(clippy::only_used_in_recursion)]
pub(crate) fn is_simple_call_argument(cx: &FormatCtx<'_>, n: &SyntaxNode, depth: u32) -> bool {
    let n = un(n);
    let simple = |c: &SyntaxNode| is_simple_call_argument(cx, c, depth + 1);
    match n.kind() {
        // A pattern of up to five characters.
        k::REG_EXP_LITERAL => n.text().to_string().rfind('/').is_some_and(|end| end <= 6),
        kind if is_literal(kind) => true,
        k::IDENTIFIER_REFERENCE
        | k::THIS_EXPRESSION
        | k::SUPER
        | k::PRIVATE_IDENTIFIER
        | k::IDENTIFIER_NAME => true,
        k::TEMPLATE_LITERAL => {
            !n.text().to_string().contains('\n') && n.children().all(|c| simple(&c))
        }
        k::OBJECT_EXPRESSION => n.children().all(|p| {
            p.kind() == k::OBJECT_PROPERTY
                && !p
                    .children_with_tokens()
                    .any(|e| e.as_token().is_some_and(|t| t.kind() == k::L_BRACKET))
                && (p.children().count() == 1 || p.last_child().is_some_and(|v| simple(&v)))
        }),
        k::ARRAY_EXPRESSION => n.children().all(|c| c.kind() == k::ELISION || simple(&c)),
        k::IMPORT_EXPRESSION => n.children().all(|c| simple(&c)),
        k::CALL_EXPRESSION | k::NEW_EXPRESSION => {
            let Some(c) = Call::of(cx, &n) else {
                return false;
            };
            is_simple_call_argument(cx, &c.callee, depth) && c.args.iter().all(|a| simple(&a.0))
        }
        k::STATIC_MEMBER_EXPRESSION | k::PRIVATE_FIELD_EXPRESSION => n
            .first_child()
            .is_some_and(|o| is_simple_call_argument(cx, &o, depth)),
        k::COMPUTED_MEMBER_EXPRESSION => {
            n.children().all(|c| is_simple_call_argument(cx, &c, depth))
        }
        k::UNARY_EXPRESSION => {
            let op = super::ast::operator_text(cx, &n);
            matches!(op.as_str(), "!" | "-" | "+" | "~")
                && n.last_child()
                    .is_some_and(|a| is_simple_call_argument(cx, &a, depth))
        }
        k::UPDATE_EXPRESSION | k::TS_NON_NULL_EXPRESSION | k::CHAIN_EXPRESSION => n
            .first_child()
            .is_some_and(|a| is_simple_call_argument(cx, &a, depth)),
        _ => false,
    }
}

/// A template literal with a line break that starts on the line of the code before it.
pub(crate) fn template_on_own_line(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    let template = match n.kind() {
        k::TEMPLATE_LITERAL => n.clone(),
        k::TAGGED_TEMPLATE_EXPRESSION => match n.last_child() {
            Some(t) if t.kind() == k::TEMPLATE_LITERAL => t,
            _ => return false,
        },
        _ => return false,
    };
    if !template.text().to_string().contains('\n') {
        return false;
    }
    let Some(first) = cx.first_token(n) else {
        return false;
    };
    let mut prev = first.prev_token();
    while let Some(t) = prev {
        if t.kind() == k::NEWLINE {
            return false;
        }
        if !cx.grammar.is_trivia(t.kind()) {
            break;
        }
        prev = t.prev_token();
    }
    true
}

/// `require("x")`, `define([...], function () {})`.
fn is_common_js_or_amd(cx: &FormatCtx<'_>, node: &SyntaxNode, c: &Call) -> bool {
    let Some(name) = super::ast::name_of(cx, &c.callee) else {
        return false;
    };
    match name.as_str() {
        "require" => c.args.len() == 1 && un(&c.args[0].0).kind() == k::STRING_LITERAL,
        "define" => {
            parent(node).is_some_and(|p| p.kind() == k::EXPRESSION_STATEMENT)
                && (c.args.len() == 1
                    || (c.args.len() == 2 && un(&c.args[1].0).kind() == k::ARRAY_EXPRESSION)
                    || (c.args.len() >= 2 && un(&c.args[0].0).kind() == k::ARRAY_EXPRESSION))
        }
        _ => false,
    }
}

/// `it("does x", () => {})`, `describe.only(...)`: kept on one line whatever the width.
fn is_test_call(cx: &FormatCtx<'_>, c: &Call) -> bool {
    if c.optional.is_some() {
        return false;
    }
    let a: Vec<SyntaxNode> = c.args.iter().map(|x| un(&x.0)).collect();
    let callee_path = callee_path(cx, &c.callee);
    match a.len() {
        1 => {
            let unit = matches!(
                callee_path.as_deref(),
                Some("beforeEach" | "beforeAll" | "afterEach" | "afterAll")
            );
            unit && is_angular_wrapper(cx, &a[0])
        }
        2 | 3 => {
            let named = matches!(a[0].kind(), k::TEMPLATE_LITERAL | k::STRING_LITERAL);
            if !named
                || !callee_path
                    .as_deref()
                    .is_some_and(|p| TEST_CALLEES.contains(&p))
            {
                return false;
            }
            if a.len() == 3 && a[2].kind() != k::NUMERIC_LITERAL {
                return false;
            }
            if a.len() == 2 {
                is_function(a[1].kind()) || is_angular_wrapper(cx, &a[1])
            } else {
                (is_function(a[1].kind())
                    && a[1]
                        .last_child()
                        .is_some_and(|b| b.kind() == k::FUNCTION_BODY)
                    && arrow::param_count(&a[1]) <= 1)
                    || is_angular_wrapper(cx, &a[1])
            }
        }
        _ => false,
    }
}

const TEST_CALLEES: &[&str] = &[
    "it",
    "it.only",
    "it.skip",
    "describe",
    "describe.only",
    "describe.skip",
    "test",
    "test.only",
    "test.skip",
    "test.step",
    "test.describe",
    "test.describe.only",
    "test.describe.parallel",
    "test.describe.parallel.only",
    "test.describe.serial",
    "test.describe.serial.only",
    "skip",
    "xit",
    "xdescribe",
    "xtest",
    "fit",
    "fdescribe",
    "ftest",
];

/// `async(() => {})`, `inject(...)`, `fakeAsync(...)`, `waitForAsync(...)`.
fn is_angular_wrapper(cx: &FormatCtx<'_>, n: &SyntaxNode) -> bool {
    n.kind() == k::CALL_EXPRESSION
        && Call::of(cx, n).is_some_and(|c| {
            c.args.len() == 1
                && is_function(un(&c.args[0].0).kind())
                && super::ast::name_of(cx, &c.callee).is_some_and(|name| {
                    matches!(
                        name.as_str(),
                        "async" | "inject" | "fakeAsync" | "waitForAsync"
                    )
                })
        })
}

/// `a.b.c` of an identifier or static member chain, as text.
fn callee_path(cx: &FormatCtx<'_>, n: &SyntaxNode) -> Option<String> {
    let n = un(n);
    match n.kind() {
        k::IDENTIFIER_REFERENCE => super::ast::name_of(cx, &n),
        k::STATIC_MEMBER_EXPRESSION => {
            let obj = callee_path(cx, &n.first_child()?)?;
            let prop = super::ast::name_of(cx, &n.last_child()?)?;
            Some(format!("{obj}.{prop}"))
        }
        _ => None,
    }
}

/// The tab width, for the short-name check of member chains.
pub(crate) fn tab_width(cx: &FormatCtx<'_>) -> usize {
    match cx.options.get("indent_size") {
        Some(OptionValue::Int(n)) => usize::try_from(*n).unwrap_or(2),
        _ => 2,
    }
}

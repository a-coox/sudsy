//! typescript-eslint rules that read the syntax tree alone and that a pattern cannot say: a
//! comparison of two nodes, a count, a name taken from the parent, the comments of the file.

use super::dupes::key_name;
use super::{Check, code_text, code_token, has_own, operator, span, un};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange};

const LINE_BREAKS: [char; 4] = ['\n', '\r', '\u{2028}', '\u{2029}'];

fn comments(root: &SyntaxNode) -> impl Iterator<Item = core_syntax::SyntaxToken> {
    (root.descendants_with_tokens())
        .filter_map(|e| e.into_token())
        .filter(|t| matches!(t.kind(), k::LINE_COMMENT | k::BLOCK_COMMENT))
}

/// `(directive, description)` of text that starts with `@ts-<one of names>`.
fn at_ts<'a>(s: &'a str, names: &[&'static str]) -> Option<(&'static str, &'a str)> {
    let rest = s.strip_prefix("@ts-")?;
    names
        .iter()
        .find_map(|n| rest.strip_prefix(n).map(|d| (*n, d)))
}

/// The `@ts-` directive a comment holds: `//` and `///` comments also hold the pragmas
/// (`check`, `nocheck`); a block comment is read on its last line.
fn directive(text: &str) -> Option<(&'static str, &str)> {
    if let Some(line) = text.strip_prefix("//") {
        let pragma = line.strip_prefix('/').unwrap_or(line).trim_start();
        return at_ts(pragma, &["check", "nocheck"]).or_else(|| {
            at_ts(
                line.trim_start_matches('/').trim_start(),
                &["expect-error", "ignore"],
            )
        });
    }
    let inner = text.strip_prefix("/*")?.trim_end_matches("*/");
    let last = inner.rsplit(LINE_BREAKS).next()?;
    at_ts(
        last.trim_start()
            .trim_start_matches(['/', '*'])
            .trim_start(),
        &["expect-error", "ignore"],
    )
}

/// `@ts-ignore`, `@ts-nocheck` before the first statement, `@ts-expect-error` without a
/// description of three characters (ts-check is allowed).
pub(super) fn ban_ts_comment(_: &Check, root: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let first_line = (root.children().next()).map(|s| ctx.file.lines.line(span(ctx, &s).start()));
    for comment in comments(root) {
        let text = comment.text();
        let Some((name, description)) = directive(text) else {
            continue;
        };
        let message = match name {
            "ignore" => {
                "Use \"@ts-expect-error\" instead of \"@ts-ignore\", as \"@ts-ignore\" will do nothing if the following line is error-free."
            }
            "nocheck"
                if first_line
                    .is_none_or(|l| l > ctx.file.lines.line(comment.text_range().start())) =>
            {
                "Do not use \"@ts-nocheck\" because it alters compilation errors."
            }
            "expect-error" if description.trim().chars().count() < 3 => {
                "Include a description after the \"@ts-expect-error\" directive to explain why the @ts-expect-error is necessary. The description must be 3 characters or longer."
            }
            _ => continue,
        };
        ctx.report(comment.text_range(), message);
    }
}

/// `a!!`, `a!?.b`, `a!?.()`
pub(super) fn extra_non_null(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(parent) = n.parent() else { return };
    let chained = matches!(
        parent.kind(),
        k::CALL_EXPRESSION
            | k::STATIC_MEMBER_EXPRESSION
            | k::COMPUTED_MEMBER_EXPRESSION
            | k::PRIVATE_FIELD_EXPRESSION
    ) && parent.children().next().as_ref() == Some(n)
        && std::iter::successors(n.next_sibling_or_token(), |e| e.next_sibling_or_token())
            .find(|e| !ctx.file.grammar.is_trivia(e.kind()))
            .is_some_and(|e| e.kind() == k::QUESTION_DOT);
    if chained || parent.kind() == k::TS_NON_NULL_EXPRESSION {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// The name a type annotation refers to (`: Foo`, not `: a.Foo` or `: Foo[]`).
fn annotated_name(node: Option<SyntaxNode>, g: &Grammar) -> Option<String> {
    let reference = (node?.children().find(|c| c.kind() == k::TS_TYPE_ANNOTATION))?
        .children()
        .next()
        .filter(|t| t.kind() == k::TS_TYPE_REFERENCE)?
        .children()
        .next()
        .filter(|n| n.kind() == k::IDENTIFIER_REFERENCE)?;
    Some(code_text(&reference, g))
}

/// The name of the class or interface whose body holds `member`.
fn owner_name(member: &SyntaxNode, g: &Grammar) -> Option<String> {
    let id = (member.parent()?.parent()?)
        .children()
        .find(|c| c.kind() == k::BINDING_IDENTIFIER)?;
    Some(code_text(&id, g))
}

/// A class method `new(): C`, an interface `new (): I` and an interface method `constructor()`.
pub(super) fn misused_new(_: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let named = |name: &str| {
        (n.children().next())
            .is_some_and(|key| key.kind() == k::IDENTIFIER_NAME && code_text(&key, g) == name)
    };
    let message = match n.kind() {
        k::TS_METHOD_SIGNATURE if named("constructor") => {
            "Interfaces cannot be constructed, only classes."
        }
        k::TS_CONSTRUCT_SIGNATURE_DECLARATION
            if n.parent().is_some_and(|p| p.kind() == k::TS_INTERFACE_BODY)
                && annotated_name(Some(n.clone()), g)
                    .is_some_and(|t| Some(t) == owner_name(n, g)) =>
        {
            "Interfaces cannot be constructed, only classes."
        }
        k::METHOD_DEFINITION if named("new") => {
            let Some(function) = n.children().find(|c| c.kind() == k::FUNCTION) else {
                return;
            };
            // a body means a method that happens to be called `new`
            if function.children().any(|c| c.kind() == k::FUNCTION_BODY)
                || annotated_name(Some(function), g).is_none_or(|t| Some(t) != owner_name(n, g))
            {
                return;
            }
            "Class cannot have method named `new`."
        }
        _ => return,
    };
    ctx.report(span(ctx, n), message);
}

fn is_literal(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        k::STRING_LITERAL
            | k::NUMERIC_LITERAL
            | k::BOOLEAN_LITERAL
            | k::NULL_LITERAL
            | k::BIG_INT_LITERAL
            | k::REG_EXP_LITERAL
    )
}

/// `let a: 'a' = 'a'`, `x as 'a'` on the same literal.
pub(super) fn prefer_as_const(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let Some(literal) = n.children().next().filter(|l| is_literal(l.kind())) else {
        return;
    };
    let Some(parent) = n.parent() else { return };
    let value = match parent.kind() {
        k::TS_AS_EXPRESSION => parent.children().next(),
        k::TS_TYPE_ASSERTION => parent.children().nth(1),
        k::TS_TYPE_ANNOTATION => parent
            .parent()
            .filter(|p| matches!(p.kind(), k::VARIABLE_DECLARATOR | k::PROPERTY_DEFINITION))
            .and_then(|_| parent.next_sibling()),
        _ => None,
    };
    if value.is_some_and(|v| v != *n && code_text(&un(v), g) == code_text(&literal, g)) {
        c.hit(ctx, span(ctx, n), "");
    }
}

/// The expression of a call of the global `name`, through an optional chain.
fn is_call(n: &SyntaxNode, name: &str, g: &Grammar) -> bool {
    let n = match n.kind() {
        k::CHAIN_EXPRESSION => n.children().next(),
        _ => Some(n.clone()),
    };
    n.filter(|n| n.kind() == k::CALL_EXPRESSION)
        .and_then(|n| n.children().next())
        .is_some_and(|f| f.kind() == k::IDENTIFIER_REFERENCE && code_text(&f, g) == name)
}

fn is_name(n: &SyntaxNode, names: &[&str], g: &Grammar) -> bool {
    n.kind() == k::IDENTIFIER_REFERENCE && names.contains(&code_text(n, g).as_str())
}

/// The operand of a unary expression whose operator is one of `ops`.
fn operand(n: &SyntaxNode, ops: &[&str], g: &Grammar) -> Option<SyntaxNode> {
    (n.kind() == k::UNARY_EXPRESSION && ops.contains(&operator(n, g).as_str()))
        .then(|| n.children().next().map(un))?
}

/// Does `value` of `ty`'s kind tell its type by itself?
fn inferrable(ty: &SyntaxNode, value: SyntaxNode, g: &Grammar) -> bool {
    let value = un(value);
    let literal = |v: &SyntaxNode, kind| v.kind() == kind;
    match ty.kind() {
        k::TS_BIG_INT_KEYWORD => {
            let v = operand(&value, &["-"], g).unwrap_or(value);
            is_call(&v, "BigInt", g) || is_literal(v.kind())
        }
        k::TS_BOOLEAN_KEYWORD => {
            operand(&value, &["!"], g).is_some()
                || is_call(&value, "Boolean", g)
                || literal(&value, k::BOOLEAN_LITERAL)
        }
        k::TS_NUMBER_KEYWORD => {
            let v = operand(&value, &["+", "-"], g).unwrap_or(value);
            is_name(&v, &["Infinity", "NaN"], g)
                || is_call(&v, "Number", g)
                || literal(&v, k::NUMERIC_LITERAL)
        }
        k::TS_NULL_KEYWORD => literal(&value, k::NULL_LITERAL),
        k::TS_STRING_KEYWORD => {
            is_call(&value, "String", g)
                || matches!(value.kind(), k::STRING_LITERAL | k::TEMPLATE_LITERAL)
        }
        k::TS_SYMBOL_KEYWORD => is_call(&value, "Symbol", g),
        k::TS_UNDEFINED_KEYWORD => {
            operand(&value, &["void"], g).is_some() || is_name(&value, &["undefined"], g)
        }
        k::TS_TYPE_REFERENCE => {
            ty.children()
                .next()
                .is_some_and(|t| code_text(&t, g) == "RegExp")
                && (literal(&value, k::REG_EXP_LITERAL)
                    || is_call(&value, "RegExp", g)
                    || (value.kind() == k::NEW_EXPRESSION
                        && value
                            .children()
                            .next()
                            .is_some_and(|f| is_name(&f, &["RegExp"], g))))
        }
        _ => false,
    }
}

/// `const a: number = 5`, a parameter `b: string = "x"`, a property `c: boolean = true`
/// (not `readonly` or optional ones).
pub(super) fn inferrable_types(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    if matches!(n.kind(), k::PROPERTY_DEFINITION | k::ACCESSOR_PROPERTY)
        && (has_own(n, g, "readonly") || has_own(n, g, "?"))
    {
        return;
    }
    let Some(annotation) = n.children().find(|a| a.kind() == k::TS_TYPE_ANNOTATION) else {
        return;
    };
    let mut seen_equals = false;
    let mut value = None;
    for e in n.children_with_tokens() {
        match e {
            core_syntax::SyntaxElement::Token(t) if t.kind() == k::OPERATOR && t.text() == "=" => {
                seen_equals = true
            }
            core_syntax::SyntaxElement::Node(v) if seen_equals => {
                value = Some(v);
                break;
            }
            _ => {}
        }
    }
    let (Some(value), Some(ty)) = (value, annotation.children().next()) else {
        return;
    };
    if inferrable(&ty, value, g) {
        // a parameter property starts at its name, after the modifiers
        let first = (n.kind() == k::FORMAL_PARAMETER)
            .then(|| n.children().find(|c| c.kind() != k::DECORATOR))
            .flatten();
        let start = span(ctx, first.as_ref().unwrap_or(n)).start();
        let name = if ty.kind() == k::TS_TYPE_REFERENCE {
            "RegExp".into()
        } else {
            code_text(&ty, g)
        };
        c.hit(ctx, TextRange::new(start, span(ctx, n).end()), &name);
    }
}

/// `Array<T>`, `ReadonlyArray<T>`, `Readonly<T[]>` where `T[]` is the form to write.
pub(super) fn array_type(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let Some(name) = n
        .children()
        .next()
        .filter(|c| c.kind() == k::IDENTIFIER_REFERENCE)
    else {
        return;
    };
    let Some(arguments) = n
        .children()
        .find(|c| c.kind() == k::TS_TYPE_PARAMETER_INSTANTIATION)
    else {
        return;
    };
    let mut arguments = arguments.children();
    let (Some(only), None) = (arguments.next(), arguments.next()) else {
        return;
    };
    let name = code_text(&name, g);
    if matches!(name.as_str(), "Array" | "ReadonlyArray")
        || (name == "Readonly" && only.kind() == k::TS_ARRAY_TYPE)
    {
        c.hit(ctx, span(ctx, n), &name);
    }
}

/// The comments before the first token of the file that point at a module: the line `/// <reference
/// types="x" />` as `(kind, module)`.
fn references(
    root: &SyntaxNode,
    g: &Grammar,
) -> Vec<(core_syntax::SyntaxToken, &'static str, String)> {
    let end = code_token(root, g).map(|t| t.text_range().start());
    let mut out = Vec::new();
    for t in comments(root)
        .filter(|t| t.kind() == k::LINE_COMMENT && end.is_none_or(|e| t.text_range().start() < e))
    {
        let rest = (|| {
            let rest = t
                .text()
                .strip_prefix("///")?
                .trim_start()
                .strip_prefix("<reference")?
                .trim_start();
            let (kind, rest) = ["types", "path", "lib"]
                .iter()
                .find_map(|k| rest.strip_prefix(k).map(|r| (*k, r)))?;
            let rest = rest
                .trim_start()
                .strip_prefix('=')?
                .trim_start()
                .strip_prefix(['"', '|', '\''])?;
            Some((kind, rest[..rest.rfind(['"', '|', '\''])?].to_string()))
        })();
        if let Some((kind, module)) = rest {
            out.push((t, kind, module));
        }
    }
    out
}

/// `/// <reference path="x" />`, and `types` for a module the file also imports.
pub(super) fn triple_slash_reference(c: &Check, root: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let g = ctx.file.grammar;
    let found = references(root, g);
    let mut types = Vec::new();
    for (comment, kind, module) in found {
        match kind {
            "path" => {
                c.hit(ctx, comment.text_range(), &module);
            }
            "types" => types.push((comment, module)),
            _ => {}
        }
    }
    if types.is_empty() {
        return;
    }
    let sources: Vec<String> = (root.descendants())
        .filter(|n| {
            matches!(
                n.kind(),
                k::IMPORT_DECLARATION | k::TS_EXTERNAL_MODULE_REFERENCE
            )
        })
        .filter_map(|n| n.children().find(|s| s.kind() == k::STRING_LITERAL))
        .map(|s| {
            let text = code_text(&s, g);
            text[1..text.len() - 1].to_string()
        })
        .collect();
    for (comment, module) in types {
        if sources.contains(&module) {
            c.hit(ctx, comment.text_range(), &module);
        }
    }
}

#[derive(Clone, PartialEq)]
struct Signature {
    name: String,
    call: bool,
    is_static: bool,
}

/// A function or method of a body, by the name its overloads share.
fn signature(n: &SyntaxNode, ctx: &RuleContext<'_>) -> Option<Signature> {
    let g = ctx.file.grammar;
    let plain = |name: String, call| Signature {
        name,
        call,
        is_static: false,
    };
    match n.kind() {
        k::EXPORT_DECLARATION | k::EXPORT_DEFAULT_DECLARATION | k::EXPORT_NAMED_DECLARATION => {
            signature(&n.children().next()?, ctx)
        }
        k::FUNCTION => {
            let id = n.children().find(|c| c.kind() == k::BINDING_IDENTIFIER)?;
            Some(plain(code_text(&id, g), false))
        }
        k::TS_CALL_SIGNATURE_DECLARATION => Some(plain("call".into(), true)),
        k::TS_CONSTRUCT_SIGNATURE_DECLARATION => Some(plain("new".into(), false)),
        k::METHOD_DEFINITION | k::TS_METHOD_SIGNATURE => {
            let key = n.children().find(|c| c.kind() != k::DECORATOR)?;
            let name = match key_name(&key, ctx.file.text) {
                Some(name)
                    if key.kind() == k::STRING_LITERAL
                        && !name
                            .chars()
                            .all(|c| c.is_alphanumeric() || c == '_' || c == '$') =>
                {
                    format!("\"{name}\"")
                }
                Some(name) => name,
                None => code_text(&key, g),
            };
            Some(Signature {
                is_static: has_own(n, g, "static"),
                ..plain(name, false)
            })
        }
        _ => None,
    }
}

/// Overloads of one name that are not next to each other.
pub(super) fn adjacent_overloads(c: &Check, body: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut seen: Vec<Signature> = Vec::new();
    let mut last: Option<Signature> = None;
    for member in body.children() {
        let Some(current) = signature(&member, ctx) else {
            last = None;
            continue;
        };
        if !seen.contains(&current) {
            seen.push(current.clone());
        } else if last.as_ref() != Some(&current) {
            let name = format!(
                "{}{}",
                if current.is_static { "static " } else { "" },
                current.name
            );
            c.hit(ctx, span(ctx, &member), &name);
        }
        last = Some(current);
    }
}

//! What the shared rules ask of TOML: a key is its parts decoded and joined with `.`, and string
//! and number items of an array have their value.

use crate::generated::{Literal, kind};
use core_rules::{RuleHooks, Scalar};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use rowan::{GreenNodeData, GreenTokenData, NodeOrToken};
use std::borrow::Cow;

pub struct TomlHooks;

fn text_of<'a>(token: &SyntaxToken, text: &'a str) -> &'a str {
    let range = token.text_range();
    &text[usize::from(range.start())..usize::from(range.end())]
}

/// The value of a single-line quoted string token: `'literal'` as written, `"basic"` with its
/// escapes decoded (an escape TOML does not define is kept as written).
fn unquote(raw: &str) -> Cow<'_, str> {
    let inner = raw.get(1..raw.len().saturating_sub(1)).unwrap_or(raw);
    if raw.starts_with('\'') || !inner.contains('\\') {
        return Cow::Borrowed(inner);
    }
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('u') => out.push(hex(&mut chars, 4)),
            Some('U') => out.push(hex(&mut chars, 8)),
            Some(e) => out.push(match e {
                'b' => '\u{8}',
                't' => '\t',
                'n' => '\n',
                'f' => '\u{c}',
                'r' => '\r',
                other => other,
            }),
            None => out.push('\\'),
        }
    }
    Cow::Owned(out)
}

/// The character of the next `digits` hex digits; U+FFFD when they are not a character.
fn hex(chars: &mut std::str::Chars<'_>, digits: usize) -> char {
    (0..digits)
        .try_fold(0u32, |v, _| Some(v * 16 + chars.next()?.to_digit(16)?))
        .and_then(char::from_u32)
        .unwrap_or('\u{FFFD}')
}

/// The value of an integer token (`1_000`, `0xff`, `0o17`, `0b11`).
fn integer(raw: &str) -> Option<f64> {
    let digits = raw.replace('_', "");
    let radix = |prefix: &str, base| i128::from_str_radix(digits.strip_prefix(prefix)?, base).ok();
    radix("0x", 16)
        .or_else(|| radix("0o", 8))
        .or_else(|| radix("0b", 2))
        .or_else(|| digits.parse::<i128>().ok())
        .map(|i| i as f64)
}

/// A key written only with bare parts (`a`, `a.b`): its text, which is also its name.
fn bare_key<'a>(key: &SyntaxNode, text: &'a str) -> Option<&'a str> {
    let range = key.text_range();
    let raw = &text[usize::from(range.start())..usize::from(range.end())];
    raw.split('.')
        .all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .then_some(raw)
}

/// The tokens under a green node with their start offsets.
pub fn leaves(node: &GreenNodeData, start: u32) -> impl Iterator<Item = (u32, &GreenTokenData)> {
    let (mut stack, mut at) = (vec![node.children()], start);
    std::iter::from_fn(move || {
        loop {
            match stack.last_mut()?.next() {
                None => {
                    stack.pop();
                }
                Some(NodeOrToken::Node(n)) => stack.push(n.children()),
                Some(NodeOrToken::Token(t)) => {
                    at += u32::from(t.text_len());
                    return Some((at - u32::from(t.text_len()), t));
                }
            }
        }
    })
}

/// Replaces `out` with the decoded parts of the key at `start` (`a."b.c".d` has three); returns
/// where the last one ends.
pub fn key_parts<'a>(
    key: &GreenNodeData,
    start: u32,
    text: &'a str,
    out: &mut Vec<Cow<'a, str>>,
) -> u32 {
    let mut end = start;
    out.clear();
    out.extend(leaves(key, start).filter_map(|(at, t)| {
        let to = at + u32::from(t.text_len());
        let raw = &text[at as usize..to as usize];
        let part = match SyntaxKind(t.kind().0) {
            kind::QUOTED_KEY => unquote(raw),
            kind::BARE_KEY => Cow::Borrowed(raw),
            _ => return None,
        };
        end = to;
        Some(part)
    }));
    end
}

fn parts<'a>(key: &SyntaxNode, text: &'a str) -> Vec<Cow<'a, str>> {
    let mut out = Vec::new();
    key_parts(
        &key.green(),
        key.text_range().start().into(),
        text,
        &mut out,
    );
    out
}

impl RuleHooks for TomlHooks {
    fn key_parts<'a>(&self, key: &SyntaxElement, text: &'a str) -> Option<Vec<Cow<'a, str>>> {
        let SyntaxElement::Node(key) = key else {
            return None;
        };
        if let Some(bare) = bare_key(key, text) {
            return bare
                .contains('.')
                .then(|| bare.split('.').map(Cow::Borrowed).collect());
        }
        Some(parts(key, text))
    }

    fn key_name<'a>(&self, key: &SyntaxElement, text: &'a str) -> Cow<'a, str> {
        let SyntaxElement::Node(key) = key else {
            return Cow::Borrowed("");
        };
        if let Some(plain) = bare_key(key, text) {
            return Cow::Borrowed(plain);
        }
        let mut parts = parts(key, text).into_iter();
        let (Some(first), second) = (parts.next(), parts.next()) else {
            return Cow::Borrowed("");
        };
        let Some(second) = second else {
            return first;
        };
        let mut joined = format!("{first}.{second}");
        for part in parts {
            joined.push('.');
            joined.push_str(&part);
        }
        Cow::Owned(joined)
    }

    fn scalar<'a>(&self, item: &SyntaxNode, text: &'a str) -> Option<Scalar<'a>> {
        let token = Literal::cast(item.clone())?.value()?;
        let raw = text_of(&token, text);
        match token.kind() {
            // Multi-line strings are left alone.
            kind::STRING if !raw.starts_with("\"\"\"") && !raw.starts_with("'''") => {
                Some(Scalar::String(unquote(raw)))
            }
            kind::INTEGER => integer(raw).map(Scalar::Number),
            kind::FLOAT => raw.replace('_', "").parse().ok().map(Scalar::Number),
            _ => None,
        }
    }
}

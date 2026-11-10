//! `invalid-expression`: the syntax of `${{ }}` expressions, and names that are no context or
//! function. Types, and which contexts exist where, are not checked.

use core_rules::rule;
use core_rules::{FileContext, RuleContext};
use core_syntax::{TextRange, TextSize};
use lang_yaml::generated::kind;

const CONTEXTS: &[&str] = &[
    "github", "env", "vars", "job", "jobs", "steps", "runner", "secrets", "strategy", "matrix",
    "needs", "inputs", // Literals.
    "true", "false", "null", "nan", "infinity",
];
const FUNCTIONS: &[&str] = &[
    "contains",
    "startswith",
    "endswith",
    "format",
    "join",
    "tojson",
    "fromjson",
    "hashfiles",
    "success",
    "always",
    "cancelled",
    "failure",
];

/// The end of the expression that starts at `from` (after `${{`): the offset of its `}}`.
fn end_of(body: &str, from: usize) -> Option<usize> {
    let bytes = body.as_bytes();
    let mut i = from;
    while i < bytes.len() {
        match bytes[i] {
            b'}' if bytes.get(i + 1) == Some(&b'}') => return Some(i),
            b'\'' => i = end_of_string(bytes, i)?,
            _ => i += 1,
        }
    }
    None
}

/// The first problem of the expression `body` as `(offset, message)`.
fn problem(body: &str) -> Option<(usize, String)> {
    if body.trim().is_empty() {
        return Some((0, "empty expression".into()));
    }
    let bytes = body.as_bytes();
    let mut open: Vec<(u8, usize)> = Vec::new();
    let (mut i, mut after_dot) = (0, false);
    while i < bytes.len() {
        let c = bytes[i];
        let start = i;
        let word = |i: &mut usize, ok: fn(u8) -> bool| {
            while *i < bytes.len() && ok(bytes[*i]) {
                *i += 1;
            }
        };
        match c {
            b' ' | b'\t' | b'\n' | b'\r' => i += 1,
            b'\'' => {
                i = end_of_string(bytes, i)?;
                after_dot = false;
            }
            b'(' | b'[' => {
                open.push((c, i));
                i += 1;
                after_dot = false;
            }
            b')' | b']' => {
                let want = if c == b')' { b'(' } else { b'[' };
                if open.pop().map(|o| o.0) != Some(want) {
                    return Some((i, format!("unbalanced `{}`", c as char)));
                }
                i += 1;
                after_dot = false;
            }
            b'.' => {
                after_dot = true;
                i += 1;
            }
            b',' | b'!' | b'<' | b'>' | b'=' | b'&' | b'|' | b'*' => {
                i += 1;
                after_dot = false;
            }
            b'0'..=b'9' | b'-' if c != b'-' || bytes.get(i + 1).is_some_and(u8::is_ascii_digit) => {
                i += 1;
                word(&mut i, |b| {
                    b.is_ascii_alphanumeric() || b == b'.' || b == b'_'
                });
                after_dot = false;
            }
            b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                word(&mut i, |b| {
                    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
                });
                let name = body[start..i].to_lowercase();
                let call = body[i..].trim_start().starts_with('(');
                if call && !after_dot && !FUNCTIONS.contains(&name.as_str()) {
                    return Some((start, format!("unknown function `{}`", &body[start..i])));
                }
                if !call && !after_dot && !CONTEXTS.contains(&name.as_str()) {
                    return Some((start, format!("unknown context `{}`", &body[start..i])));
                }
                after_dot = false;
            }
            _ => return Some((i, format!("unexpected `{}`", body[i..].chars().next()?))),
        }
    }
    if after_dot {
        return Some((bytes.len(), "the expression ends after `.`".into()));
    }
    open.pop()
        .map(|(c, at)| (at, format!("unclosed `{}`", c as char)))
}

/// The offset after the string literal that starts at `i`; `None` when it is not closed (the
/// caller found the expression's end, so that cannot happen).
fn end_of_string(bytes: &[u8], mut i: usize) -> Option<usize> {
    i += 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                i += 1;
            } else {
                return Some(i + 1);
            }
        }
        i += 1;
    }
    None
}

rule! {
    pub InvalidExpression {
        id: "invalid-expression",
        category: Correctness,
        default: Error,
        description: "A `${{ }}` expression that is not closed or empty, has unbalanced brackets or \
                  a string that is not closed, uses a character expressions do not have (such as a \
                  double quote), or names a context or function that does not exist.",
        example: "name: ${{ github.sha }\nenv:\n  A: ${{ foo.bar }}   # reported",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for token in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if !matches!(
                token.kind(),
                kind::PLAIN | kind::SINGLE_QUOTED | kind::DOUBLE_QUOTED | kind::BLOCK_CONTENT
            ) {
                continue;
            }
            let (start, body) = (usize::from(token.text_range().start()), &file.text[token.text_range()]);
            let span = |at: usize, len: usize| {
                let pos = |i: usize| TextSize::try_from(start + i).expect("a file is under 4 GiB");
                TextRange::new(pos(at), pos(at + len))
            };
            let mut from = 0;
            while let Some(open) = body[from..].find("${{").map(|i| i + from) {
                let Some(close) = end_of(body, open + 3) else {
                    ctx.report(span(open, 3), "the expression is not closed with `}}`");
                    break;
                };
                if let Some((at, message)) = problem(&body[open + 3..close]) {
                    let at = open + 3 + at;
                    let len = body[at..].chars().next().map_or(1, char::len_utf8);
                    ctx.report(span(at, len), message);
                }
                from = close + 2;
            }
        }
    }
}

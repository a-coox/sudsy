//! TOML 1.0 rules the grammar does not express, reported as parse errors (a document that breaks
//! them is not TOML, like a syntax error): string escapes and control characters, date and time
//! ranges, a missing line break between entries, and the table model (duplicate keys, redefined
//! tables, tables and arrays of tables that conflict, extending an inline table or a value).

use crate::generated::kind;
use crate::hooks::key_parts;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};
use provider_api::ParseError;
use rowan::{GreenNodeData as Green, GreenTokenData, NodeOrToken};
use std::borrow::Cow;
use std::collections::HashMap;

type Err<'a> = dyn FnMut(TextRange, &str, String) + 'a;

fn kind_of(n: &Green) -> SyntaxKind {
    SyntaxKind(n.kind().0)
}

/// The child nodes of `node` (at `start`) with their start offsets.
fn nodes(node: &Green, start: u32) -> impl Iterator<Item = (u32, &Green)> {
    let mut at = start;
    node.children().filter_map(move |c| match c {
        NodeOrToken::Node(n) => {
            at += u32::from(n.text_len());
            Some((at - u32::from(n.text_len()), n))
        }
        NodeOrToken::Token(t) => {
            at += u32::from(t.text_len());
            None
        }
    })
}

/// The tokens under a green node with their start offsets.
fn leaves(node: &Green, start: u32) -> impl Iterator<Item = (u32, &GreenTokenData)> {
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

/// Every violation in the tree of a document without syntax errors. Walks the green tree: red
/// nodes would cost an allocation each.
pub fn validate(root: &SyntaxNode, text: &str) -> Vec<ParseError> {
    let mut out = Vec::new();
    let mut err = |range: TextRange, code: &str, message: String| {
        out.push(ParseError {
            start: u32::from(range.start()),
            len: u32::from(range.len()).max(1),
            code: code.into(),
            message,
        });
    };
    let green = root.green();
    for (at, t) in leaves(&green, 0) {
        let range = TextRange::at(at.into(), t.text_len());
        match SyntaxKind(t.kind().0) {
            kind::STRING | kind::QUOTED_KEY => {
                if let Err(m) = string(t.text()) {
                    err(range, "invalid-string", m);
                }
            }
            kind::DATE_TIME => {
                if let Err(m) = date_time(t.text()) {
                    err(range, "invalid-date", m);
                }
            }
            _ => {}
        }
    }
    let mut model = Model::new();
    let mut current = 0;
    for (at, item) in nodes(&green, 0) {
        match kind_of(item) {
            kind::PAIR => {
                line_end(item, at, text, &mut err);
                model.pair(current, item, at, text, &mut err);
            }
            kind::TABLE | kind::ARRAY_TABLE => {
                let array = kind_of(item) == kind::ARRAY_TABLE;
                if let Some((at, t)) = leaves(item, at).find(|(_, t)| {
                    matches!(SyntaxKind(t.kind().0), kind::R_BRACKET | kind::R_BRACKET2)
                }) {
                    line_end_after(TextRange::at(at.into(), t.text_len()), text, &mut err);
                }
                let mut inner = nodes(item, at).peekable();
                let key = inner.next_if(|(_, n)| kind_of(n) == kind::KEY);
                current = key
                    .and_then(|(s, k)| model.header(k, s, array, text, &mut err))
                    .unwrap_or_else(|| model.scratch());
                for (s, p) in inner.filter(|(_, n)| kind_of(n) == kind::PAIR) {
                    line_end(p, s, text, &mut err);
                    model.pair(current, p, s, text, &mut err);
                }
            }
            _ => {}
        }
    }
    out
}

fn is_trivia(k: SyntaxKind) -> bool {
    matches!(k, kind::WHITESPACE | kind::NEWLINE | kind::COMMENT)
}

/// The range of the last token under `node` that is not trivia; `end` is where `node` ends.
fn last_token(node: &Green, mut end: u32) -> Option<TextRange> {
    for c in node.children().rev() {
        match c {
            NodeOrToken::Token(t) if !is_trivia(SyntaxKind(t.kind().0)) => {
                return Some(TextRange::new(
                    (end - u32::from(t.text_len())).into(),
                    end.into(),
                ));
            }
            NodeOrToken::Token(t) => end -= u32::from(t.text_len()),
            NodeOrToken::Node(n) => {
                if let Some(r) = last_token(n, end) {
                    return Some(r);
                }
                end -= u32::from(n.text_len());
            }
        }
    }
    None
}

/// An entry ends its line: only blanks, a comment or the end of the file may follow it.
fn line_end(pair: &Green, at: u32, text: &str, err: &mut Err<'_>) {
    if let Some(last) = last_token(pair, at + u32::from(pair.text_len())) {
        line_end_after(last, text, err);
    }
}

fn line_end_after(last: TextRange, text: &str, err: &mut Err<'_>) {
    let rest = text[usize::from(last.end())..].trim_start_matches([' ', '\t']);
    if !(rest.is_empty() || rest.starts_with(['\n', '\r', '#'])) {
        err(
            last,
            "missing-newline",
            "expected a line break after the entry".into(),
        );
    }
}

// ---- strings ----

/// Checks a string token (quotes included) for escapes, control characters and quote runs.
fn string(raw: &str) -> Result<(), String> {
    let (quote, multi) = match raw.as_bytes() {
        [b'"', b'"', b'"', ..] if raw.len() >= 6 => ('"', true),
        [b'\'', b'\'', b'\'', ..] if raw.len() >= 6 => ('\'', true),
        [b'"', ..] => ('"', false),
        _ => ('\'', false),
    };
    let n = if multi { 3 } else { 1 };
    let body = raw
        .get(n..raw.len().saturating_sub(n))
        .ok_or("unterminated string")?;
    let mut chars = body.chars();
    let mut quotes = 0;
    while let Some(c) = chars.next() {
        quotes = if c == quote { quotes + 1 } else { 0 };
        if multi && quotes >= 3 {
            return Err("three or more consecutive quotes in a multi-line string".into());
        }
        match c {
            '\\' if quote == '"' => {
                quotes = 0;
                match chars.next() {
                    Some('b' | 't' | 'n' | 'f' | 'r' | '"' | '\\') => {}
                    Some(h @ ('u' | 'U')) => {
                        let mut v = 0u32;
                        for _ in 0..if h == 'u' { 4 } else { 8 } {
                            v = chars
                                .next()
                                .and_then(|c| c.to_digit(16))
                                .and_then(|d| v.checked_mul(16)?.checked_add(d))
                                .ok_or("invalid unicode escape")?;
                        }
                        char::from_u32(v).ok_or("unicode escape is not a scalar value")?;
                    }
                    // A line-ending backslash: blanks, then a line break.
                    Some(' ' | '\t' | '\n' | '\r') if multi => {
                        let mut rest = body[body.len() - chars.as_str().len() - 1..].chars();
                        let mut c = rest.next();
                        while matches!(c, Some(' ' | '\t')) {
                            c = rest.next();
                        }
                        let ok = match c {
                            Some('\n') => true,
                            Some('\r') => rest.next() == Some('\n'),
                            _ => false,
                        };
                        if !ok {
                            return Err("invalid escape".into());
                        }
                    }
                    _ => return Err("invalid escape".into()),
                }
            }
            '\r' if multi && chars.as_str().starts_with('\n') => {}
            '\n' if multi => {}
            '\t' => {}
            c if c < ' ' || c == '\u{7f}' => return Err("control character in a string".into()),
            _ => {}
        }
    }
    Ok(())
}

// ---- dates and times ----

/// Checks the ranges of a date, time or date-time token (RFC 3339 shapes are the grammar's).
fn date_time(raw: &str) -> Result<(), String> {
    let b = raw.as_bytes();
    let num = |from: usize, len: usize| -> Option<u32> {
        let s = b.get(from..from + len)?;
        s.iter()
            .all(u8::is_ascii_digit)
            .then(|| s.iter().fold(0, |v, d| v * 10 + u32::from(d - b'0')))
    };
    let mut i = 0;
    if b.get(4) == Some(&b'-') {
        let (Some(y), Some(m), Some(d)) = (num(0, 4), num(5, 2), num(8, 2)) else {
            return Err("invalid date".into());
        };
        let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
        let days = match m {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return Err("month out of range".into()),
        };
        if d == 0 || d > days {
            return Err("day out of range".into());
        }
        i = 10;
        if matches!(b.get(i), Some(b'T' | b't' | b' ')) {
            i += 1;
        }
    }
    if i < b.len() {
        match (num(i, 2), num(i + 3, 2), num(i + 6, 2)) {
            (Some(h), Some(m), Some(s)) if h <= 23 && m <= 59 && s <= 60 => {}
            _ => return Err("time out of range".into()),
        }
        i += 8;
        if b.get(i) == Some(&b'.') {
            i += 1;
            while b.get(i).is_some_and(u8::is_ascii_digit) {
                i += 1;
            }
        }
        match b.get(i) {
            Some(b'Z' | b'z') => i += 1,
            Some(b'+' | b'-') => {
                match (num(i + 1, 2), num(i + 4, 2)) {
                    (Some(h), Some(m)) if h <= 23 && m <= 59 => {}
                    _ => return Err("offset out of range".into()),
                }
                i += 6;
            }
            _ => {}
        }
    }
    if i == b.len() {
        Ok(())
    } else {
        Err("invalid date or time".into())
    }
}

// ---- the table model ----

enum Kind {
    /// `explicit`: defined by a `[header]`; `dotted`: created by a dotted key.
    Table { explicit: bool, dotted: bool },
    /// The elements of `[[header]]`, each a table node.
    Array(Vec<usize>),
    /// A value, an inline table or a static array: nothing can be added to it.
    Value,
}

struct Node {
    kind: Kind,
}

/// The tables of a document as headers and keys define them; node 0 is the root.
struct Model<'t> {
    nodes: Vec<Node>,
    /// (table, key part) -> member; one map, so a table costs no allocation of its own.
    kids: HashMap<(usize, Cow<'t, str>), usize>,
    /// The parts of the key being checked, kept between keys to save an allocation each.
    parts: Vec<Cow<'t, str>>,
}

impl<'t> Model<'t> {
    fn new() -> Self {
        let mut m = Model {
            nodes: Vec::new(),
            kids: HashMap::new(),
            parts: Vec::new(),
        };
        m.add(Kind::Table {
            explicit: true,
            dotted: false,
        });
        m
    }

    fn add(&mut self, kind: Kind) -> usize {
        self.nodes.push(Node { kind });
        self.nodes.len() - 1
    }

    /// A table outside the document, for the entries of a rejected header and inline tables.
    fn scratch(&mut self) -> usize {
        self.add(Kind::Table {
            explicit: true,
            dotted: false,
        })
    }

    fn child(&mut self, parent: usize, name: Cow<'t, str>, kind: Kind) -> usize {
        let n = self.add(kind);
        self.kids.insert((parent, name), n);
        n
    }

    fn element(&mut self, array: usize) -> usize {
        let e = self.scratch();
        if let Kind::Array(v) = &mut self.nodes[array].kind {
            v.push(e);
        }
        e
    }

    /// `[a.b]` or `[[a.b]]`: the table its entries go to. The parts before the last continue in
    /// the last element of an array of tables and create missing tables; a value stops the walk.
    fn header(
        &mut self,
        key: &Green,
        start: u32,
        array: bool,
        text: &'t str,
        err: &mut Err<'_>,
    ) -> Option<usize> {
        let mut parts = std::mem::take(&mut self.parts);
        let end = key_parts(key, start, text, &mut parts);
        let range = TextRange::new(start.into(), end.into());
        let result = self.header_in(&parts, range, array, err);
        parts.clear();
        self.parts = parts;
        result
    }

    fn header_in(
        &mut self,
        parts: &[Cow<'t, str>],
        range: TextRange,
        array: bool,
        err: &mut Err<'_>,
    ) -> Option<usize> {
        let (last, path) = parts.split_last()?;
        let mut cur = 0;
        for p in path {
            cur = match self.kids.get(&(cur, p.clone())).copied() {
                None => self.child(
                    cur,
                    p.clone(),
                    Kind::Table {
                        explicit: false,
                        dotted: false,
                    },
                ),
                Some(n) => match &self.nodes[n].kind {
                    Kind::Table { .. } => n,
                    Kind::Array(v) => *v.last()?,
                    Kind::Value => {
                        err(
                            range,
                            "invalid-table",
                            format!("`{p}` is a value, not a table"),
                        );
                        return None;
                    }
                },
            };
        }
        let existing = self.kids.get(&(cur, last.clone())).copied();
        let result = match (existing, array) {
            (None, false) => Ok(self.child(
                cur,
                last.clone(),
                Kind::Table {
                    explicit: true,
                    dotted: false,
                },
            )),
            (None, true) => {
                let n = self.child(cur, last.clone(), Kind::Array(Vec::new()));
                Ok(self.element(n))
            }
            (Some(n), false) => match self.nodes[n].kind {
                Kind::Table {
                    explicit: false,
                    dotted: false,
                } => {
                    self.nodes[n].kind = Kind::Table {
                        explicit: true,
                        dotted: false,
                    };
                    Ok(n)
                }
                _ => Err(format!("`{last}` is already defined")),
            },
            (Some(n), true) => match self.nodes[n].kind {
                Kind::Array(_) => Ok(self.element(n)),
                _ => Err(format!(
                    "`{last}` is already defined and is not an array of tables"
                )),
            },
        };
        result.map_err(|m| err(range, "invalid-table", m)).ok()
    }

    /// `a.b = value` in the table `base`; a dotted key may only pass tables other dotted keys made.
    fn pair(&mut self, base: usize, pair: &Green, at: u32, text: &'t str, err: &mut Err<'_>) {
        let mut inner = nodes(pair, at).peekable();
        if let Some((start, key)) = inner.next_if(|(_, n)| kind_of(n) == kind::KEY) {
            let mut parts = std::mem::take(&mut self.parts);
            let end = key_parts(key, start, text, &mut parts);
            let range = TextRange::new(start.into(), end.into());
            let mut cur = base;
            for (i, p) in parts.iter().enumerate() {
                let last = i + 1 == parts.len();
                cur = match self.kids.get(&(cur, p.clone())).copied() {
                    None => {
                        let kind = if last {
                            Kind::Value
                        } else {
                            Kind::Table {
                                explicit: false,
                                dotted: true,
                            }
                        };
                        self.child(cur, p.clone(), kind)
                    }
                    Some(n)
                        if !last
                            && matches!(self.nodes[n].kind, Kind::Table { dotted: true, .. }) =>
                    {
                        n
                    }
                    Some(_) => {
                        err(
                            range,
                            "duplicate-key",
                            format!("key `{p}` is already defined"),
                        );
                        break;
                    }
                };
            }
            parts.clear();
            self.parts = parts;
        }
        if let Some((start, v)) = inner.next() {
            self.value(v, start, text, err);
        }
    }

    /// The entries of inline tables inside a value are checked in a scope of their own.
    fn value(&mut self, v: &Green, at: u32, text: &'t str, err: &mut Err<'_>) {
        match kind_of(v) {
            kind::INLINE_TABLE => {
                let scope = self.scratch();
                for (s, p) in nodes(v, at).filter(|(_, n)| kind_of(n) == kind::PAIR) {
                    self.pair(scope, p, s, text, err);
                }
            }
            kind::ARRAY => {
                for (s, e) in nodes(v, at) {
                    self.value(e, s, text, err);
                }
            }
            _ => {}
        }
    }
}

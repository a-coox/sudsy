//! Protobuf formatting, laid out like `buf format`: two-space indented blocks, one statement per
//! line, a single space between the parts of a statement, compact options (`[a = 1, b = 2]`)
//! broken one per line as soon as there are two, message literals expanded one field per line.

use crate::generated::kind as k;
use crate::sort;
use core_config::{OptionSchema, shared};
use core_format::shapes::{BlockStyle, block_between, dangling_comments, statement_sequence_with};
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments, doc};
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken};

pub fn option_schema() -> OptionSchema {
    OptionSchema::new(vec![
        shared::indent_size(2),
        shared::indent_style(),
        shared::final_newline(true),
        shared::end_of_line(),
    ])
}

pub struct ProtoFormatter;

const BLOCK: BlockStyle = BlockStyle {
    max_blank_lines: 1,
    empty: "",
    force_blank: None,
    expand_empty: false,
    keep_same_line: false,
};

/// The literals of every string of a tree.
fn literals(tree: &SyntaxNode) -> Vec<String> {
    tree.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::STRING)
        .flat_map(|t| {
            pieces(t.text())
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .collect()
}

impl NodeFormatter for ProtoFormatter {
    /// Only the blanks between adjacent string literals may change.
    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        if literals(before) == literals(after) {
            Ok(())
        } else {
            Err("a string changed".into())
        }
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        _: &core_syntax::Grammar,
        _: &core_config::ResolvedOptions,
    ) -> Option<String> {
        crate::sort::sort_header(tree, text)
    }

    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let doc = match node.kind() {
            k::DOCUMENT => document(cx, node)?,
            k::MESSAGE_BODY | k::ENUM_BODY => core_format::shapes::block(cx, node, &BLOCK)?,
            k::BLOCK_LIT => literal(cx, node)?,
            _ => statement(cx, node)?,
        };
        Ok(Some(doc))
    }
}

/// Blank line required between two top-level statements: between package, imports and options,
/// and no blank line inside the import and option groups (`sort_header` removed them).
fn force_blank(_: &FormatCtx<'_>, prev: &SyntaxNode, next: &SyntaxNode) -> bool {
    let (p, n) = (sort::group(prev), sort::group(next));
    p != n && (1..=3).contains(&p) && (1..=3).contains(&n)
}

/// The statements, then the comments after them (owned by the end-of-file token).
fn document(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let items: Vec<SyntaxNode> = node.children().collect();
    let mut parts = vec![statement_sequence_with(
        cx,
        &items,
        1,
        Some(force_blank),
        false,
    )?];
    let eof = cx
        .last_token(node)
        .or_else(|| node.last_token())
        .ok_or_else(|| FormatError::Shape("Document without tokens".into()))?;
    let trailing = comments::leading_comments(cx.grammar, &eof);
    if !trailing.is_empty() {
        if !items.is_empty() {
            parts.push(doc::hardline());
            if comments::blank_lines_before(cx.grammar, &eof) > 0 {
                parts.push(doc::hardline());
            }
        }
        parts.push(core_format::context::leading_comments_doc(&trailing));
    }
    Ok(doc::concat(parts))
}

fn elements(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Vec<SyntaxElement> {
    node.children_with_tokens()
        .by_kind(|kind| !cx.grammar.is_trivia(kind))
        .collect()
}

fn element_doc(cx: &FormatCtx<'_>, e: &SyntaxElement) -> Result<Doc, FormatError> {
    match e {
        SyntaxElement::Node(n) => cx.format(n),
        SyntaxElement::Token(t) if t.kind() == k::STRING && pieces(t.text()).len() > 1 => {
            // Adjacent string literals: one per line, indented, as buf writes them.
            let lines = pieces(t.text()).into_iter().map(|p| {
                doc::concat(vec![
                    doc::hardline(),
                    core_format::context::verbatim_text(p),
                ])
            });
            Ok(doc::concat(vec![
                cx.leading_comments_doc(t),
                doc::indent(doc::concat(lines.collect())),
                cx.trailing_comments_doc(t),
            ]))
        }
        SyntaxElement::Token(t) => Ok(cx.token(t)),
    }
}

/// The literals of a string token (`"a" "b"` is two) without the blanks between them.
fn pieces(text: &str) -> Vec<&str> {
    let (mut out, mut start, mut quote) = (Vec::new(), 0, None);
    let mut chars = text.char_indices();
    while let Some((i, c)) = chars.next() {
        match (quote, c) {
            (Some(_), '\\') => {
                chars.next();
            }
            (Some(q), c) if c == q => {
                out.push(&text[start..=i]);
                quote = None;
            }
            (None, '"' | '\'') => {
                start = i;
                quote = Some(c);
            }
            _ => {}
        }
    }
    out
}

fn is_multi_string(e: &SyntaxElement) -> bool {
    let first = match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => n.first_token(),
    };
    first.is_some_and(|t| t.kind() == k::STRING && pieces(t.text()).len() > 1)
}

/// The text a spacing rule looks at: a token's, or the edge token of a node's.
fn edge(cx: &FormatCtx<'_>, e: &SyntaxElement, last: bool) -> String {
    match e {
        SyntaxElement::Token(t) => t.text().to_string(),
        SyntaxElement::Node(n) => (if last {
            cx.last_token(n)
        } else {
            cx.first_token(n)
        })
        .map(|t| t.text().to_string())
        .unwrap_or_default(),
    }
}

/// One space between two parts of a statement, except around the punctuation that hugs its
/// neighbours: `a.b`, `(x)`, `map<k, v>`, `1, 2`, `-1`, `name;`.
fn space_between(cx: &FormatCtx<'_>, prev: &SyntaxElement, next: &SyntaxElement) -> bool {
    if is_multi_string(next) {
        return false;
    }
    let (p, n) = (edge(cx, prev, true), edge(cx, next, false));
    match n.as_str() {
        ";" | "," | ")" | "]" | ">" | "<" | "/" | ":" => return false,
        "(" => return matches!(p.as_str(), "option" | "returns"),
        "." => return p == "extend",
        _ => {}
    }
    !matches!(p.as_str(), "(" | "[" | "<" | "-" | "+" | "." | "/")
}

/// A statement or a part of one: its parts one space apart; `[options]` and `{ block }` laid out
/// as buf does.
fn statement(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let els = elements(cx, node);
    let options = matches!(
        node.kind(),
        k::FIELD | k::ONEOF_FIELD | k::MAP_FIELD | k::ENUM_FIELD | k::GROUP | k::EXTENSIONS
    );
    let mut parts = Vec::new();
    let mut i = 0;
    while i < els.len() {
        if i > 0 && space_between(cx, &els[i - 1], &els[i]) {
            parts.push(doc::lit(" "));
        }
        let end = match &els[i] {
            SyntaxElement::Token(t) if options && t.text() == "[" => {
                let close = (i..els.len()).find(|&j| edge(cx, &els[j], false) == "]");
                close.map(|j| (j, compact_options(cx, &els[i..=j])))
            }
            SyntaxElement::Token(t) if t.kind() == k::L_BRACE => {
                let close = els
                    .iter()
                    .rposition(|e| matches!(e, SyntaxElement::Token(c) if c.kind() == k::R_BRACE));
                match (close, &els[close.unwrap_or(0)]) {
                    (Some(j), SyntaxElement::Token(c)) => {
                        Some((j, block_between(cx, node, t, c, &BLOCK)))
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        match end {
            Some((j, d)) => {
                parts.push(d?);
                i = j + 1;
            }
            None => {
                parts.push(element_doc(cx, &els[i])?);
                i += 1;
            }
        }
    }
    Ok(doc::concat(parts))
}

/// `[a = 1]` on one line for one option; `[` and the options one per line for more.
fn compact_options(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Result<Doc, FormatError> {
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first(), els.last())
    else {
        return Err(FormatError::Shape("options without brackets".into()));
    };
    let mut items: Vec<(SyntaxNode, Option<SyntaxToken>)> = Vec::new();
    for e in &els[1..els.len() - 1] {
        match e {
            SyntaxElement::Node(n) if n.kind() == k::FIELD_OPTIONS => {
                for c in elements(cx, n) {
                    push_option(&mut items, c);
                }
            }
            _ => push_option(&mut items, e.clone()),
        }
    }
    let mut parts = vec![cx.token(open)];
    let commented = els.iter().any(|e| match e {
        SyntaxElement::Node(n) => n.descendants_with_tokens().any(|d| d.kind() == k::COMMENT),
        SyntaxElement::Token(_) => false,
    });
    // A comment right after `[` goes on a line of its own.
    let after_open = comments::trailing_comments(cx.grammar, open);
    let commented = commented || !after_open.is_empty();
    if let Some(first) = parts.first_mut().filter(|_| !after_open.is_empty()) {
        *first = cx.token_text(open);
    }
    if items.len() <= 1 && !commented {
        for (n, _) in &items {
            parts.push(cx.format(n)?);
        }
    } else {
        let mut inner = vec![dangling_comments(&after_open, false)];
        for (idx, (n, comma)) in items.iter().enumerate() {
            inner.push(doc::hardline());
            inner.push(cx.format(n)?);
            if idx + 1 < items.len() {
                inner.push(
                    comma
                        .as_ref()
                        .map_or_else(|| doc::lit(","), |c| cx.token(c)),
                );
            }
        }
        parts.push(doc::indent(doc::concat(inner)));
        parts.push(doc::hardline());
    }
    parts.push(cx.token(close));
    Ok(doc::concat(parts))
}

fn push_option(items: &mut Vec<(SyntaxNode, Option<SyntaxToken>)>, e: SyntaxElement) {
    match e {
        SyntaxElement::Node(n) => items.push((n, None)),
        SyntaxElement::Token(t) => {
            if let Some(last) = items.last_mut() {
                last.1 = Some(t);
            }
        }
    }
}

fn push_value(fields: &mut [Field], e: &SyntaxElement) {
    if let Some(f) = fields.last_mut() {
        f.value.push(e.clone());
    }
}

fn blank_before(cx: &FormatCtx<'_>, e: &SyntaxElement) -> bool {
    let tok = match e {
        SyntaxElement::Token(t) => Some(t.clone()),
        SyntaxElement::Node(n) => cx.first_token(n),
    };
    tok.is_some_and(|t| comments::blank_lines_before(cx.grammar, &t) > 0)
}

/// A field of a message literal: its name and its value (a constant, or `[` constants `]`).
struct Field {
    /// A blank line stands before the field in the source.
    blank: bool,
    name: SyntaxElement,
    value: Vec<SyntaxElement>,
    sep: Option<SyntaxToken>,
}

/// `{a: 1 b: [1, 2] c {d: 2}}` as buf writes it: `name: value` per line, a colon after every
/// name, a list or literal of one element on one line and of more elements expanded.
fn literal(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    let els = elements(cx, node);
    let (Some(SyntaxElement::Token(open)), Some(SyntaxElement::Token(close))) =
        (els.first(), els.last())
    else {
        return Err(FormatError::Shape(
            "message literal without delimiters".into(),
        ));
    };
    let mut fields: Vec<Field> = Vec::new();
    let mut in_list = false;
    for e in &els[1..els.len() - 1] {
        let text = edge(cx, e, false);
        match (e, text.as_str()) {
            (SyntaxElement::Token(_), ":") if !in_list => {}
            (SyntaxElement::Token(t), "," | ";") if !in_list => {
                if let Some(f) = fields.last_mut() {
                    f.sep = Some(t.clone());
                }
            }
            (SyntaxElement::Token(_), "[") => {
                in_list = true;
                push_value(&mut fields, e);
            }
            (SyntaxElement::Token(_), "]") => {
                in_list = false;
                push_value(&mut fields, e);
            }
            _ if in_list => push_value(&mut fields, e),
            // A name starts a field; a constant after it is its value.
            (SyntaxElement::Node(n), _) if n.kind() == k::CONSTANT => push_value(&mut fields, e),
            _ => fields.push(Field {
                blank: !fields.is_empty() && blank_before(cx, e),
                name: e.clone(),
                value: Vec::new(),
                sep: None,
            }),
        }
    }
    let mut docs = Vec::new();
    for f in &fields {
        let mut d = vec![element_doc(cx, &f.name)?, doc::lit(": ")];
        d.push(value(cx, &f.value)?);
        if let Some(s) = &f.sep {
            d.push(cx.trailing_comments_doc(s));
        }
        docs.push(doc::concat(d));
    }
    let closing = doc::concat(vec![cx.token_text(close), cx.trailing_comments_doc(close)]);
    let dangling = comments::leading_comments(cx.grammar, close);
    let mut parts = vec![cx.token(open)];
    let compact = fields.iter().all(|f| is_scalar(&f.value)) && fields.len() <= 1;
    if !compact || !dangling.is_empty() {
        let mut inner = Vec::new();
        for (f, d) in fields.iter().zip(docs) {
            inner.push(doc::hardline());
            if f.blank {
                inner.push(doc::hardline());
            }
            inner.push(d);
        }
        inner.push(dangling_comments(&dangling, false));
        parts.push(doc::indent(doc::concat(inner)));
        parts.push(doc::hardline());
    } else {
        parts.extend(docs);
    }
    parts.push(closing);
    Ok(doc::concat(parts))
}

/// A value buf keeps on one line: a constant that is no message literal, or a list of at most
/// one such constant.
fn is_scalar(value: &[SyntaxElement]) -> bool {
    let plain = |e: &SyntaxElement| {
        matches!(e, SyntaxElement::Node(n) if n.kind() == k::CONSTANT
            && !n.children().any(|c| c.kind() == k::BLOCK_LIT))
    };
    match value {
        [SyntaxElement::Token(_), items @ .., SyntaxElement::Token(_)] => {
            items
                .iter()
                .filter(|e| matches!(e, SyntaxElement::Node(_)))
                .all(plain)
                && items.len() <= 1
        }
        [e] => plain(e),
        _ => false,
    }
}

/// The value of a literal field: a constant, or a list that is expanded from two elements on.
fn value(cx: &FormatCtx<'_>, els: &[SyntaxElement]) -> Result<Doc, FormatError> {
    match els {
        [
            SyntaxElement::Token(open),
            items @ ..,
            SyntaxElement::Token(close),
        ] if open.text() == "[" => {
            let docs: Vec<Doc> = items
                .iter()
                .filter(|e| matches!(e, SyntaxElement::Node(_)))
                .map(|e| element_doc(cx, e))
                .collect::<Result<_, _>>()?;
            let mut parts = vec![cx.token(open)];
            if docs.len() > 1 {
                let n = docs.len();
                let mut inner = Vec::new();
                for (i, d) in docs.into_iter().enumerate() {
                    inner.push(doc::hardline());
                    inner.push(d);
                    if i + 1 < n {
                        inner.push(doc::lit(","));
                    }
                }
                parts.push(doc::indent(doc::concat(inner)));
                parts.push(doc::hardline());
            } else {
                parts.extend(docs);
            }
            parts.push(cx.token(close));
            Ok(doc::concat(parts))
        }
        _ => Ok(doc::concat(
            els.iter()
                .map(|e| element_doc(cx, e))
                .collect::<Result<_, _>>()?,
        )),
    }
}

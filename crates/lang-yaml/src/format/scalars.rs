//! Scalars: line layout of flow scalars, quote style, and block scalar contents.

use super::comments;
use super::{Quotes, Style};
use crate::generated::kind as k;
use crate::scalar::{block_header, double_quoted_value, single_quoted_value};
use crate::tree::{block_parent_indent, is_trivia};
use core_format::{Doc, FormatCtx, FormatError, doc};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};
use std::borrow::Cow;

fn token_of(node: &SyntaxNode, what: &str) -> Result<SyntaxToken, FormatError> {
    node.children_with_tokens()
        .by_kind(|kind| kind.is_token() && !is_trivia(kind))
        .find_map(|e| e.into_token())
        .ok_or_else(|| FormatError::Shape(format!("{what} without a token")))
}

pub(crate) fn alias(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Result<Doc, FormatError> {
    Ok(comments::tok(cx, &token_of(node, "an alias")?))
}

pub(crate) fn scalar(
    cx: &FormatCtx<'_>,
    st: &Style,
    node: &SyntaxNode,
) -> Result<Doc, FormatError> {
    let t = token_of(node, "a scalar")?;
    let text = match t.kind() {
        k::PLAIN => Cow::Borrowed(t.text()),
        kind => restyle(t.text(), kind, st.quotes).map_or(Cow::Borrowed(t.text()), Cow::Owned),
    };
    Ok(comments::with_text(cx, &t, lines(&text)))
}

/// The lines of a flow scalar. Continuation lines are re-indented one level and lose their
/// surrounding blanks, which line folding ignores anyway; empty lines stay.
fn lines(text: &str) -> Doc {
    if !text.contains(['\n', '\r']) {
        return doc::snippet(text);
    }
    let all = split_lines(text);
    let last = all.len() - 1;
    let mut rest = Vec::new();
    for (i, line) in all.iter().enumerate().skip(1) {
        rest.push(doc::hardline());
        rest.push(line_doc(trim(line, true, i < last)));
    }
    doc::concat(vec![
        line_doc(trim(all[0], false, true)),
        doc::indent(doc::concat(rest)),
    ])
}

/// A line of a flow scalar; the printer keeps the blank of an escape at its end.
fn line_doc(line: &str) -> Doc {
    if line.ends_with([' ', '\t']) {
        doc::verbatim(line)
    } else {
        doc::text(line)
    }
}

/// Trims the blanks at the start and/or end of a line. A blank right after a backslash at the
/// end stays: `\ ` and `\<tab>` are escapes, not blanks.
fn trim(line: &str, start: bool, end: bool) -> &str {
    let mut s = line;
    if start {
        s = s.trim_start_matches([' ', '\t']);
    }
    if end {
        let trimmed = s.trim_end_matches([' ', '\t']);
        let backslashes = trimmed.bytes().rev().take_while(|c| *c == b'\\').count();
        let escaped = backslashes % 2 == 1 && trimmed.len() < s.len();
        s = &s[..trimmed.len() + usize::from(escaped)];
    }
    s
}

fn split_lines(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

/// The token text in the requested quote style, if that is possible without changing the value
/// and without more escapes than the source has.
pub(crate) fn restyle(text: &str, kind: SyntaxKind, quotes: Quotes) -> Option<String> {
    if text.contains(['\n', '\r']) {
        return None;
    }
    match (kind, quotes) {
        (k::DOUBLE_QUOTED, Quotes::Single) => {
            let value = double_quoted_value(text)?;
            if value.chars().any(|c| c.is_control()) {
                return None;
            }
            let escapes_before = text.matches('\\').count();
            let escapes_after = value.matches('\'').count();
            (escapes_after <= escapes_before).then(|| format!("'{}'", value.replace('\'', "''")))
        }
        (k::SINGLE_QUOTED, Quotes::Double) => {
            let value = single_quoted_value(text);
            if value.chars().any(|c| c.is_control()) {
                return None;
            }
            let escapes_before = text.matches("''").count();
            let escapes_after = value.matches(['"', '\\']).count();
            (escapes_after == 0 || escapes_after <= escapes_before).then(|| {
                let mut out = String::from('"');
                for c in value.chars() {
                    if matches!(c, '"' | '\\') {
                        out.push('\\');
                    }
                    out.push(c);
                }
                out.push('"');
                out
            })
        }
        _ => None,
    }
}

/// A block scalar: the header, then the content lines re-indented one level deeper than the
/// node it belongs to. `extra` is how far the surrounding layout has moved the scalar's
/// indentation base from that node's own indentation (+1 for a document-level scalar, whose node
/// is at -1); it only matters for the indentation indicator.
pub(crate) fn block_scalar(
    cx: &FormatCtx<'_>,
    st: &Style,
    node: &SyntaxNode,
    extra: usize,
) -> Result<Doc, FormatError> {
    let header = node
        .children_with_tokens()
        .by_kind(|kind| kind == k::BLOCK_HEADER)
        .find_map(|e| e.into_token())
        .ok_or_else(|| FormatError::Shape("a block scalar without a header".into()))?;
    let content = node
        .children_with_tokens()
        .by_kind(|kind| kind == k::BLOCK_CONTENT)
        .find_map(|e| e.into_token());
    let info = block_header(header.text());
    // With an indentation indicator the content depth must be 1-9 columns below the parent.
    let depth = match info.indent {
        Some(_) => st.indent.min(9 - extra.min(8)).max(1),
        None => st.indent,
    };
    let mut head = header.text().to_string();
    if info.indent.is_some() {
        let digit = char::from_digit((depth + extra) as u32, 10).unwrap_or('1');
        head = head
            .chars()
            .map(|c| if c.is_ascii_digit() { digit } else { c })
            .collect();
    }
    let mut parts = vec![comments::with_text(cx, &header, doc::text(head))];
    if let Some(content) = content {
        let m = match info.indent {
            Some(d) => (block_parent_indent(node) + d as i64).max(0) as usize,
            None => 0,
        };
        let lines = split_lines(content.text());
        let m = if info.indent.is_some() {
            m
        } else {
            // The first line with text sets the indentation.
            lines
                .iter()
                .find(|l| l.bytes().any(|c| c != b' '))
                .map_or(usize::MAX, |l| l.bytes().take_while(|c| *c == b' ').count())
        };
        let mut body = Vec::new();
        for line in &lines {
            body.push(doc::hardline());
            let spaces = line.bytes().take_while(|c| *c == b' ').count();
            let rest = &line[spaces.min(m)..];
            let blank = spaces <= m && rest.trim_matches(' ').is_empty();
            if !blank {
                body.push(doc::verbatim(rest));
            }
        }
        parts.push(doc::align(depth, doc::concat(body)));
    }
    Ok(doc::concat(parts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_change_only_when_no_new_escapes_are_needed() {
        use Quotes::{Double, Single};
        assert_eq!(
            restyle("\"a b\"", k::DOUBLE_QUOTED, Single).unwrap(),
            "'a b'"
        );
        assert_eq!(
            restyle("\"say \\\"hi\\\"\"", k::DOUBLE_QUOTED, Single).unwrap(),
            "'say \"hi\"'"
        );
        assert_eq!(restyle("\"it's\"", k::DOUBLE_QUOTED, Single), None);
        assert_eq!(restyle("\"tab\\t\"", k::DOUBLE_QUOTED, Single), None);
        assert_eq!(restyle("\"a\\nb\"", k::DOUBLE_QUOTED, Single), None);
        assert_eq!(
            restyle("'a b'", k::SINGLE_QUOTED, Double).unwrap(),
            "\"a b\""
        );
        assert_eq!(
            restyle("'it''s'", k::SINGLE_QUOTED, Double).unwrap(),
            "\"it's\""
        );
        assert_eq!(restyle("'say \"hi\"'", k::SINGLE_QUOTED, Double), None);
        assert_eq!(restyle("'a\\b'", k::SINGLE_QUOTED, Double), None);
        assert_eq!(restyle("''", k::SINGLE_QUOTED, Double).unwrap(), "\"\"");
        assert_eq!(restyle("'a'", k::SINGLE_QUOTED, Single), None);
    }

    #[test]
    fn trim_keeps_an_escaped_space() {
        assert_eq!(trim("a\\  ", false, true), "a\\ ");
        assert_eq!(trim("a\\\t ", false, true), "a\\\t");
        assert_eq!(trim("a  ", false, true), "a");
        assert_eq!(trim("  a  ", true, true), "a");
    }
}

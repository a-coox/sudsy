//! The meaning check of the output (debug builds and tests). The generic check has already
//! re-parsed the output with oxc and compared nodes and tokens, leaving out trivia, `;` and the
//! text of literals (tagged `#restyle`); here the literals of both trees are compared by value:
//! quotes and quote escapes of strings, the spelling of numbers, the order of regular expression
//! flags may differ, nothing else. JSX text (`JSXText` is transparent to the generic check) is
//! compared by its meaning per element: lines trimmed as JSX does, runs of whitespace one space,
//! `{" "}` a space.

use crate::format::same_value;
use crate::generated::kind as k;
use core_syntax::{SyntaxNode, SyntaxToken};

pub(crate) fn check(before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
    let literals = |tree: &SyntaxNode| {
        tree.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(|t| matches!(t.kind(), k::STRING | k::NUMBER | k::BIG_INT | k::REG_EXP))
            .filter(|t| t.parent().is_some_and(|p| p.kind() != k::JSX_TEXT))
            .collect::<Vec<_>>()
    };
    let (a, b) = (literals(before), literals(after));
    if a.len() != b.len() {
        return Err("the output has a different number of literals".into());
    }
    for (x, y) in a.iter().zip(&b) {
        let same = if in_attribute(x) && in_attribute(y) {
            attribute_value(x.text()) == attribute_value(y.text())
        } else {
            x.kind() == y.kind() && same_value(x.kind(), x.text(), y.text())
        };
        if !same {
            return Err(format!("the literal `{}` became `{}`", x.text(), y.text()));
        }
    }
    let (mut c, mut d) = (comments(before), comments(after));
    c.sort();
    d.sort();
    if let Some(i) = (0..c.len().max(d.len())).find(|&i| c.get(i) != d.get(i)) {
        return Err(format!(
            "comment {i} changed from {:?} to {:?}",
            c.get(i),
            d.get(i)
        ));
    }
    if jsx_texts(before) != jsx_texts(after) {
        return Err("the text of a JSX element changed".into());
    }
    Ok(())
}

/// The comments, without whitespace (a block comment is re-indented).
fn comments(tree: &SyntaxNode) -> Vec<String> {
    tree.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| matches!(t.kind(), k::LINE_COMMENT | k::BLOCK_COMMENT))
        .map(|t| t.text().split_whitespace().collect())
        .collect()
}

fn in_attribute(t: &SyntaxToken) -> bool {
    t.kind() == k::STRING
        && t.parent()
            .and_then(|p| p.parent())
            .is_some_and(|g| g.kind() == k::JSX_ATTRIBUTE)
}

/// The content of an attribute string: no backslash escapes, `&quot;` and `&apos;` are quotes.
fn attribute_value(raw: &str) -> String {
    raw[1..raw.len() - 1]
        .replace("&apos;", "'")
        .replace("&quot;", "\"")
}

/// Per element in document order: its text (`Some`) and the places of its other children (`None`).
fn jsx_texts(tree: &SyntaxNode) -> Vec<Vec<Option<String>>> {
    let flush = |text: &mut String, items: &mut Vec<Option<String>>| {
        let mut t = String::new();
        for c in text.chars() {
            if !matches!(c, ' ' | '\t' | '\n' | '\r') {
                t.push(c);
            } else if !t.ends_with(' ') {
                t.push(' ');
            }
        }
        if !t.is_empty() {
            items.push(Some(t));
        }
        text.clear();
    };
    let mut all = Vec::new();
    for e in tree.descendants() {
        if !matches!(e.kind(), k::JSX_ELEMENT | k::JSX_FRAGMENT) {
            continue;
        }
        let (mut items, mut text) = (Vec::new(), String::new());
        for c in e.children() {
            match c.kind() {
                k::JSX_OPENING_ELEMENT
                | k::JSX_CLOSING_ELEMENT
                | k::JSX_OPENING_FRAGMENT
                | k::JSX_CLOSING_FRAGMENT => {}
                k::JSX_TEXT => {
                    let raw = c.text().to_string();
                    text.push_str(&if raw.starts_with('{') {
                        " ".into()
                    } else {
                        clean_text(&raw)
                    });
                }
                _ => {
                    flush(&mut text, &mut items);
                    items.push(None);
                }
            }
        }
        flush(&mut text, &mut items);
        all.push(items);
    }
    all
}

/// JSX's reading of text: each line trimmed on the sides that touch a line break, empty lines
/// dropped, the rest joined by spaces.
fn clean_text(text: &str) -> String {
    let lines: Vec<&str> = text.split(['\n', '\r']).collect();
    let last = lines
        .iter()
        .rposition(|l| l.contains(|c| c != ' ' && c != '\t'));
    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        let mut l = line.replace('\t', " ");
        if i > 0 {
            l = l.trim_start_matches(' ').to_string();
        }
        if i + 1 < lines.len() {
            l = l.trim_end_matches(' ').to_string();
        }
        if !l.is_empty() {
            out.push_str(&l);
            if Some(i) != last {
                out.push(' ');
            }
        }
    }
    out
}

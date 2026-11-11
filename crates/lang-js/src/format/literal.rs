//! Strings, numbers, regular expressions and directives, and the printing of a node as written
//! with those restyled (what every node without layout code of its own gets).

use super::util::{semi, style};
use crate::generated::kind as k;
use core_format::context::verbatim_text;
use core_format::doc::{self, Doc};
use core_format::{FormatCtx, FormatError};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

pub(crate) fn format(node: &SyntaxNode, cx: &FormatCtx<'_>) -> Result<Option<Doc>, FormatError> {
    if node.kind() != k::DIRECTIVE {
        return Ok(None);
    }
    let Some(lit) = cx.first_token(node) else {
        return Ok(None);
    };
    let text = directive(lit.text(), style(cx).single_quote);
    let doc = doc::concat(vec![
        cx.leading_comments_doc(&lit),
        doc::text(text),
        cx.trailing_comments_doc(&lit),
        semi(cx, node),
    ]);
    Ok(Some(doc))
}

/// The node's text with its comments, strings and numbers restyled; nothing else changes.
pub(crate) fn verbatim(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Doc {
    let (Some(first), Some(last)) = (cx.first_token(node), cx.last_token(node)) else {
        return Doc::Nil;
    };
    let mut raw = String::new();
    let mut tok = first.clone();
    loop {
        match restyle(cx, &tok) {
            Some(text) => raw.push_str(&text),
            None => raw.push_str(tok.text()),
        }
        if tok == last {
            break;
        }
        match tok.next_token() {
            Some(next) => tok = next,
            None => break,
        }
    }
    doc::concat(vec![
        cx.leading_comments_doc(&first),
        verbatim_text(&raw),
        cx.trailing_comments_doc(&last),
    ])
}

/// The restyled text of a literal token, `None` when it stays.
fn restyle(cx: &FormatCtx<'_>, tok: &SyntaxToken) -> Option<String> {
    let text = tok.text();
    let new = match tok.kind() {
        k::STRING => {
            // Strings of JSX attributes and directives have rules of their own.
            let owner = tok.parent()?.parent();
            if owner.is_some_and(|o| matches!(o.kind(), k::JSX_ATTRIBUTE | k::DIRECTIVE)) {
                return None;
            }
            string(text, style(cx).single_quote)
        }
        k::NUMBER => number(text),
        k::BIG_INT => text.to_ascii_lowercase(),
        k::REG_EXP => regex(text),
        _ => return None,
    };
    (new != text).then_some(new)
}

/// A directive keeps its quotes unless it has none inside, then it takes the preferred ones.
fn directive(raw: &str, single: bool) -> String {
    let content = &raw[1..raw.len() - 1];
    if content.contains(['"', '\'']) {
        return raw.to_string();
    }
    let q = if single { '\'' } else { '"' };
    format!("{q}{content}{q}")
}

/// A string with the preferred quote, unless its text has more of those than of the other
/// quote. Escapes stay as written except the quotes: an escaped quote that needs no escape
/// loses its backslash, an unescaped enclosing quote gains one.
pub(crate) fn string(raw: &str, single: bool) -> String {
    let content = &raw[1..raw.len() - 1];
    let count = |q: char| content.matches(q).count();
    let (doubles, singles) = (count('"'), count('\''));
    let use_single = if single {
        singles <= doubles
    } else {
        doubles > singles
    };
    let (enclosing, other) = if use_single { ('\'', '"') } else { ('"', '\'') };
    let mut out = String::with_capacity(raw.len() + 2);
    out.push(enclosing);
    let mut chars = content.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(e) if e == other => out.push(e),
                Some(e) => {
                    out.push('\\');
                    out.push(e);
                }
                None => out.push('\\'),
            },
            c if c == enclosing => {
                out.push('\\');
                out.push(c);
            }
            c => out.push(c),
        }
    }
    out.push(enclosing);
    out
}

/// Lowercase, no `+` and no leading zeros in an exponent (none at all when it is zero), a zero
/// before a leading dot, no trailing zeros of a fraction (one digit stays) and no trailing dot.
pub(crate) fn number(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    if lower.starts_with("0x") || lower.starts_with("0b") || lower.starts_with("0o") {
        return lower;
    }
    let (mantissa, exponent) = match lower.split_once('e') {
        Some((m, e)) => (m, Some(e)),
        None => (lower.as_str(), None),
    };
    let mut out = String::new();
    if mantissa.starts_with('.') {
        out.push('0');
    }
    match mantissa.split_once('.') {
        Some((int, frac)) => {
            out.push_str(int);
            let digits = frac.bytes().all(|b| b.is_ascii_digit());
            let trimmed = frac.trim_end_matches('0');
            let frac = if digits && frac.len() > 1 && trimmed.len() < frac.len() {
                &frac[..trimmed.len().max(1)]
            } else {
                frac
            };
            if !frac.is_empty() {
                out.push('.');
                out.push_str(frac);
            }
        }
        None => out.push_str(mantissa),
    }
    if let Some(e) = exponent {
        let (sign, digits) = match e.strip_prefix('-') {
            Some(d) => ("-", d),
            None => ("", e.strip_prefix('+').unwrap_or(e)),
        };
        let digits = digits.trim_start_matches('0');
        if !digits.is_empty() {
            out.push('e');
            out.push_str(sign);
            out.push_str(digits);
        }
    }
    out
}

/// The flags of a regular expression in alphabetical order.
fn regex(raw: &str) -> String {
    let Some(end) = raw.rfind('/') else {
        return raw.to_string();
    };
    let mut flags: Vec<char> = raw[end + 1..].chars().collect();
    flags.sort_unstable();
    format!("{}{}", &raw[..=end], flags.into_iter().collect::<String>())
}

/// Do two literal tokens of the input and the output have one meaning?
pub(crate) fn same_value(kind: SyntaxKind, a: &str, b: &str) -> bool {
    match kind {
        k::STRING => unquoted(a) == unquoted(b),
        k::NUMBER => canonical_number(a) == canonical_number(b),
        k::BIG_INT => a.eq_ignore_ascii_case(b),
        _ => regex(a) == regex(b),
    }
}

/// The content of a string with quote escapes resolved, the one thing a quote change may alter.
fn unquoted(raw: &str) -> String {
    let content = raw.get(1..raw.len().saturating_sub(1)).unwrap_or("");
    let mut out = String::with_capacity(content.len());
    let mut chars = content.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some(e @ ('"' | '\'')) => out.push(e),
                Some(e) => {
                    out.push('\\');
                    out.push(e);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn canonical_number(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase().replace('_', "");
    if lower.starts_with("0x") || lower.starts_with("0b") || lower.starts_with("0o") {
        return lower;
    }
    match lower.parse::<f64>() {
        Ok(n) => format!("{n:e}"),
        Err(_) => lower,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_take_the_cheaper_quote() {
        assert_eq!(string("'a'", false), "\"a\"");
        assert_eq!(string("\"a\"", true), "'a'");
        assert_eq!(string("'say \"hi\"'", false), "'say \"hi\"'");
        assert_eq!(string("'a\"b\\'c\\'d'", false), "\"a\\\"b'c'd\"");
        assert_eq!(string("'\\d \\n'", false), "\"\\d \\n\"");
        assert_eq!(string("\"it's\"", true), "\"it's\"");
    }

    #[test]
    fn numbers_are_normalized() {
        for (from, to) in [
            ("0XAB", "0xab"),
            ("1E5", "1e5"),
            ("1e+05", "1e5"),
            ("1e-05", "1e-5"),
            ("1.0", "1.0"),
            ("1.50", "1.5"),
            (".5", "0.5"),
            ("5.", "5"),
            ("5.e3", "5e3"),
            ("0.0e0", "0.0"),
            ("1.00", "1.0"),
            ("1_0.0_0", "1_0.0_0"),
            ("1.0e10", "1.0e10"),
            ("0.10", "0.1"),
        ] {
            assert_eq!(number(from), to, "{from}");
        }
    }

    #[test]
    fn values_are_compared_not_text() {
        assert!(same_value(k::STRING, "'a\"'", "\"a\\\"\""));
        assert!(!same_value(k::STRING, "'a'", "'b'"));
        assert!(same_value(k::NUMBER, "1.50", "1.5"));
        assert!(!same_value(k::NUMBER, "1.5", "1.6"));
        assert!(same_value(k::REG_EXP, "/a/gi", "/a/ig"));
    }
}

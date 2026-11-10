//! Well-formedness the grammar accepts: an end tag that does not match its start tag, a repeated
//! attribute, `--` inside a comment, `]]>` in character data. Reported as parse errors.

use crate::generated::kind as k;
use core_syntax::{SyntaxNode, SyntaxNodeExt, SyntaxToken};
use provider_api::ParseError;

fn err(range: core_syntax::TextRange, code: &str, message: String) -> ParseError {
    ParseError {
        start: range.start().into(),
        len: u32::from(range.len()),
        code: code.into(),
        message,
    }
}

fn legal(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | ' '..='\u{D7FF}' | '\u{E000}'..='\u{FFFD}' | '\u{10000}'..)
}

/// The problems of the references in a token: a character reference to an illegal character
/// and, without a DOCTYPE that could declare them, an entity other than the five predefined.
fn references(t: &SyntaxToken, declared: bool, out: &mut Vec<ParseError>) {
    let text = t.text();
    for (i, _) in text.match_indices('&') {
        let Some(len) = text[i..].find(';') else {
            continue;
        };
        let body = &text[i + 1..i + len];
        let bad = if let Some(num) = body.strip_prefix('#') {
            let code = match num.strip_prefix('x') {
                Some(h) => u32::from_str_radix(h, 16),
                None => num.parse(),
            };
            !code.ok().and_then(char::from_u32).is_some_and(legal)
        } else {
            !declared && !matches!(body, "amp" | "lt" | "gt" | "quot" | "apos")
        };
        if bad {
            out.push(ParseError {
                start: u32::from(t.text_range().start()) + i as u32,
                len: len as u32 + 1,
                code: "invalid-reference".into(),
                message: format!("`&{body};` is not a legal reference"),
            });
        }
    }
}

pub fn validate(root: &SyntaxNode, text: &str) -> Vec<ParseError> {
    let mut out = Vec::new();
    let declared = root
        .descendants_with_tokens()
        .any(|e| e.kind() == k::DOCTYPE);
    for e in root.descendants_with_tokens() {
        if let Some(t) = e.as_token()
            && let Some((at, _)) = t.text().char_indices().find(|(_, c)| !legal(*c))
        {
            let start = u32::from(t.text_range().start()) + at as u32;
            out.push(ParseError {
                start,
                len: 1,
                code: "illegal-character".into(),
                message: "a character XML does not allow".into(),
            });
        }
        match e {
            rowan::NodeOrToken::Token(t)
                if t.kind() == k::XML_DECL
                    && usize::from(t.text_range().start())
                        != text.len() - text.trim_start_matches('\u{feff}').len() =>
            {
                out.push(err(
                    t.text_range(),
                    "misplaced-declaration",
                    "the XML declaration must start the file".into(),
                ));
            }
            rowan::NodeOrToken::Token(t)
                if t.kind() == k::NAME
                    && !t.text().starts_with(|c: char| {
                        c.is_alphabetic() || matches!(c, '_' | ':') || !c.is_ascii()
                    }) =>
            {
                out.push(err(
                    t.text_range(),
                    "invalid-name",
                    format!("`{}` is not a name", t.text()),
                ));
            }
            rowan::NodeOrToken::Token(t) if matches!(t.kind(), k::REFERENCE | k::ATT_VALUE) => {
                references(&t, declared, &mut out);
            }
            rowan::NodeOrToken::Node(n) if n.kind() == k::ELEMENT => {
                let name_of = |kind| n.child(kind).and_then(|c| c.token(k::NAME));
                if let (Some(s), Some(e)) = (name_of(k::S_TAG), name_of(k::E_TAG))
                    && s.text() != e.text()
                {
                    out.push(err(
                        e.text_range(),
                        "mismatched-end-tag",
                        format!("`</{}>` closes `<{}>`", e.text(), s.text()),
                    ));
                }
            }
            rowan::NodeOrToken::Node(n) if n.kind() == k::S_TAG || n.kind() == k::EMPTY_TAG => {
                let mut seen = std::collections::HashSet::new();
                for a in n.children() {
                    let Some(name) = a.token(k::NAME) else {
                        continue;
                    };
                    if !seen.insert(name.text().to_string()) {
                        out.push(err(
                            name.text_range(),
                            "duplicate-attribute",
                            format!("attribute `{}` appears twice", name.text()),
                        ));
                    }
                }
            }
            rowan::NodeOrToken::Token(t) if t.kind() == k::COMMENT => {
                let body = &t.text()[4.min(t.text().len())..t.text().len().saturating_sub(3)];
                if body.contains("--") || body.ends_with('-') {
                    out.push(err(
                        t.text_range(),
                        "invalid-comment",
                        "`--` inside a comment".into(),
                    ));
                }
            }
            rowan::NodeOrToken::Token(t)
                if t.kind() == k::CHAR_DATA && t.text().contains("]]>") =>
            {
                out.push(err(
                    t.text_range(),
                    "invalid-text",
                    "`]]>` in character data".into(),
                ));
            }
            _ => {}
        }
    }
    out
}

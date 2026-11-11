//! The families of a `font-family` declaration.

use super::model;
use crate::generated::kind as k;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxToken, TextRange};

pub struct Family {
    /// Unquoted.
    pub name: String,
    pub quoted: bool,
    pub range: TextRange,
}

pub const GENERIC: &[&str] = &[
    "serif",
    "sans-serif",
    "monospace",
    "cursive",
    "fantasy",
    "system-ui",
    "ui-serif",
    "ui-sans-serif",
    "ui-monospace",
    "ui-rounded",
    "math",
    "emoji",
    "fangsong",
];

/// The comma-separated families of a `font-family` declaration; `None` for another declaration or
/// a value that is computed (`var()`, `$x`, interpolation).
pub fn families(decl: &SyntaxNode) -> Option<Vec<Family>> {
    if model::property(decl)?.0 != "font-family" {
        return None;
    }
    let value = model::value(decl)?;
    let mut out = Vec::new();
    let mut run: Vec<SyntaxToken> = Vec::new();
    for e in value.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if model::is_trivia(t.kind()) => {}
            SyntaxElement::Token(t) if t.kind() == k::COMMA => push(&mut out, &mut run)?,
            SyntaxElement::Token(t) if t.text() == "!" => break,
            SyntaxElement::Token(t) => run.push(t),
            SyntaxElement::Node(_) => return None,
        }
    }
    push(&mut out, &mut run)?;
    Some(out)
}

fn push(out: &mut Vec<Family>, run: &mut Vec<SyntaxToken>) -> Option<()> {
    let (first, last) = (run.first()?, run.last()?);
    let range = first.text_range().cover(last.text_range());
    let family = if run.len() == 1 && first.kind() == k::STRING {
        let text = first.text();
        Family {
            name: text[1..text.len() - 1].to_string(),
            quoted: true,
            range,
        }
    } else if run.iter().all(|t| t.kind() == k::IDENT) {
        let name = run.iter().map(|t| t.text()).collect::<Vec<_>>().join(" ");
        Family {
            name,
            quoted: false,
            range,
        }
    } else {
        return None;
    };
    out.push(family);
    run.clear();
    Some(())
}

/// The families of a `font` shorthand: what follows the size (and `/ line-height`).
pub fn shorthand_families(decl: &SyntaxNode) -> Option<Vec<Family>> {
    if model::property(decl)?.0 != "font" {
        return None;
    }
    let value = model::value(decl)?;
    let mut out = Vec::new();
    let mut run: Vec<SyntaxToken> = Vec::new();
    let (mut sized, mut slash) = (false, false);
    for e in value.children_with_tokens() {
        let SyntaxElement::Token(t) = e else {
            return None;
        };
        if model::is_trivia(t.kind()) {
            continue;
        }
        if !sized {
            sized = matches!(t.kind(), k::DIMENSION | k::PERCENTAGE);
        } else if slash {
            slash = false;
        } else if t.text() == "/" {
            slash = true;
        } else if t.kind() == k::COMMA {
            push(&mut out, &mut run)?;
        } else if t.text() == "!" {
            break;
        } else {
            run.push(t);
        }
    }
    push(&mut out, &mut run)?;
    Some(out)
}

/// The families of a `font-family` or `font` declaration.
pub fn of(decl: &SyntaxNode) -> Option<Vec<Family>> {
    match model::property(decl)?.0.as_str() {
        "font-family" => families(decl),
        "font" => shorthand_families(decl),
        _ => None,
    }
}

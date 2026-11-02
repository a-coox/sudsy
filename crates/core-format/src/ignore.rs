//! `sudsy-ignore`, `sudsy-ignore-start` and `sudsy-ignore-end` comments. The marker words come
//! from each grammar's `suppress` lines and the comment delimiters from its `comments` line.

use crate::comments::{Comment, CommentKind};
use core_syntax::Grammar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreMark {
    /// Leave the next node as written.
    Ignore,
    /// Leave everything as written from the next node until the matching end.
    Start,
    End,
}

/// The comment's text without its delimiters; empty for what is not a plain comment.
fn body(g: &Grammar, c: &Comment) -> String {
    let text = c.text();
    let text: &str = &text;
    match c.kind {
        CommentKind::Line => {
            let prefix = g.comments.line.as_deref().unwrap_or("");
            text.strip_prefix(prefix).unwrap_or(text)
        }
        CommentKind::Block => match &g.comments.block {
            Some((open, close)) => text
                .strip_prefix(open.as_str())
                .and_then(|t| t.strip_suffix(close.as_str()))
                .unwrap_or(text),
            None => text,
        },
        CommentKind::Lines | CommentKind::Directive => "",
    }
    .trim()
    .to_string()
}

fn marker<'a>(g: &'a Grammar, role: &str) -> Option<&'a str> {
    g.suppressions
        .iter()
        .find(|s| s.role == role)
        .map(|s| s.marker.as_str())
}

pub fn mark_of(g: &Grammar, c: &Comment) -> Option<IgnoreMark> {
    let body = body(g, c);
    let word = body.split_whitespace().next()?;
    [
        ("ignore", IgnoreMark::Ignore),
        ("ignore-start", IgnoreMark::Start),
        ("ignore-end", IgnoreMark::End),
    ]
    .into_iter()
    .find(|(role, _)| marker(g, role) == Some(word))
    .map(|(_, mark)| mark)
}

/// The ignore mark among a token's leading comments (the last one wins).
pub fn mark_in(g: &Grammar, comments: &[Comment]) -> Option<IgnoreMark> {
    comments.iter().rev().find_map(|c| mark_of(g, c))
}

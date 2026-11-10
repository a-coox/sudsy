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
    match c.kind {
        CommentKind::Line | CommentKind::Block => g.comment_body(&c.text()).to_string(),
        CommentKind::Lines | CommentKind::Directive => String::new(),
    }
}

/// A role may list several marker words (`sudsy-ignore` next to `prettier-ignore`).
fn has_marker(g: &Grammar, role: &str, word: &str) -> bool {
    g.suppressions
        .iter()
        .any(|s| s.role == role && s.marker == word)
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
    .find(|(role, _)| has_marker(g, role, word))
    .map(|(_, mark)| mark)
}

/// The ignore mark among a token's leading comments (the last one wins).
pub fn mark_in(g: &Grammar, comments: &[Comment]) -> Option<IgnoreMark> {
    comments.iter().rev().find_map(|c| mark_of(g, c))
}

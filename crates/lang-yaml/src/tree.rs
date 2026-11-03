//! Small questions about positions in a YAML tree, shared by the loader and the formatter.

use crate::generated::kind as k;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

pub fn is_trivia(kind: SyntaxKind) -> bool {
    matches!(kind, k::WHITESPACE | k::NEWLINE | k::COMMENT)
}

/// Column of a token in its line, in characters.
pub fn column(tok: &SyntaxToken) -> usize {
    let mut col = 0;
    let mut prev = tok.prev_token();
    while let Some(t) = prev {
        let text = t.text();
        if let Some(i) = text.rfind(['\n', '\r']) {
            return col + text[i + 1..].chars().count();
        }
        col += text.chars().count();
        prev = t.prev_token();
    }
    col
}

pub fn first_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut t = node.first_token();
    while let Some(tok) = t {
        if !is_trivia(tok.kind()) {
            return Some(tok);
        }
        t = tok.next_token();
    }
    None
}

pub fn last_token(node: &SyntaxNode) -> Option<SyntaxToken> {
    let mut t = node.last_token();
    while let Some(tok) = t {
        if !is_trivia(tok.kind()) {
            return Some(tok);
        }
        t = tok.prev_token();
    }
    None
}

/// The significant token before `tok`.
pub fn prev_significant(tok: &SyntaxToken) -> Option<SyntaxToken> {
    let mut t = tok.prev_token();
    while let Some(p) = t {
        if !is_trivia(p.kind()) {
            return Some(p);
        }
        t = p.prev_token();
    }
    None
}

/// The significant token after `tok`.
pub fn next_significant(tok: &SyntaxToken) -> Option<SyntaxToken> {
    let mut t = tok.next_token();
    while let Some(n) = t {
        if !is_trivia(n.kind()) {
            return Some(n);
        }
        t = n.next_token();
    }
    None
}

/// Is there a line break between the end of `a` and the start of `b`?
pub fn line_break_between(a: &SyntaxToken, b: &SyntaxToken) -> bool {
    let mut t = a.next_token();
    while let Some(n) = t {
        if n == *b {
            return false;
        }
        if n.kind() == k::NEWLINE || n.text().contains(['\n', '\r']) {
            return true;
        }
        t = n.next_token();
    }
    false
}

/// Indentation of the block entry (or the document) a block scalar belongs to: the `n` that its
/// content indentation indicator is relative to. -1 at the document level.
pub fn block_parent_indent(node: &SyntaxNode) -> i64 {
    for a in node.ancestors().skip(1) {
        match a.kind() {
            k::BLOCK_MAPPING_ENTRY | k::BLOCK_SEQUENCE_ENTRY => {
                return first_token(&a).map_or(0, |t| column(&t) as i64);
            }
            k::DOCUMENT => return -1,
            _ => {}
        }
    }
    -1
}

/// Blank lines right after the end of a line of text: the lines that hold nothing but blanks,
/// up to the next comment or token. A last line of blanks without a line break counts too.
pub fn blank_lines_after(tok: &SyntaxToken) -> usize {
    let mut broken = false;
    let mut blank_pending = false;
    let mut count = 0;
    let mut t = tok.next_token();
    while let Some(n) = t {
        match n.kind() {
            k::NEWLINE if !broken => broken = true,
            k::NEWLINE => {
                count += 1;
                blank_pending = false;
            }
            k::WHITESPACE => blank_pending = broken,
            k::COMMENT if !broken => {}
            k::EOF => {
                count += usize::from(blank_pending);
                break;
            }
            _ => break,
        }
        t = n.next_token();
    }
    count
}

/// The first token of `kind` among `node`'s direct children.
pub fn token_of(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxToken> {
    node.children_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| t.kind() == kind)
}

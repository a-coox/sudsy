//! Comment placement, shared by every language and driven by the trivia attachment rule
//! (`core_syntax::trivia`): a token owns the rest of its line as *trailing* trivia and
//! everything between that and the next token as that token's *leading* trivia. Comments
//! therefore always have an owner token, and formatting code that prints tokens through
//! [`crate::FormatCtx::token`] can never lose one. A comment is *dangling* when its owner is a
//! closing delimiter of an otherwise empty (or fully consumed) container.
//!
//! Everything here reads the tree locally (token neighbours), so it works for any subtree.

use core_syntax::trivia::is_line_break;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, SyntaxToken};
use std::ops::ControlFlow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
}

#[derive(Clone, Debug)]
pub struct Comment {
    pub token: SyntaxToken,
    pub kind: CommentKind,
    /// Line breaks between this comment and whatever follows it (the next comment or the owner
    /// token). Only meaningful for leading comments.
    pub breaks_after: usize,
}

impl Comment {
    /// The comment as written, without trailing whitespace.
    pub fn text(&self) -> &str {
        self.token.text().trim_end()
    }
}

/// Number of line breaks in `text` (`\r\n` counts once).
pub fn count_breaks(text: &str) -> usize {
    let mut n = 0;
    let mut prev_cr = false;
    for c in text.chars() {
        if is_line_break(c) && !(c == '\n' && prev_cr) {
            n += 1;
        }
        prev_cr = c == '\r';
    }
    n
}

fn ends_line(t: &SyntaxToken) -> bool {
    t.text().chars().next_back().is_some_and(is_line_break)
}

fn is_trivia(g: &Grammar, k: SyntaxKind) -> bool {
    g.is_trivia(k)
}

/// The trivia run directly before `tok` and whether a non-trivia token precedes it.
fn run_before(g: &Grammar, tok: &SyntaxToken) -> (Vec<SyntaxToken>, bool) {
    let mut run = Vec::new();
    let mut cur = tok.prev_token();
    while let Some(t) = cur {
        if !is_trivia(g, t.kind()) {
            run.reverse();
            return (run, true);
        }
        cur = t.prev_token();
        run.push(t);
    }
    run.reverse();
    (run, false)
}

fn run_after(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxToken> {
    let mut run = Vec::new();
    let mut cur = tok.next_token();
    while let Some(t) = cur {
        if !is_trivia(g, t.kind()) {
            break;
        }
        cur = t.next_token();
        run.push(t);
    }
    run
}

fn trailing_cut(run: &[SyntaxToken]) -> usize {
    run.iter().position(ends_line).map_or(run.len(), |i| i + 1)
}

/// Trivia owned by `tok` that follows it on its line.
pub fn trailing_trivia(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxToken> {
    let mut run = run_after(g, tok);
    run.truncate(trailing_cut(&run));
    run
}

/// Trivia owned by `tok` that precedes it (own-line comments, indentation, blank lines).
pub fn leading_trivia(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxToken> {
    let (mut run, has_prev) = run_before(g, tok);
    let cut = if has_prev { trailing_cut(&run) } else { 0 };
    run.drain(..cut);
    run
}

fn kind_of(g: &Grammar, t: &SyntaxToken) -> Option<CommentKind> {
    if !g.has_tag(t.kind(), "comment") {
        return None;
    }
    let text = t.text();
    let is_line = g
        .comments
        .line
        .as_deref()
        .is_some_and(|l| text.starts_with(l));
    Some(if is_line {
        CommentKind::Line
    } else {
        CommentKind::Block
    })
}

/// What the trivia run before a token holds, found without collecting it. Most tokens have no
/// comment, and then every question about their trivia is answered by this one backward walk.
struct ScanBefore {
    /// Does a non-trivia token precede the run?
    has_prev: bool,
    /// Is there a comment in the run? Then the other fields are not computed.
    has_comment: bool,
    /// Line breaks in the whole run.
    breaks: usize,
    /// Line breaks in the tokens after the first one that ends a line.
    after_first_line_end: Option<usize>,
}

impl ScanBefore {
    /// Line breaks in the part of the run the token owns (see [`leading_trivia`]).
    fn leading_breaks(&self) -> usize {
        if self.has_prev {
            self.after_first_line_end.unwrap_or(0)
        } else {
            self.breaks
        }
    }
}

/// Calls `visit` with each token before the child at `idx` of `parent` (`None`: nothing
/// before), nearest first, until it returns `Break`. Walks the green tree: the neighbours of a
/// token are mostly trivia, and creating a red token for each of them would cost far more than
/// looking at them.
fn walk_back<B>(
    parent: Option<SyntaxNode>,
    mut idx: usize,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children().rev() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => down(n, visit)?,
            }
        }
        ControlFlow::Continue(())
    }
    let mut node = parent?;
    loop {
        for child in node.green().children().take(idx).rev() {
            let flow = match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text()),
                rowan::NodeOrToken::Node(n) => down(n, &mut visit),
            };
            if let ControlFlow::Break(b) = flow {
                return Some(b);
            }
        }
        idx = node.index();
        node = node.parent()?;
    }
}

/// Like [`walk_back`], for the tokens after `tok`.
fn walk_forward<B>(
    tok: &SyntaxToken,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => down(n, visit)?,
            }
        }
        ControlFlow::Continue(())
    }
    let mut node = tok.parent()?;
    let mut idx = tok.index() + 1;
    loop {
        for child in node.green().children().skip(idx) {
            let flow = match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text()),
                rowan::NodeOrToken::Node(n) => down(n, &mut visit),
            };
            if let ControlFlow::Break(b) = flow {
                return Some(b);
            }
        }
        idx = node.index() + 1;
        node = node.parent()?;
    }
}

fn text_ends_line(text: &str) -> bool {
    text.chars().next_back().is_some_and(is_line_break)
}

fn scan_back(g: &Grammar, parent: Option<SyntaxNode>, idx: usize) -> ScanBefore {
    let mut breaks = 0;
    let mut after_first_line_end = None;
    let stopped = walk_back(parent, idx, |kind, text| {
        if !is_trivia(g, kind) {
            return ControlFlow::Break(ScanBefore {
                has_prev: true,
                has_comment: false,
                breaks,
                after_first_line_end,
            });
        }
        if g.has_tag(kind, "comment") {
            return ControlFlow::Break(ScanBefore {
                has_prev: false,
                has_comment: true,
                breaks: 0,
                after_first_line_end: None,
            });
        }
        if text_ends_line(text) {
            after_first_line_end = Some(breaks);
        }
        breaks += count_breaks(text);
        ControlFlow::Continue(())
    });
    stopped.unwrap_or(ScanBefore {
        has_prev: false,
        has_comment: false,
        breaks,
        after_first_line_end,
    })
}

fn scan_before(g: &Grammar, tok: &SyntaxToken) -> ScanBefore {
    scan_back(g, tok.parent(), tok.index())
}

/// What precedes the first non-trivia token of a node: see [`node_leading`].
pub struct NodeLeading {
    /// Does the node hold a non-trivia token at all?
    pub has_tokens: bool,
    /// Is there a comment among the leading trivia of its first token? When there is, the
    /// other fields are not computed and the caller has to look at the token itself.
    pub has_comment: bool,
    /// [`blank_lines_before`] for the node's first token.
    pub blank_lines: usize,
}

/// Answers what [`blank_lines_before`] and [`leading_comments`] would for the first non-trivia
/// token of `node`, without creating that token or any of the trivia around it.
pub fn node_leading(g: &Grammar, node: &SyntaxNode) -> NodeLeading {
    // The trivia inside the node, in front of its first token.
    let (mut total, mut after_first, mut seen_line_end) = (0, 0, false);
    let mut comment = false;
    let found = walk_node(node, |kind, text| {
        if !is_trivia(g, kind) {
            return ControlFlow::Break(true);
        }
        if g.has_tag(kind, "comment") {
            comment = true;
            return ControlFlow::Break(true);
        }
        let n = count_breaks(text);
        total += n;
        if seen_line_end {
            after_first += n;
        } else if text_ends_line(text) {
            seen_line_end = true;
        }
        ControlFlow::Continue(())
    })
    .unwrap_or(false);
    if !found {
        return NodeLeading {
            has_tokens: false,
            has_comment: false,
            blank_lines: 0,
        };
    }
    // The trivia before the node, joined to the above.
    let outer = scan_back(g, node.parent(), node.index());
    if comment || outer.has_comment {
        return NodeLeading {
            has_tokens: true,
            has_comment: true,
            blank_lines: 0,
        };
    }
    let blank_lines = match (outer.has_prev, outer.after_first_line_end, seen_line_end) {
        (false, ..) => 0,
        (true, Some(after), _) => after + total,
        (true, None, true) => after_first,
        (true, None, false) => 0,
    };
    NodeLeading {
        has_tokens: true,
        has_comment: false,
        blank_lines,
    }
}

/// Calls `visit` with the tokens of `node` in document order until it returns `Break`.
fn walk_node<B>(
    node: &SyntaxNode,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => down(n, visit)?,
            }
        }
        ControlFlow::Continue(())
    }
    match down(&node.green(), &mut visit) {
        ControlFlow::Break(b) => Some(b),
        ControlFlow::Continue(()) => None,
    }
}

/// Is there a comment in the trivia directly after `tok`?
fn comment_follows(g: &Grammar, tok: &SyntaxToken) -> bool {
    walk_forward(tok, |kind, _| {
        if !is_trivia(g, kind) {
            ControlFlow::Break(false)
        } else if g.has_tag(kind, "comment") {
            ControlFlow::Break(true)
        } else {
            ControlFlow::Continue(())
        }
    })
    .unwrap_or(false)
}

pub fn leading_comments(g: &Grammar, tok: &SyntaxToken) -> Vec<Comment> {
    if !scan_before(g, tok).has_comment {
        return Vec::new();
    }
    let trivia = leading_trivia(g, tok);
    let mut out: Vec<Comment> = Vec::new();
    for t in trivia {
        match kind_of(g, &t) {
            Some(kind) => out.push(Comment {
                token: t,
                kind,
                breaks_after: 0,
            }),
            None => {
                if let Some(last) = out.last_mut() {
                    last.breaks_after += count_breaks(t.text());
                }
            }
        }
    }
    out
}

pub fn trailing_comments(g: &Grammar, tok: &SyntaxToken) -> Vec<Comment> {
    if !comment_follows(g, tok) {
        return Vec::new();
    }
    trailing_trivia(g, tok)
        .into_iter()
        .filter_map(|t| {
            kind_of(g, &t).map(|kind| Comment {
                token: t,
                kind,
                breaks_after: 0,
            })
        })
        .collect()
}

/// Blank lines between the previous line and `tok` (or its first leading comment), 0 at the
/// start of a file.
pub fn blank_lines_before(g: &Grammar, tok: &SyntaxToken) -> usize {
    let scan = scan_before(g, tok);
    if !scan.has_comment {
        return if scan.has_prev {
            scan.leading_breaks()
        } else {
            0
        };
    }
    let (_, has_prev) = run_before(g, tok);
    if !has_prev {
        return 0;
    }
    leading_trivia(g, tok)
        .iter()
        .take_while(|t| kind_of(g, t).is_none())
        .map(|t| count_breaks(t.text()))
        .sum()
}

/// Is there a line break in the trivia between `tok` and the next non-trivia token?
pub fn break_after(g: &Grammar, tok: &SyntaxToken) -> bool {
    walk_forward(tok, |kind, text| {
        if !is_trivia(g, kind) {
            ControlFlow::Break(false)
        } else if count_breaks(text) > 0 {
            ControlFlow::Break(true)
        } else {
            ControlFlow::Continue(())
        }
    })
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::{SyntaxNode, TreeBuilder, grammars};

    /// Tokens of `src` as a flat JSON-grammar tree: lexed by splitting on a tiny alphabet.
    fn tree(parts: &[(&str, &str)]) -> SyntaxNode {
        let g = grammars::json();
        let mut b = TreeBuilder::new();
        b.start_node(g.kind_by_name("Document").unwrap());
        for (kind, text) in parts {
            b.token(g.kind_by_name(kind).unwrap(), text);
        }
        b.token(g.kind_by_name("Eof").unwrap(), "");
        b.finish_node();
        SyntaxNode::new_root(b.finish())
    }

    fn token(root: &SyntaxNode, text: &str) -> SyntaxToken {
        root.descendants_with_tokens()
            .filter_map(|e| e.into_token())
            .find(|t| t.text() == text)
            .unwrap()
    }

    #[test]
    fn splits_trivia_between_trailing_and_leading() {
        let g = grammars::json();
        let root = tree(&[
            ("Number", "1"),
            ("Whitespace", " "),
            ("LineComment", "// a"),
            ("Newline", "\n"),
            ("Newline", "\n"),
            ("LineComment", "// b"),
            ("Newline", "\n"),
            ("Number", "2"),
        ]);
        let one = token(&root, "1");
        let two = token(&root, "2");
        let trailing: Vec<_> = trailing_comments(g, &one)
            .iter()
            .map(|c| c.text().to_string())
            .collect();
        assert_eq!(trailing, ["// a"]);
        let leading = leading_comments(g, &two);
        assert_eq!(leading.len(), 1);
        assert_eq!(leading[0].text(), "// b");
        assert_eq!(leading[0].breaks_after, 1);
        assert_eq!(blank_lines_before(g, &two), 1);
        assert_eq!(blank_lines_before(g, &one), 0);
    }

    #[test]
    fn same_line_block_comment_is_trailing() {
        let g = grammars::json();
        let root = tree(&[
            ("Number", "1"),
            ("BlockComment", "/* a */"),
            ("Number", "2"),
        ]);
        assert_eq!(trailing_comments(g, &token(&root, "1")).len(), 1);
        assert!(leading_comments(g, &token(&root, "2")).is_empty());
        assert!(!break_after(g, &token(&root, "1")));
    }

    #[test]
    fn counts_crlf_once() {
        assert_eq!(count_breaks("\r\n"), 1);
        assert_eq!(count_breaks("\n\r\n\r"), 3);
    }
}

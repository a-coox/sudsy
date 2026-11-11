//! Comment placement, shared by every language and driven by the trivia attachment rule
//! (`core_syntax::trivia`): a token owns the rest of its line as *trailing* trivia and
//! everything between that and the next token as that token's *leading* trivia. Comments
//! therefore always have an owner token, and formatting code that prints tokens through
//! [`crate::FormatCtx::token`] can never lose one. A comment is *dangling* when its owner is a
//! closing delimiter of an otherwise empty (or fully consumed) container.
//!
//! Everything here reads the tree locally (token neighbours), so it works for any subtree.

use core_syntax::trivia::is_line_break;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken};
use std::borrow::Cow;
use std::ops::ControlFlow;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
    /// An own-line comment that is a structured trivia node (a C# doc comment) or a one-line
    /// directive tagged `#indented`: printed line by line at the current indentation.
    Lines,
    /// Own-line text kept as written at column 0 (tag `#verbatim-trivia`): a preprocessor
    /// directive, disabled code. It ends its own line.
    Directive,
}

#[derive(Clone, Debug)]
pub struct Comment {
    /// A trivia token, or a structured trivia node (one element of the run, see
    /// [`core_syntax::trivia`]).
    pub element: SyntaxElement,
    pub kind: CommentKind,
    /// Line breaks between this comment and whatever follows it (the next comment or the owner
    /// token), counting the comment's own line end. Only meaningful for leading comments.
    pub breaks_after: usize,
}

impl Comment {
    /// The token of a comment that is one (every comment of a language without structured
    /// trivia).
    pub fn token(&self) -> &SyntaxToken {
        match &self.element {
            SyntaxElement::Token(t) => t,
            SyntaxElement::Node(_) => panic!("a structured trivia is not a token"),
        }
    }

    /// The comment as written, without trailing whitespace. For [`CommentKind::Directive`] only
    /// the line break that ends it is removed: the lines of disabled code are exact.
    pub fn text(&self) -> Cow<'_, str> {
        let raw = element_text(&self.element);
        let strip = |t: &str| -> String {
            if self.kind == CommentKind::Directive {
                t.strip_suffix("\r\n")
                    .or_else(|| t.strip_suffix(['\n', '\r']))
                    .unwrap_or(t)
                    .to_string()
            } else {
                t.trim_end().to_string()
            }
        };
        match raw {
            Cow::Borrowed(t) if self.kind != CommentKind::Directive => Cow::Borrowed(t.trim_end()),
            other => Cow::Owned(strip(&other)),
        }
    }
}

/// How comments are printed, set by the language through `NodeFormatter::comment_style`.
#[derive(Clone, Copy, Debug)]
pub struct CommentStyle {
    /// Spaces between code and a comment after it on the same line.
    pub gap: usize,
    /// No blank line between a line comment (or a documentation comment) and what it comments:
    /// the blank lines after the last comment in front of a token are dropped, except in front
    /// of the first token of a file and after a `////` comment.
    pub attach_comments: bool,
    /// A comment after code keeps the blanks the source has before it when they are more than
    /// `gap` (comments aligned in a column stay aligned).
    pub keep_gap: bool,
}

impl Default for CommentStyle {
    fn default() -> Self {
        CommentStyle {
            gap: 1,
            attach_comments: false,
            keep_gap: false,
        }
    }
}

impl CommentStyle {
    pub fn gap_doc(&self) -> crate::Doc {
        match self.gap {
            1 => crate::doc::lit(" "),
            n => crate::doc::text(" ".repeat(n)),
        }
    }

    /// The gap before `c`: `gap`, or the blanks the source has when `keep_gap` and they are more.
    pub fn gap_before(&self, c: &Comment) -> crate::Doc {
        let source = match (&c.element, self.keep_gap) {
            (SyntaxElement::Token(t), true) => t
                .prev_token()
                .filter(|p| p.text().bytes().all(|b| b == b' ' || b == b'\t'))
                .map_or(0, |p| p.text().len()),
            _ => 0,
        };
        if source > self.gap {
            crate::doc::text(" ".repeat(source))
        } else {
            self.gap_doc()
        }
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

fn element_text(e: &SyntaxElement) -> Cow<'_, str> {
    match e {
        SyntaxElement::Token(t) => Cow::Borrowed(t.text()),
        SyntaxElement::Node(n) => Cow::Owned(n.text().to_string()),
    }
}

fn ends_line(e: &SyntaxElement) -> bool {
    match e {
        SyntaxElement::Token(t) => t.text().chars().next_back().is_some_and(is_line_break),
        // A structured trivia ends with a zero-width token; its text decides.
        SyntaxElement::Node(n) => n
            .text()
            .to_string()
            .chars()
            .next_back()
            .is_some_and(is_line_break),
    }
}

fn is_trivia(g: &Grammar, k: SyntaxKind) -> bool {
    g.is_trivia(k)
}

/// Is this kind a comment, or text that is kept as written like one?
fn is_comment_like(g: &Grammar, k: SyntaxKind) -> bool {
    g.has_tag(k, "comment") || g.has_tag(k, "verbatim-trivia")
}

/// The outermost structured trivia node a token is inside of, if any.
fn trivia_ancestor(g: &Grammar, t: &SyntaxToken) -> Option<SyntaxNode> {
    t.parent_ancestors()
        .filter(|a| is_trivia(g, a.kind()))
        .last()
}

/// A token as one element of a trivia run: the structured trivia it is in, else itself.
fn element_of(g: &Grammar, t: SyntaxToken) -> SyntaxElement {
    match trivia_ancestor(g, &t) {
        Some(n) => SyntaxElement::Node(n),
        None => SyntaxElement::Token(t),
    }
}

fn before_element(g: &Grammar, e: &SyntaxElement) -> Option<SyntaxElement> {
    let first = match e {
        SyntaxElement::Token(t) => t.clone(),
        SyntaxElement::Node(n) => n.first_token()?,
    };
    first.prev_token().map(|t| element_of(g, t))
}

fn after_element(g: &Grammar, e: &SyntaxElement) -> Option<SyntaxElement> {
    let last = match e {
        SyntaxElement::Token(t) => t.clone(),
        SyntaxElement::Node(n) => n.last_token()?,
    };
    last.next_token().map(|t| element_of(g, t))
}

fn is_trivia_element(g: &Grammar, e: &SyntaxElement) -> bool {
    is_trivia(g, e.kind())
}

/// The trivia run directly before `tok` and whether a non-trivia token precedes it.
fn run_before(g: &Grammar, tok: &SyntaxToken) -> (Vec<SyntaxElement>, bool) {
    let mut run = Vec::new();
    let mut cur = before_element(g, &SyntaxElement::Token(tok.clone()));
    while let Some(e) = cur {
        if !is_trivia_element(g, &e) {
            run.reverse();
            return (run, true);
        }
        cur = before_element(g, &e);
        run.push(e);
    }
    run.reverse();
    (run, false)
}

fn run_after(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxElement> {
    let mut run = Vec::new();
    let mut cur = after_element(g, &SyntaxElement::Token(tok.clone()));
    while let Some(e) = cur {
        if !is_trivia_element(g, &e) {
            break;
        }
        cur = after_element(g, &e);
        run.push(e);
    }
    run
}

fn trailing_cut(run: &[SyntaxElement]) -> usize {
    run.iter().position(ends_line).map_or(run.len(), |i| i + 1)
}

/// Trivia owned by `tok` that follows it on its line.
pub fn trailing_trivia(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxElement> {
    let mut run = run_after(g, tok);
    run.truncate(trailing_cut(&run));
    run
}

/// Trivia owned by `tok` that precedes it (own-line comments, indentation, blank lines).
pub fn leading_trivia(g: &Grammar, tok: &SyntaxToken) -> Vec<SyntaxElement> {
    let (mut run, has_prev) = run_before(g, tok);
    let cut = if has_prev { trailing_cut(&run) } else { 0 };
    run.drain(..cut);
    run
}

fn kind_of(g: &Grammar, e: &SyntaxElement) -> Option<CommentKind> {
    let kind = e.kind();
    if g.has_tag(kind, "verbatim-trivia") {
        return Some(if g.has_tag(kind, "indented") {
            CommentKind::Lines
        } else {
            CommentKind::Directive
        });
    }
    if !g.has_tag(kind, "comment") {
        return None;
    }
    let text = element_text(e);
    let is_line = g
        .comments
        .line
        .as_deref()
        .is_some_and(|l| text.starts_with(l));
    Some(match e {
        SyntaxElement::Node(_) if text.starts_with("///") => CommentKind::Lines,
        _ if is_line => CommentKind::Line,
        _ => CommentKind::Block,
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
    g: &Grammar,
    parent: Option<SyntaxNode>,
    mut idx: usize,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        g: &Grammar,
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children().rev() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => visit_node(g, n, visit, down)?,
            }
        }
        ControlFlow::Continue(())
    }
    let mut node = parent?;
    loop {
        for child in node.green().children().take(idx).rev() {
            let flow = match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text()),
                rowan::NodeOrToken::Node(n) => visit_node(g, n, &mut visit, down),
            };
            if let ControlFlow::Break(b) = flow {
                return Some(b);
            }
        }
        idx = node.index();
        node = node.parent()?;
    }
}

/// Visits a green node: a structured trivia is one element (its kind and whole text), any other
/// node is entered with `down`.
fn visit_node<B, V: FnMut(SyntaxKind, &str) -> ControlFlow<B>>(
    g: &Grammar,
    node: &rowan::GreenNodeData,
    visit: &mut V,
    down: fn(&Grammar, &rowan::GreenNodeData, &mut V) -> ControlFlow<B>,
) -> ControlFlow<B> {
    let kind = SyntaxKind(node.kind().0);
    if is_trivia(g, kind) {
        visit(kind, &node.to_string())
    } else {
        down(g, node, visit)
    }
}

/// Like [`walk_back`], for the tokens after `tok`.
fn walk_forward<B>(
    g: &Grammar,
    tok: &SyntaxToken,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        g: &Grammar,
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => visit_node(g, n, visit, down)?,
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
                rowan::NodeOrToken::Node(n) => visit_node(g, n, &mut visit, down),
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
    let stopped = walk_back(g, parent, idx, |kind, text| {
        if !is_trivia(g, kind) {
            return ControlFlow::Break(ScanBefore {
                has_prev: true,
                has_comment: false,
                breaks,
                after_first_line_end,
            });
        }
        if is_comment_like(g, kind) {
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
    let found = walk_node(g, node, |kind, text| {
        if !is_trivia(g, kind) {
            return ControlFlow::Break(true);
        }
        if is_comment_like(g, kind) {
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

/// Is there a comment among the trivia at the end of `node`, after its last significant token?
/// Walks the green tree backwards without creating tokens, so it can ask this of every node.
pub fn comment_at_end(g: &Grammar, node: &SyntaxNode) -> bool {
    fn down(g: &Grammar, node: &rowan::GreenNodeData) -> ControlFlow<bool> {
        for child in node.children().rev() {
            let (kind, flow) = match child {
                rowan::NodeOrToken::Token(t) => (SyntaxKind(t.kind().0), None),
                rowan::NodeOrToken::Node(n) if !is_trivia(g, SyntaxKind(n.kind().0)) => {
                    (SyntaxKind(n.kind().0), Some(down(g, n)))
                }
                rowan::NodeOrToken::Node(n) => (SyntaxKind(n.kind().0), None),
            };
            if let Some(flow) = flow {
                flow?;
            } else if !is_trivia(g, kind) {
                return ControlFlow::Break(false);
            } else if is_comment_like(g, kind) {
                return ControlFlow::Break(true);
            }
        }
        ControlFlow::Continue(())
    }
    matches!(down(g, &node.green()), ControlFlow::Break(true))
}

/// Calls `visit` with the tokens of `node` in document order until it returns `Break`.
fn walk_node<B>(
    g: &Grammar,
    node: &SyntaxNode,
    mut visit: impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
) -> Option<B> {
    fn down<B>(
        g: &Grammar,
        node: &rowan::GreenNodeData,
        visit: &mut impl FnMut(SyntaxKind, &str) -> ControlFlow<B>,
    ) -> ControlFlow<B> {
        for child in node.children() {
            match child {
                rowan::NodeOrToken::Token(t) => visit(SyntaxKind(t.kind().0), t.text())?,
                rowan::NodeOrToken::Node(n) => visit_node(g, n, visit, down)?,
            }
        }
        ControlFlow::Continue(())
    }
    match down(g, &node.green(), &mut visit) {
        ControlFlow::Break(b) => Some(b),
        ControlFlow::Continue(()) => None,
    }
}

/// Is there a comment in the trivia directly after `tok`?
fn comment_follows(g: &Grammar, tok: &SyntaxToken) -> bool {
    walk_forward(g, tok, |kind, _| {
        if !is_trivia(g, kind) {
            ControlFlow::Break(false)
        } else if is_comment_like(g, kind) {
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
                // A structured trivia ends its own line.
                breaks_after: usize::from(ends_line(&t) && kind != CommentKind::Line),
                element: t,
                kind,
            }),
            None => {
                if let Some(last) = out.last_mut() {
                    last.breaks_after += count_breaks(&element_text(&t));
                }
            }
        }
    }
    out
}

pub fn has_trailing_comment(g: &Grammar, tok: &SyntaxToken) -> bool {
    comment_follows(g, tok)
}

pub fn trailing_comments(g: &Grammar, tok: &SyntaxToken) -> Vec<Comment> {
    if !comment_follows(g, tok) {
        return Vec::new();
    }
    trailing_trivia(g, tok)
        .into_iter()
        .filter_map(|t| {
            kind_of(g, &t).map(|kind| Comment {
                element: t,
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
        .map(|t| count_breaks(&element_text(t)))
        .sum()
}

/// Does no non-trivia token come before `tok`?
pub fn starts_file(g: &Grammar, tok: &SyntaxToken) -> bool {
    !run_before(g, tok).1
}

/// Is there a line break in the trivia between `tok` and the next non-trivia token?
pub fn break_after(g: &Grammar, tok: &SyntaxToken) -> bool {
    walk_forward(g, tok, |kind, text| {
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

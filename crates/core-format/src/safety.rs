//! The generic safety check: the output must parse and must have the
//! same tree as the input once trivia are ignored.
//!
//! Tokens tagged `#separator` are ignored as well. A formatter may legitimately add or remove a
//! trailing comma, and a missing separator elsewhere changes the item structure or fails the
//! re-parse, so nothing is lost by not comparing them.
//!
//! A node tagged `#transparent` (JavaScript's parenthesized expression) is not compared, nor are
//! its own tokens (the parentheses): a formatter may add or remove it.
//!
//! Tokens tagged `#optional` are ignored too: the formatter may add or remove them.
//!
//! Tokens and nodes tagged `#restyle` (YAML's scalars, CSS's numbers) may change their text, for
//! example from single to double quotes or by re-indenting continuation lines; they have to stay
//! where they are, and their [`NodeFormatter::value`] has to stay the same.
//! The subtree of a `#restyle` node is not compared token by token.
//!
//! The comment tokens (tagged `#comment` and `#trivia`) are compared as a multiset of their texts
//! (line breaks as LF, the lines of a multi-line comment trimmed, the end of a one-line comment
//! trimmed): a formatter may move a comment, not lose, duplicate or edit one.
//! The provider's [`NodeFormatter::extra_check`] verifies what is not per item.

use crate::FormatError;
use crate::context::NodeFormatter;
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode};
use rowan::WalkEvent;
use std::borrow::Cow;

#[derive(Debug, PartialEq, Eq)]
enum Event<'a> {
    Enter(SyntaxKind),
    Leave(SyntaxKind),
    Token(SyntaxKind, &'a str),
    /// A `#restyle` token or node: only its position is compared here.
    Restyled,
}

/// What the comparison needs to know about a token kind, looked up once per kind.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Ignored,
    Comment,
    Restyle,
    Compared,
}

/// The significant events of a tree in document order, each with its byte offset. It walks the
/// green tree directly: building red nodes for millions of tokens would cost more than the
/// comparison itself.
struct Events<'a> {
    g: &'a Grammar,
    /// Open nodes: their kind and the children still to visit.
    stack: Vec<(SyntaxKind, rowan::Children<'a>, bool)>,
    /// Byte offset after the tokens seen so far.
    pos: usize,
    root: Option<&'a rowan::GreenNodeData>,
    /// The texts of the comments seen so far.
    comments: Vec<Cow<'a, str>>,
}

/// The text of a green node.
fn green_text(node: &rowan::GreenNodeData, out: &mut String) {
    for c in node.children() {
        match c {
            rowan::NodeOrToken::Node(n) => green_text(n, out),
            rowan::NodeOrToken::Token(t) => out.push_str(t.text()),
        }
    }
}

impl<'a> Events<'a> {
    fn new(g: &'a Grammar, root: &'a rowan::GreenNodeData) -> Self {
        Events {
            g,
            stack: Vec::new(),
            pos: 0,
            root: Some(root),
            comments: Vec::new(),
        }
    }

    fn enter(&mut self, node: &'a rowan::GreenNodeData) -> (usize, Event<'a>) {
        let kind = SyntaxKind(node.kind().0);
        let transparent = self.g.has_tag(kind, "transparent");
        self.stack.push((kind, node.children(), transparent));
        (self.pos, Event::Enter(kind))
    }

    fn class(&self, kind: SyntaxKind) -> Class {
        let g = self.g;
        if g.is_trivia(kind) {
            if g.has_tag(kind, "comment") {
                Class::Comment
            } else {
                Class::Ignored
            }
        } else if g.has_tag(kind, "separator") || g.has_tag(kind, "optional") {
            Class::Ignored
        } else if g.has_tag(kind, "restyle") {
            Class::Restyle
        } else {
            Class::Compared
        }
    }
}

impl<'a> Iterator for Events<'a> {
    type Item = (usize, Event<'a>);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(root) = self.root.take() {
            return Some(self.enter(root));
        }
        loop {
            let (kind, children, transparent) = self.stack.last_mut()?;
            let transparent = *transparent;
            match children.next() {
                None => {
                    let kind = *kind;
                    self.stack.pop();
                    if transparent {
                        continue;
                    }
                    return Some((self.pos, Event::Leave(kind)));
                }
                Some(rowan::NodeOrToken::Node(n)) => {
                    let kind = SyntaxKind(n.kind().0);
                    // A structured trivia (a C# doc comment, a directive) is trivia as a whole.
                    if self.g.is_trivia(kind) {
                        self.pos += usize::from(n.text_len());
                        if self.g.has_tag(kind, "comment") {
                            let mut text = String::new();
                            green_text(n, &mut text);
                            self.comments.push(text.into());
                        }
                        continue;
                    }
                    if self.g.has_tag(kind, "restyle") {
                        let at = self.pos;
                        self.pos += usize::from(n.text_len());
                        return Some((at, Event::Restyled));
                    }
                    let entered = self.enter(n);
                    if self.stack.last().is_some_and(|f| f.2) {
                        continue;
                    }
                    return Some(entered);
                }
                Some(rowan::NodeOrToken::Token(t)) => {
                    let kind = SyntaxKind(t.kind().0);
                    let at = self.pos;
                    self.pos += t.text().len();
                    let class = self.class(kind);
                    if class == Class::Comment {
                        self.comments.push(t.text().into());
                    }
                    if transparent {
                        continue;
                    }
                    match class {
                        Class::Ignored | Class::Comment => {}
                        Class::Restyle => return Some((at, Event::Restyled)),
                        Class::Compared => return Some((at, Event::Token(kind, t.text()))),
                    }
                }
            }
        }
    }
}

/// `Ok` when both trees have the same nodes and non-trivia tokens in the same order and the same
/// comments; else a description of the first difference. The value is the number of `#restyle`
/// items.
fn compare(g: &Grammar, a: &SyntaxNode, b: &SyntaxNode) -> Result<usize, String> {
    let (a, b) = (a.green(), b.green());
    let (mut a, mut b) = (Events::new(g, &a), Events::new(g, &b));
    let mut restyled = 0;
    loop {
        match (a.next(), b.next()) {
            (None, None) => break,
            (Some((_, x)), Some((_, y))) if x == y => restyled += usize::from(x == Event::Restyled),
            (x, y) => {
                return Err(format!(
                    "the trees differ: input has {}, output has {}",
                    describe(g, x),
                    describe(g, y)
                ));
            }
        }
    }
    let sorted = |v: Vec<Cow<'_, str>>| {
        let mut v: Vec<String> = v.iter().map(|c| comment_key(c)).collect();
        v.sort_unstable();
        v
    };
    let (c, d) = (sorted(a.comments), sorted(b.comments));
    match (0..c.len().max(d.len())).find(|&i| c.get(i) != d.get(i)) {
        None => Ok(restyled),
        Some(i) => Err(format!(
            "a comment was lost, duplicated or edited: input has {:?}, output has {:?}",
            c.get(i),
            d.get(i)
        )),
    }
}

/// A comment without what the printer changes: it writes LF only and re-indents the lines of a
/// multi-line comment; the end of a one-line comment is trimmed.
fn comment_key(text: &str) -> String {
    let text = text.replace("\r\n", "\n");
    if text.contains('\n') {
        text.lines().map(str::trim).collect::<Vec<_>>().join("\n")
    } else {
        text.trim_end().to_string()
    }
}

/// [`compare`] without the values of the `#restyle` items.
pub fn equal_ignoring_trivia(g: &Grammar, a: &SyntaxNode, b: &SyntaxNode) -> Result<(), String> {
    compare(g, a, b).map(drop)
}

/// The `#restyle` items of a tree in document order, as `compare` counts them.
fn restyled(g: &Grammar, root: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut out = Vec::new();
    let mut walk = root.preorder_with_tokens();
    while let Some(event) = walk.next() {
        let WalkEvent::Enter(e) = event else { continue };
        let kind = e.kind();
        if g.is_trivia(kind) {
            walk.skip_subtree();
        } else if g.has_tag(kind, "restyle") {
            let in_transparent = e
                .as_token()
                .and_then(|t| t.parent())
                .is_some_and(|p| g.has_tag(p.kind(), "transparent"));
            if !in_transparent && !g.has_tag(kind, "separator") && !g.has_tag(kind, "optional") {
                out.push(e);
            }
            walk.skip_subtree();
        }
    }
    out
}

fn same_values(
    g: &Grammar,
    f: &dyn NodeFormatter,
    a: &SyntaxNode,
    b: &SyntaxNode,
) -> Result<(), String> {
    let (a, b) = (restyled(g, a), restyled(g, b));
    for (x, y) in a.iter().zip(&b) {
        let (vx, vy) = (f.value(x), f.value(y));
        if vx != vy {
            return Err(format!(
                "{} `{vx}` became {} `{vy}`",
                g.name(x.kind()),
                g.name(y.kind())
            ));
        }
    }
    Ok(())
}

fn describe(g: &Grammar, ev: Option<(usize, Event<'_>)>) -> String {
    match ev {
        None => "nothing more".into(),
        Some((at, Event::Enter(k))) => format!("start of {} at byte {at}", g.name(k)),
        Some((at, Event::Leave(k))) => format!("end of {} at byte {at}", g.name(k)),
        Some((at, Event::Token(k, text))) => {
            format!("{} `{text}` at byte {at}", g.name(k))
        }
        Some((at, Event::Restyled)) => format!("a restyled token at byte {at}"),
    }
}

/// Runs the generic check and then the provider's own. `reparse` parses the output and returns
/// an error message if it has parse errors.
pub fn verify(
    g: &Grammar,
    formatter: &dyn NodeFormatter,
    before_tree: &SyntaxNode,
    after: &str,
    reparse: impl FnOnce(&str) -> Result<SyntaxNode, String>,
) -> Result<(), FormatError> {
    let after_tree = reparse(after)
        .map_err(|e| FormatError::Safety(format!("the output does not parse: {e}")))?;
    if compare(g, before_tree, &after_tree).map_err(FormatError::Safety)? > 0 {
        same_values(g, formatter, before_tree, &after_tree).map_err(FormatError::Safety)?;
    }
    formatter
        .extra_check(before_tree, &after_tree)
        .map_err(FormatError::Safety)
}

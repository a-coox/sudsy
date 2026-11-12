//! The lossless tree: `rowan` green/red trees keyed by our [`SyntaxKind`].

use crate::kind::SyntaxKind;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SudsyLang {}

impl rowan::Language for SudsyLang {
    type Kind = SyntaxKind;

    fn kind_from_raw(raw: rowan::SyntaxKind) -> SyntaxKind {
        SyntaxKind(raw.0)
    }

    fn kind_to_raw(kind: SyntaxKind) -> rowan::SyntaxKind {
        rowan::SyntaxKind(kind.0)
    }
}

pub type SyntaxNode = rowan::SyntaxNode<SudsyLang>;
pub type SyntaxToken = rowan::SyntaxToken<SudsyLang>;
pub type SyntaxElement = rowan::SyntaxElement<SudsyLang>;

/// A child of a green node.
pub type GreenElement<'a> = rowan::NodeOrToken<&'a rowan::GreenNodeData, &'a rowan::GreenTokenData>;

/// Lookups on a node's direct children.
pub trait SyntaxNodeExt {
    /// The first child node of `kind`.
    fn child(&self, kind: SyntaxKind) -> Option<SyntaxNode>;
    /// The own tokens (not those of child nodes).
    fn tokens(&self) -> impl Iterator<Item = SyntaxToken>;
    /// The first own token of `kind`.
    fn token(&self, kind: SyntaxKind) -> Option<SyntaxToken> {
        self.tokens().find(|t| t.kind() == kind)
    }
}

impl SyntaxNodeExt for SyntaxNode {
    fn child(&self, kind: SyntaxKind) -> Option<SyntaxNode> {
        self.children().find(|c| c.kind() == kind)
    }

    fn tokens(&self) -> impl Iterator<Item = SyntaxToken> {
        self.children_with_tokens().filter_map(|e| e.into_token())
    }
}

/// The span from the first to the last token of `node` that is neither trivia nor empty, `None`
/// without one. Walks the green tree, so no red element is allocated.
pub fn code_span(node: &SyntaxNode, grammar: &crate::Grammar) -> Option<rowan::TextRange> {
    green_code_span(&node.green(), node.text_range().start(), &|k| {
        grammar.is_trivia(k)
    })
}

/// [`code_span`] of a green node that starts at `base`, for any trivia test.
pub fn green_code_span(
    green: &rowan::GreenNodeData,
    base: rowan::TextSize,
    is_trivia: &dyn Fn(SyntaxKind) -> bool,
) -> Option<rowan::TextRange> {
    use rowan::{GreenNodeData, NodeOrToken};
    fn edge(
        n: &GreenNodeData,
        is_trivia: &dyn Fn(SyntaxKind) -> bool,
        back: bool,
        at: &mut u32,
    ) -> Option<(u32, u32)> {
        let mut visit = |c: NodeOrToken<&GreenNodeData, &rowan::GreenTokenData>| match c {
            NodeOrToken::Node(n) => edge(n, is_trivia, back, at),
            NodeOrToken::Token(t) => {
                let len = u32::from(t.text_len());
                if back {
                    *at -= len;
                }
                let start = *at;
                if !back {
                    *at += len;
                }
                (len != 0 && !is_trivia(SyntaxKind(t.kind().0))).then_some((start, len))
            }
        };
        if back {
            n.children().rev().find_map(&mut visit)
        } else {
            n.children().find_map(&mut visit)
        }
    }
    let (first, _) = edge(green, is_trivia, false, &mut 0)?;
    let (last, len) = edge(green, is_trivia, true, &mut u32::from(green.text_len()))?;
    Some(rowan::TextRange::new(
        base + rowan::TextSize::from(first),
        base + rowan::TextSize::from(last + len),
    ))
}

/// Prints the tree. The lossless invariant is `print(&tree) == source`.
pub fn print(node: &SyntaxNode) -> String {
    node.text().to_string()
}

/// Thin wrapper over `rowan::GreenNodeBuilder` that speaks our kinds.
#[derive(Default)]
pub struct TreeBuilder {
    inner: rowan::GreenNodeBuilder<'static>,
}

impl TreeBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_node(&mut self, kind: SyntaxKind) {
        debug_assert!(!kind.is_token());
        self.inner.start_node(rowan::SyntaxKind(kind.0));
    }

    pub fn token(&mut self, kind: SyntaxKind, text: &str) {
        debug_assert!(kind.is_token());
        self.inner.token(rowan::SyntaxKind(kind.0), text);
    }

    pub fn finish_node(&mut self) {
        self.inner.finish_node();
    }

    pub fn finish(self) -> rowan::GreenNode {
        self.inner.finish()
    }
}

/// Indented dump for debugging: `Kind start..end` for nodes, plus the escaped text for tokens.
pub fn dump_tree(root: &SyntaxNode, grammar: &crate::Grammar) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    let mut depth = 0usize;
    for event in root.preorder_with_tokens() {
        match event {
            rowan::WalkEvent::Enter(el) => {
                let name = grammar.name(el.kind());
                let range = el.text_range();
                let (start, end) = (u32::from(range.start()), u32::from(range.end()));
                let _ = write!(
                    out,
                    "{:indent$}{name} {start}..{end}",
                    "",
                    indent = depth * 2
                );
                if let rowan::NodeOrToken::Token(t) = &el {
                    let _ = write!(out, " {:?}", t.text());
                }
                out.push('\n');
                depth += 1;
            }
            rowan::WalkEvent::Leave(_) => depth -= 1,
        }
    }
    out
}

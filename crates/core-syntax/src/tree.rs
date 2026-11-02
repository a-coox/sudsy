//! The lossless tree: `rowan` green/red trees keyed by our [`SyntaxKind`].

use crate::kind::{LanguageId, SyntaxKind};

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

/// Prints the tree. The lossless invariant is `print(&tree) == source`.
pub fn print(node: &SyntaxNode) -> String {
    node.text().to_string()
}

/// For an [`SyntaxKind::EMBEDDED`] node, the language of the nested tree.
///
/// The nested tree is just the node's children: kinds are namespaced per
/// language, so a tree can mix languages without any extra bookkeeping, and
/// the wire encoding carries embedded trees with no special case.
pub fn embedded_language(node: &SyntaxNode) -> Option<LanguageId> {
    if node.kind() != SyntaxKind::EMBEDDED {
        return None;
    }
    node.children_with_tokens().next().map(|c| c.kind().lang())
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

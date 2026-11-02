//! Support code for the generated typed wrappers (`json::Member::key()` etc.).

use crate::kind::SyntaxKind;
use crate::tree::{SyntaxElement, SyntaxNode, SyntaxToken};

/// A typed view over a [`SyntaxNode`].
pub trait AstNode: Sized {
    fn can_cast(kind: SyntaxKind) -> bool;
    fn cast(syntax: SyntaxNode) -> Option<Self>;
    fn syntax(&self) -> &SyntaxNode;
}

/// Defines a wrapper for exactly one node kind.
#[macro_export]
macro_rules! ast_node {
    ($(#[$meta:meta])* $name:ident, $kind:expr) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name($crate::SyntaxNode);
        impl $crate::ast::AstNode for $name {
            fn can_cast(kind: $crate::SyntaxKind) -> bool {
                kind == $kind
            }
            fn cast(syntax: $crate::SyntaxNode) -> Option<Self> {
                Self::can_cast(syntax.kind()).then_some(Self(syntax))
            }
            fn syntax(&self) -> &$crate::SyntaxNode {
                &self.0
            }
        }
    };
}

/// Defines a wrapper for any node kind in a list (a `#tag` used as a field type).
#[macro_export]
macro_rules! ast_any {
    ($(#[$meta:meta])* $name:ident, $($kind:path)|+) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name($crate::SyntaxNode);
        impl $crate::ast::AstNode for $name {
            fn can_cast(kind: $crate::SyntaxKind) -> bool {
                matches!(kind, $($kind)|+)
            }
            fn cast(syntax: $crate::SyntaxNode) -> Option<Self> {
                Self::can_cast(syntax.kind()).then_some(Self(syntax))
            }
            fn syntax(&self) -> &$crate::SyntaxNode {
                &self.0
            }
        }
    };
}

/// One position in a node's body: the kinds it accepts and whether it repeats.
#[derive(Clone, Copy, Debug)]
pub struct Slot<'a> {
    /// Sorted, so membership is a binary search.
    pub kinds: &'a [SyntaxKind],
    pub many: bool,
}

/// A node's body as data: its slots in order, plus the kinds that never take part in matching.
#[derive(Clone, Copy, Debug)]
pub struct Pattern<'a> {
    pub slots: &'a [Slot<'a>],
    /// Trivia and `ERROR` nodes: invisible to the body.
    pub skip: &'a [SyntaxKind],
}

fn contains(kinds: &[SyntaxKind], kind: SyntaxKind) -> bool {
    kinds.binary_search(&kind).is_ok()
}

/// Assigns each child of `parent` to a slot of `pattern` (the generated accessors and the
/// conformance shape check use this one routine).
///
/// Children are matched in order: a child goes to the first slot at or after the current one that
/// accepts its kind, which skips slots of absent optional items. A `many` slot stays current until a
/// later slot takes a child; any other slot is current only up to its match. A child no later slot
/// accepts (a separator, or something the grammar does not describe) gets no slot and does not
/// move the current one.
pub fn assign<'a>(
    parent: &SyntaxNode,
    pattern: &'a Pattern<'a>,
) -> impl Iterator<Item = (usize, SyntaxElement)> + 'a {
    let mut current = 0;
    parent.children_with_tokens().filter_map(move |child| {
        let kind = child.kind();
        if contains(pattern.skip, kind) {
            return None;
        }
        let index =
            (current..pattern.slots.len()).find(|&i| contains(pattern.slots[i].kinds, kind))?;
        current = if pattern.slots[index].many {
            index
        } else {
            index + 1
        };
        Some((index, child))
    })
}

pub mod support {
    use super::*;

    /// The node in slot `index`, if the tree has one.
    pub fn node<N: AstNode>(parent: &SyntaxNode, pattern: &Pattern, index: usize) -> Option<N> {
        nodes(parent, pattern, index).next()
    }

    /// Every node in slot `index`.
    pub fn nodes<'a, N: AstNode + 'a>(
        parent: &SyntaxNode,
        pattern: &'a Pattern<'a>,
        index: usize,
    ) -> impl Iterator<Item = N> + 'a {
        assign(parent, pattern)
            .filter(move |(i, _)| *i == index)
            .filter_map(|(_, e)| e.into_node().and_then(N::cast))
    }

    /// The token in slot `index`, if the tree has one.
    pub fn token(parent: &SyntaxNode, pattern: &Pattern, index: usize) -> Option<SyntaxToken> {
        tokens(parent, pattern, index).next()
    }

    /// Every token in slot `index`.
    pub fn tokens<'a>(
        parent: &SyntaxNode,
        pattern: &'a Pattern<'a>,
        index: usize,
    ) -> impl Iterator<Item = SyntaxToken> + 'a {
        assign(parent, pattern)
            .filter(move |(i, _)| *i == index)
            .filter_map(|(_, e)| e.into_token())
    }
}

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

/// What [`assign`] needs to know about a node's body: a [`Pattern`], or a grammar's resolved body
/// (see [`crate::Grammar::slot`]).
pub trait Body {
    fn slots(&self) -> usize;
    fn accepts(&self, slot: usize, kind: SyntaxKind) -> bool;
    fn many(&self, slot: usize) -> bool;
    fn skips(&self, kind: SyntaxKind) -> bool;
}

impl Body for Pattern<'_> {
    fn slots(&self) -> usize {
        self.slots.len()
    }
    fn accepts(&self, slot: usize, kind: SyntaxKind) -> bool {
        contains(self.slots[slot].kinds, kind)
    }
    fn many(&self, slot: usize) -> bool {
        self.slots[slot].many
    }
    fn skips(&self, kind: SyntaxKind) -> bool {
        contains(self.skip, kind)
    }
}

impl<B: Body> Body for &B {
    fn slots(&self) -> usize {
        (**self).slots()
    }
    fn accepts(&self, slot: usize, kind: SyntaxKind) -> bool {
        (**self).accepts(slot, kind)
    }
    fn many(&self, slot: usize) -> bool {
        (**self).many(slot)
    }
    fn skips(&self, kind: SyntaxKind) -> bool {
        (**self).skips(kind)
    }
}

/// Assigns each child of `parent` to a slot of `body` (the generated accessors, the conformance
/// shape check and [`crate::Grammar::slot`] use this one routine).
///
/// Children are matched in order: a child goes to the first slot at or after the current one that
/// accepts its kind, which skips slots of absent optional items. A `many` slot stays current until a
/// later slot takes a child; any other slot is current only up to its match. A child no later slot
/// accepts (a separator, or something the grammar does not describe) gets no slot and does not
/// move the current one.
pub fn assign<'a, B: Body + Copy + 'a>(
    parent: &SyntaxNode,
    body: B,
) -> impl Iterator<Item = (usize, SyntaxElement)> + 'a {
    // A leading trivia child is passed over by kind, without materializing it; the rest is a
    // plain walk (a by-kind walk costs more allocations when nothing is skipped).
    let mut next = parent.first_child_or_token_by_kind(&|k| !body.skips(k));
    let mut started = false;
    let children = std::iter::from_fn(move || {
        let element = if started {
            next.take()?.to_next_sibling_or_token()
        } else {
            started = true;
            next.take()
        };
        next = element.clone();
        element
    });
    let mut placer = Placer::new(body);
    children.filter_map(move |child| Some((placer.place(child.kind())?, child)))
}

/// The slot walk of [`assign`], one child kind at a time (the red and the green walks share it).
pub struct Placer<B> {
    body: B,
    current: usize,
}

impl<B: Body> Placer<B> {
    pub fn new(body: B) -> Self {
        Placer { body, current: 0 }
    }

    /// The slot the next child, of `kind`, goes to; `None` for a skipped child or one without a slot.
    pub fn place(&mut self, kind: SyntaxKind) -> Option<usize> {
        let body = &self.body;
        if body.skips(kind) {
            return None;
        }
        let index = (self.current..body.slots()).find(|&i| body.accepts(i, kind))?;
        self.current = if body.many(index) { index } else { index + 1 };
        Some(index)
    }
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

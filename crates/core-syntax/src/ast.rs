//! Support code for the generated typed wrappers (`json::Member::key()` etc.).

use crate::kind::SyntaxKind;
use crate::tree::{SyntaxNode, SyntaxToken};

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

pub mod support {
    use super::*;

    /// The `nth` child node castable to `N`.
    pub fn child<N: AstNode>(parent: &SyntaxNode, nth: usize) -> Option<N> {
        children(parent).nth(nth)
    }

    pub fn children<N: AstNode>(parent: &SyntaxNode) -> impl Iterator<Item = N> {
        parent.children().filter_map(N::cast)
    }

    /// The `nth` direct child token whose kind satisfies `pred`.
    pub fn token(
        parent: &SyntaxNode,
        pred: impl Fn(SyntaxKind) -> bool,
        nth: usize,
    ) -> Option<SyntaxToken> {
        tokens(parent, pred).nth(nth)
    }

    pub fn tokens(
        parent: &SyntaxNode,
        pred: impl Fn(SyntaxKind) -> bool,
    ) -> impl Iterator<Item = SyntaxToken> {
        parent
            .children_with_tokens()
            .filter_map(|e| e.into_token())
            .filter(move |t| pred(t.kind()))
    }
}

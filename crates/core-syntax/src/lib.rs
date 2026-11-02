//! Shared syntax tree model for every language Sudsy handles.
//!
//! * [`kind`]: the 16-bit `SyntaxKind` namespace scheme.
//! * [`tree`]: the `rowan`-based lossless tree and a small builder.
//! * [`trivia`]: the single trivia attachment rule and a checker for it.
//! * [`grammar`]: the `.grammar` description format, its loader and content hash.
//! * [`ast`]: support code used by the generated typed wrappers.
//! * [`codegen`]: generates those wrappers (driven by `cargo xtask codegen`).
//! * [`wire`]: the versioned tree encoding used across process boundaries.

pub mod ast;
pub mod codegen;
pub mod generated;
pub mod grammar;
pub mod grammars;
pub mod kind;
pub mod tree;
pub mod trivia;
pub mod wire;

pub use grammar::Grammar;
pub use kind::{LanguageId, SyntaxKind};
pub use rowan::{GreenNode, TextRange, TextSize};
pub use tree::{
    SudsyLang, SyntaxElement, SyntaxNode, SyntaxToken, TreeBuilder, embedded_language, print,
};

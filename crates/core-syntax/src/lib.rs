//! Shared syntax tree model for every language Sudsy handles.
//!
//! * [`kind`]: the 32-bit `SyntaxKind` namespace scheme.
//! * [`tree`]: the `rowan`-based lossless tree and a small builder.
//! * [`trivia`]: the single trivia attachment rule and a checker for it.
//! * [`grammar`]: the `.grammar` description format, its loader and content hash.
//! * [`ast`]: support code used by the generated typed wrappers.
//! * [`codegen`]: generates those wrappers (driven by `cargo xtask codegen`).
//! * [`shape`]: checks trees against the bodies of a grammar.
//! * [`wire`]: the versioned tree encoding used across process boundaries.

pub mod ast;
pub mod codegen;
pub mod embedded;
pub mod grammar;
pub mod grammars;
pub mod kind;
pub mod shape;
pub mod tree;
pub mod trivia;
pub mod wire;

pub use embedded::Embedded;
pub use grammar::Grammar;
pub use kind::{LanguageId, SyntaxKind};
pub use rowan::{GreenNode, TextRange, TextSize};
pub use tree::{
    SudsyLang, SyntaxElement, SyntaxNode, SyntaxNodeExt, SyntaxToken, TreeBuilder, dump_tree, print,
};

/// The typed wrappers the crate's build script generates from its grammar (`build/lang-crate.rs`).
#[macro_export]
macro_rules! include_generated {
    () => {
        #[allow(clippy::all, dead_code, unused_imports)]
        pub mod generated {
            include!(concat!(env!("OUT_DIR"), "/generated.rs"));
        }
    };
}

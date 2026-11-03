//! Capability layers, symbol ids, the facts a provider's semantic model answers about a file, and
//! behavior summaries (plan.md "Semantic analysis"). Language neutral: a provider fills these in,
//! rules read them.

mod facts;
mod layer;
mod summaries;

pub use facts::{
    Accessibility, Answers, CompilerDiagnostic, Declaration, DeclarationFlags, DeclarationKind,
    FileSemantics, Reference, Span, qualified_name,
};
pub use layer::{Layer, Layers};
pub use summaries::{BehaviorSummaries, BodyFlow, SemanticView};

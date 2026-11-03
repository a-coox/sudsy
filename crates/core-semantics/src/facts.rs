//! What a provider's semantic model says about one file. These types are the worker's wire
//! payload too (MessagePack arrays in field order, mirrored by
//! `workers/csharp/Sudsy.Worker/Protocol/SemanticMessages.cs`). Spans are UTF-8 byte offsets
//! into the file text.

use core_flow::BodyGraph;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub start: u32,
    pub len: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeclarationKind {
    Type,
    Method,
    Field,
    Property,
    Event,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accessibility {
    Private,
    Internal,
    Protected,
    Public,
}

pub struct DeclarationFlags;

impl DeclarationFlags {
    pub const ASYNC: u8 = 1;
    pub const OVERRIDE: u8 = 2;
    /// Implements an interface member (implicitly or explicitly).
    pub const IMPLEMENTS: u8 = 4;
    pub const EXPLICIT: u8 = 8;
    pub const HAS_ATTRIBUTES: u8 = 16;
    /// `extern` or part of a `partial` method.
    pub const EXTERN_OR_PARTIAL: u8 = 32;
    /// Returns `Task`, `ValueTask` or their generic forms.
    pub const RETURNS_TASK: u8 = 64;
    pub const STATIC: u8 = 128;
}

/// A type or member declared in the file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Declaration {
    /// The symbol id (C#: a documentation comment id, `M:Shop.Orders.OrderService.Get(System.Int32)`).
    pub symbol: String,
    pub kind: u8,
    pub name: String,
    /// The name's span.
    pub start: u32,
    pub len: u32,
    pub accessibility: u8,
    pub flags: u8,
}

impl Declaration {
    pub fn kind(&self) -> DeclarationKind {
        match self.kind {
            0 => DeclarationKind::Type,
            1 => DeclarationKind::Method,
            2 => DeclarationKind::Field,
            3 => DeclarationKind::Property,
            _ => DeclarationKind::Event,
        }
    }

    pub fn accessibility(&self) -> Accessibility {
        match self.accessibility {
            1 => Accessibility::Internal,
            2 => Accessibility::Protected,
            3 => Accessibility::Public,
            _ => Accessibility::Private,
        }
    }

    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }
}

/// A use of a symbol declared in the project.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reference {
    pub symbol: String,
    pub start: u32,
    pub len: u32,
}

/// A compiler diagnostic (only asked for by tests that compare analyses with the compiler).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompilerDiagnostic {
    pub id: String,
    pub start: u32,
    pub len: u32,
    pub message: String,
}

/// The answers to a `Query`; items that were not asked for are empty.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Answers {
    pub file_id: String,
    pub version: u64,
    pub declarations: Vec<Declaration>,
    pub references: Vec<Reference>,
    /// Unnecessary imports. Empty when the file does not bind cleanly.
    pub unused_imports: Vec<Span>,
    /// Expressions that are dereferenced while the nullable analysis says they may be null.
    pub maybe_null: Vec<Span>,
    pub diagnostics: Vec<CompilerDiagnostic>,
    /// Compiler errors in the file.
    pub semantic_errors: u32,
}

/// Everything a provider answered about a file for the layers its rules need.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileSemantics {
    pub answers: Answers,
    pub graphs: Vec<BodyGraph>,
}

impl FileSemantics {
    /// The graph of the body whose syntax spans exactly `start..start + len`.
    pub fn graph_at(&self, start: u32, len: u32) -> Option<&BodyGraph> {
        self.graphs
            .iter()
            .find(|g| g.start == start && g.len == len)
    }
}

/// `Shop.Orders.OrderService.Get` for `M:Shop.Orders.OrderService.Get(System.Int32)`: the part of
/// a symbol id that the `semantics:` config names.
pub fn qualified_name(symbol: &str) -> &str {
    let rest = symbol.split_once(':').map_or(symbol, |(_, r)| r);
    rest.split('(').next().unwrap_or(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_names_drop_the_prefix_and_the_signature() {
        assert_eq!(qualified_name("M:A.B.C.Get(System.Int32)"), "A.B.C.Get");
        assert_eq!(qualified_name("M:A.B.C.#ctor"), "A.B.C.#ctor");
        assert_eq!(qualified_name("A.B"), "A.B");
    }
}

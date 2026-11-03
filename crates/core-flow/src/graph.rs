//! The flow graph model. These types are also the worker's wire payload: MessagePack arrays in
//! field order, mirrored by `workers/csharp/Sudsy.Worker/Protocol/FlowMessages.cs`. Enumerations
//! travel as their `u8` codes so both sides agree without naming variants.

use serde::{Deserialize, Serialize};

/// What a body is.
pub mod body_kind {
    pub const METHOD: u8 = 0;
    pub const LAMBDA: u8 = 1;
    pub const LOCAL_FUNCTION: u8 = 2;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpKind {
    /// The variable's value is used.
    Read,
    /// A new value is stored; [`OpFlags::CONSTANT`] says whether it is a compile-time constant.
    Write,
    /// `x += 1`, `x++`, `ref` arguments.
    ReadWrite,
    /// The variable comes into existence with a value that is not the program's own (catch
    /// variables, parameters, loop variables): neither a use nor a store.
    Declare,
    /// A call; `callee` is the target's symbol id.
    Call,
    Other,
}

impl OpKind {
    pub fn from_code(code: u8) -> OpKind {
        match code {
            0 => OpKind::Read,
            1 => OpKind::Write,
            2 => OpKind::ReadWrite,
            3 => OpKind::Declare,
            4 => OpKind::Call,
            _ => OpKind::Other,
        }
    }
}

pub struct OpFlags;

impl OpFlags {
    /// A write of a compile-time constant.
    pub const CONSTANT: u8 = 1;
    /// A call the callee's attributes say never returns.
    pub const NO_RETURN: u8 = 2;
    /// A call the callee's attributes say is pure.
    pub const PURE: u8 = 4;
}

/// One simple operation. Spans are UTF-8 byte offsets into the file text; `stmt_*` is the
/// statement the operation belongs to.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Op {
    pub kind: u8,
    /// Index into [`BodyGraph::locals`], or -1.
    pub local: i32,
    pub start: u32,
    pub len: u32,
    pub stmt_start: u32,
    pub stmt_len: u32,
    pub callee: String,
    pub flags: u8,
}

impl Op {
    pub fn kind(&self) -> OpKind {
        OpKind::from_code(self.kind)
    }

    pub fn has(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    pub fn local(&self) -> Option<usize> {
        usize::try_from(self.local).ok()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Always,
    /// Taken when the block's branch value is true.
    WhenTrue,
    /// Taken when the branch value is false.
    WhenFalse,
    /// From a block of a `try` to the start of a handler (catch, filter, finally).
    Handler,
}

impl EdgeKind {
    pub fn from_code(code: u8) -> EdgeKind {
        match code {
            1 => EdgeKind::WhenTrue,
            2 => EdgeKind::WhenFalse,
            3 => EdgeKind::Handler,
            _ => EdgeKind::Always,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Edge {
    pub to: u32,
    pub kind: u8,
}

impl Edge {
    pub fn kind(&self) -> EdgeKind {
        EdgeKind::from_code(self.kind)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub ops: Vec<Op>,
    pub succ: Vec<Edge>,
    /// The constant value of the block's branch condition, when it has one.
    pub branch_const: Option<bool>,
}

impl Block {
    /// Can the edge ever be taken? A branch on a constant goes one way only.
    pub fn feasible(&self, edge: &Edge) -> bool {
        !matches!(
            (edge.kind(), self.branch_const),
            (EdgeKind::WhenTrue, Some(false)) | (EdgeKind::WhenFalse, Some(true))
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalKind {
    /// A variable declaration statement (or `for` initializer): compilers warn when unused.
    Declaration,
    /// A catch variable.
    Catch,
    /// Loop variables, pattern and `out` variables, `using` variables, ...
    Other,
}

impl LocalKind {
    pub fn from_code(code: u8) -> LocalKind {
        match code {
            0 => LocalKind::Declaration,
            1 => LocalKind::Catch,
            _ => LocalKind::Other,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Local {
    pub name: String,
    /// The declared name.
    pub start: u32,
    pub len: u32,
    pub kind: u8,
    /// [`Local::CAPTURED`] and [`Local::ADDRESS_TAKEN`]: uses the graph cannot place in time.
    pub flags: u8,
}

impl Local {
    pub const CAPTURED: u8 = 1;
    pub const ADDRESS_TAKEN: u8 = 2;

    pub fn kind(&self) -> LocalKind {
        LocalKind::from_code(self.kind)
    }

    /// Is every use visible as an operation of this graph, in the order it happens?
    pub fn tracked(&self) -> bool {
        self.flags == 0
    }
}

/// The flow graph of one method, lambda or local function. Block 0 is the entry; blocks that no
/// path reaches are kept, which is how unreachable code is found.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyGraph {
    pub kind: u8,
    /// The body owner's syntax node, without leading or trailing trivia.
    pub start: u32,
    pub len: u32,
    /// The owner's symbol id (empty for lambdas).
    pub symbol: String,
    pub locals: Vec<Local>,
    pub blocks: Vec<Block>,
}

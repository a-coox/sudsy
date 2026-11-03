//! The flow graph of a method body and the dataflow framework that runs over it (plan.md "Flow
//! graph and analysis framework"). The graph model is language neutral; a provider exports it
//! (C#: converted from Roslyn's operation-based control-flow graph in the worker) and the
//! analyses run here, in the core.

mod analyses;
mod bitset;
mod graph;
mod solver;

pub use analyses::{
    DeadStore, Reachability, assigned_before_use, dead_stores, liveness, reachability,
    reaching_definitions, unassigned_reads,
};
pub use bitset::BitSet;
pub use graph::{
    Block, BodyGraph, Edge, EdgeKind, Local, LocalKind, Op, OpFlags, OpKind, body_kind,
};
pub use solver::{Direction, Lattice, Solution, solve};

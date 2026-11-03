//! Behavior summaries (plan.md): per-method facts for rules and fix preconditions, from
//! attributes, a provider's built-in list for its standard library, and the user's `semantics:`.

use crate::facts::{FileSemantics, qualified_name};
use core_flow::{BodyGraph, Op, OpFlags, Reachability, reachability};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BehaviorSummaries {
    does_not_return: BTreeSet<String>,
    pure: BTreeSet<String>,
}

impl BehaviorSummaries {
    /// Qualified names (see [`qualified_name`]).
    pub fn new<S: AsRef<str>>(does_not_return: &[S], pure: &[S]) -> Self {
        let set = |names: &[S]| names.iter().map(|n| n.as_ref().to_string()).collect();
        BehaviorSummaries {
            does_not_return: set(does_not_return),
            pure: set(pure),
        }
    }

    /// Adds the facts of `other`.
    pub fn merge(&mut self, other: &BehaviorSummaries) {
        self.does_not_return
            .extend(other.does_not_return.iter().cloned());
        self.pure.extend(other.pure.iter().cloned());
    }

    /// Never returns: the callee's attributes say so, or a summary names it.
    pub fn is_no_return(&self, op: &Op) -> bool {
        op.has(OpFlags::NO_RETURN) || self.does_not_return.contains(qualified_name(&op.callee))
    }

    /// Has no side effects.
    pub fn is_pure(&self, op: &Op) -> bool {
        op.has(OpFlags::PURE) || self.pure.contains(qualified_name(&op.callee))
    }
}

/// The semantic data of a file with the summaries that apply to it.
#[derive(Clone, Copy)]
pub struct SemanticView<'a> {
    pub data: &'a FileSemantics,
    pub summaries: &'a BehaviorSummaries,
}

impl<'a> SemanticView<'a> {
    /// The flow of the body whose syntax spans `start..start + len`.
    pub fn body(&self, start: u32, len: u32) -> Option<BodyFlow<'a>> {
        self.data.graph_at(start, len).map(|graph| BodyFlow {
            graph,
            summaries: self.summaries,
        })
    }
}

/// A body's flow graph with the summaries its analyses use.
#[derive(Clone, Copy)]
pub struct BodyFlow<'a> {
    pub graph: &'a BodyGraph,
    pub summaries: &'a BehaviorSummaries,
}

impl BodyFlow<'_> {
    /// Reachability with constant branches and calls that do not return.
    pub fn reachability(&self) -> Reachability {
        reachability(self.graph, &|op| self.summaries.is_no_return(op))
    }

    /// Reachability as the compiler sees it: constants only, no summaries.
    pub fn compiler_reachability(&self) -> Reachability {
        reachability(self.graph, &|_| false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_match_every_overload_and_the_attribute_flag() {
        let s = BehaviorSummaries::new(&["MyApp.Log.Fatal"], &["MyApp.Money.Add"]);
        let call = |callee: &str, flags| Op {
            kind: 4,
            local: -1,
            callee: callee.into(),
            flags,
            ..Op::default()
        };
        assert!(s.is_no_return(&call("M:MyApp.Log.Fatal(System.String)", 0)));
        assert!(!s.is_no_return(&call("M:MyApp.Log.Info(System.String)", 0)));
        assert!(s.is_no_return(&call("M:Other.Quit", OpFlags::NO_RETURN)));
        assert!(s.is_pure(&call("M:MyApp.Money.Add(System.Int32)", 0)));
    }
}

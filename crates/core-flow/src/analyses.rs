//! The v0.1 analyses on the solver: reachability, liveness, reaching definitions and definite
//! assignment.

use crate::bitset::BitSet;
use crate::graph::{Block, BodyGraph, Edge, EdgeKind, Op, OpKind};
use crate::solver::{Direction, Lattice, Solution, solve};

/// Which blocks can run, and where in a block the code stops being reachable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reachability {
    pub block: Vec<bool>,
    /// For a reachable block that contains a call that never returns: the index of the first
    /// operation after it.
    pub cut: Vec<Option<usize>>,
}

impl Reachability {
    /// Can operation `op` of `block` run?
    pub fn op_reachable(&self, block: usize, op: usize) -> bool {
        self.block[block] && self.cut[block].is_none_or(|c| op < c)
    }
}

struct Reach<'a> {
    no_return: &'a dyn Fn(&Op) -> bool,
}

impl Lattice for Reach<'_> {
    type Fact = bool;
    const DIRECTION: Direction = Direction::Forward;

    fn boundary(&self, _: &BodyGraph) -> bool {
        true
    }

    fn initial(&self, _: &BodyGraph) -> bool {
        false
    }

    fn join(&self, into: &mut bool, other: &bool) {
        *into |= *other;
    }

    fn transfer(&self, _: &BodyGraph, _: usize, block: &Block, fact: &mut bool) {
        *fact &= !block.ops.iter().any(self.no_return);
    }

    fn along(&self, block: &Block, edge: &Edge, entry: &bool, exit: &bool) -> Option<bool> {
        if !block.feasible(edge) {
            return None;
        }
        // A handler can run from anywhere in the try, including before a call that never returns.
        Some(if edge.kind() == EdgeKind::Handler {
            *entry
        } else {
            *exit
        })
    }
}

/// Reachability from the entry block. Branches on constants go one way, and code after a call
/// for which `no_return` holds cannot run.
pub fn reachability(graph: &BodyGraph, no_return: &dyn Fn(&Op) -> bool) -> Reachability {
    let solution = solve(graph, &Reach { no_return });
    let cut = graph
        .blocks
        .iter()
        .map(|b| b.ops.iter().position(no_return).map(|i| i + 1))
        .collect();
    Reachability {
        block: solution.entry,
        cut,
    }
}

fn is_read(op: &Op) -> bool {
    matches!(op.kind(), OpKind::Read | OpKind::ReadWrite)
}

struct Live;

impl Lattice for Live {
    type Fact = BitSet;
    const DIRECTION: Direction = Direction::Backward;

    fn boundary(&self, g: &BodyGraph) -> BitSet {
        BitSet::new(g.locals.len())
    }

    fn initial(&self, g: &BodyGraph) -> BitSet {
        BitSet::new(g.locals.len())
    }

    fn join(&self, into: &mut BitSet, other: &BitSet) {
        into.union_with(other);
    }

    fn transfer(&self, _: &BodyGraph, _: usize, block: &Block, fact: &mut BitSet) {
        for op in block.ops.iter().rev() {
            step_live(op, fact);
        }
    }
}

fn step_live(op: &Op, fact: &mut BitSet) {
    let Some(l) = op.local() else { return };
    match op.kind() {
        OpKind::Write | OpKind::Declare => fact.remove(l),
        OpKind::Read | OpKind::ReadWrite => fact.insert(l),
        OpKind::Call | OpKind::Other => {}
    }
}

/// Live locals at the start (`entry`) and end (`exit`) of every block.
pub fn liveness(graph: &BodyGraph) -> Solution<BitSet> {
    solve(graph, &Live)
}

/// A store whose value no later operation can read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeadStore {
    pub block: usize,
    pub op: usize,
}

/// Stores to locals whose value is dead afterwards, in reachable code, for locals whose every
/// use the graph can place (not captured, address not taken).
pub fn dead_stores(graph: &BodyGraph, reach: &Reachability) -> Vec<DeadStore> {
    let live = liveness(graph);
    let mut out = Vec::new();
    for (b, block) in graph.blocks.iter().enumerate() {
        if !reach.block[b] {
            continue;
        }
        let mut fact = live.exit[b].clone();
        for (i, op) in block.ops.iter().enumerate().rev() {
            if op.kind() == OpKind::Write
                && let Some(l) = op.local()
                && graph.locals[l].tracked()
                && !fact.contains(l)
                && reach.op_reachable(b, i)
            {
                out.push(DeadStore { block: b, op: i });
            }
            step_live(op, &mut fact);
        }
    }
    out.reverse();
    out
}

struct Reaching {
    /// For every local, the definitions of it (indices into the definition list).
    by_local: Vec<Vec<usize>>,
    /// For every block, the definition index of each of its defining operations.
    ids: Vec<Vec<Option<usize>>>,
    count: usize,
}

impl Lattice for Reaching {
    type Fact = BitSet;
    const DIRECTION: Direction = Direction::Forward;

    fn boundary(&self, _: &BodyGraph) -> BitSet {
        BitSet::new(self.count)
    }

    fn initial(&self, _: &BodyGraph) -> BitSet {
        BitSet::new(self.count)
    }

    fn join(&self, into: &mut BitSet, other: &BitSet) {
        into.union_with(other);
    }

    fn transfer(&self, _: &BodyGraph, b: usize, block: &Block, fact: &mut BitSet) {
        for (i, op) in block.ops.iter().enumerate() {
            if let (Some(id), Some(l)) = (self.ids[b][i], op.local()) {
                for &d in &self.by_local[l] {
                    fact.remove(d);
                }
                fact.insert(id);
            }
        }
    }
}

/// The definitions (block, operation) of the graph, and which of them reach the start and end of
/// every block. Bit `n` of a fact is definition `n`.
pub fn reaching_definitions(graph: &BodyGraph) -> (Vec<(usize, usize)>, Solution<BitSet>) {
    let mut defs = Vec::new();
    let mut by_local = vec![Vec::new(); graph.locals.len()];
    let mut ids = Vec::new();
    for (b, block) in graph.blocks.iter().enumerate() {
        let mut row = Vec::new();
        for (i, op) in block.ops.iter().enumerate() {
            let defines = matches!(
                op.kind(),
                OpKind::Write | OpKind::ReadWrite | OpKind::Declare
            );
            match op.local() {
                Some(l) if defines => {
                    by_local[l].push(defs.len());
                    row.push(Some(defs.len()));
                    defs.push((b, i));
                }
                _ => row.push(None),
            }
        }
        ids.push(row);
    }
    let analysis = Reaching {
        by_local,
        ids,
        count: defs.len(),
    };
    let solution = solve(graph, &analysis);
    (defs, solution)
}

struct Assigned;

impl Lattice for Assigned {
    type Fact = BitSet;
    const DIRECTION: Direction = Direction::Forward;

    fn boundary(&self, g: &BodyGraph) -> BitSet {
        BitSet::new(g.locals.len())
    }

    fn initial(&self, g: &BodyGraph) -> BitSet {
        BitSet::full(g.locals.len())
    }

    fn join(&self, into: &mut BitSet, other: &BitSet) {
        into.intersect_with(other);
    }

    fn transfer(&self, _: &BodyGraph, _: usize, block: &Block, fact: &mut BitSet) {
        for op in &block.ops {
            step_assigned(op, fact);
        }
    }
}

fn step_assigned(op: &Op, fact: &mut BitSet) {
    if let (Some(l), OpKind::Write | OpKind::ReadWrite | OpKind::Declare) = (op.local(), op.kind())
    {
        fact.insert(l);
    }
}

/// The locals that are definitely assigned at the start and end of every block.
pub fn assigned_before_use(graph: &BodyGraph) -> Solution<BitSet> {
    solve(graph, &Assigned)
}

/// Reads of locals that are not assigned on every path to them, in reachable code.
pub fn unassigned_reads(graph: &BodyGraph, reach: &Reachability) -> Vec<(usize, usize)> {
    let solution = assigned_before_use(graph);
    let mut out = Vec::new();
    for (b, block) in graph.blocks.iter().enumerate() {
        if !reach.block[b] {
            continue;
        }
        let mut fact = solution.entry[b].clone();
        for (i, op) in block.ops.iter().enumerate() {
            if let Some(l) = op.local()
                && is_read(op)
                && !fact.contains(l)
                && reach.op_reachable(b, i)
            {
                out.push((b, i));
            }
            step_assigned(op, &mut fact);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::{Edge, Local, LocalKind, OpFlags};

    fn op(kind: OpKind, local: i32) -> Op {
        Op {
            kind: kind as u8,
            local,
            ..Op::default()
        }
    }

    fn block(ops: Vec<Op>, succ: &[(u32, EdgeKind)]) -> Block {
        Block {
            ops,
            succ: succ
                .iter()
                .map(|&(to, k)| Edge { to, kind: k as u8 })
                .collect(),
            branch_const: None,
        }
    }

    fn graph(locals: usize, blocks: Vec<Block>) -> BodyGraph {
        BodyGraph {
            locals: (0..locals)
                .map(|i| Local {
                    name: format!("v{i}"),
                    kind: LocalKind::Declaration as u8,
                    ..Local::default()
                })
                .collect(),
            blocks,
            ..BodyGraph::default()
        }
    }

    const NEVER: &dyn Fn(&Op) -> bool = &|_| false;

    #[test]
    fn constant_branches_and_dead_ends_make_code_unreachable() {
        // 0: if (false) 1 else 2;  1: dead;  2: exit
        let mut b0 = block(vec![], &[(1, EdgeKind::WhenTrue), (2, EdgeKind::WhenFalse)]);
        b0.branch_const = Some(false);
        let g = graph(
            0,
            vec![
                b0,
                block(vec![], &[(2, EdgeKind::Always)]),
                block(vec![], &[]),
            ],
        );
        assert_eq!(reachability(&g, NEVER).block, [true, false, true]);
    }

    #[test]
    fn handlers_are_reachable_from_their_try_and_calls_that_do_not_return_cut_blocks() {
        let mut call = op(OpKind::Call, -1);
        call.flags = OpFlags::NO_RETURN;
        let g = graph(
            0,
            vec![
                block(
                    vec![call, op(OpKind::Other, -1)],
                    &[(2, EdgeKind::Always), (1, EdgeKind::Handler)],
                ),
                block(vec![], &[(2, EdgeKind::Always)]),
                block(vec![], &[]),
            ],
        );
        let r = reachability(&g, &|o| o.has(OpFlags::NO_RETURN));
        assert_eq!(r.block, [true, true, true]);
        assert_eq!(r.cut[0], Some(1));
        assert!(r.op_reachable(0, 0) && !r.op_reachable(0, 1));
        // Without the call's summary the block runs to its successor, with it only the handler does.
        let r = reachability(
            &graph(
                0,
                vec![
                    block(
                        vec![{
                            let mut c = op(OpKind::Call, -1);
                            c.flags = OpFlags::NO_RETURN;
                            c
                        }],
                        &[(1, EdgeKind::Always)],
                    ),
                    block(vec![], &[]),
                ],
            ),
            &|o| o.has(OpFlags::NO_RETURN),
        );
        assert_eq!(r.block, [true, false]);
    }

    #[test]
    fn liveness_finds_dead_stores_through_loops() {
        // 0: x = 1 (dead: overwritten);  x = 2; -> 1: loop { read x; x = 3 (live: read next time) } -> 2
        let g = graph(
            1,
            vec![
                block(
                    vec![op(OpKind::Write, 0), op(OpKind::Write, 0)],
                    &[(1, EdgeKind::Always)],
                ),
                block(
                    vec![op(OpKind::Read, 0), op(OpKind::Write, 0)],
                    &[(1, EdgeKind::WhenTrue), (2, EdgeKind::WhenFalse)],
                ),
                block(vec![], &[]),
            ],
        );
        let reach = reachability(&g, NEVER);
        assert_eq!(dead_stores(&g, &reach), [DeadStore { block: 0, op: 0 }]);
    }

    #[test]
    fn a_store_read_by_a_handler_is_live() {
        let g = graph(
            1,
            vec![
                block(
                    vec![op(OpKind::Write, 0), op(OpKind::Call, -1)],
                    &[(2, EdgeKind::Always), (1, EdgeKind::Handler)],
                ),
                block(vec![op(OpKind::Read, 0)], &[(2, EdgeKind::Always)]),
                block(vec![], &[]),
            ],
        );
        assert!(dead_stores(&g, &reachability(&g, NEVER)).is_empty());
    }

    #[test]
    fn captured_locals_have_no_dead_stores() {
        let mut g = graph(1, vec![block(vec![op(OpKind::Write, 0)], &[])]);
        g.locals[0].flags = Local::CAPTURED;
        assert!(dead_stores(&g, &reachability(&g, NEVER)).is_empty());
    }

    #[test]
    fn reaching_definitions_merge_at_joins() {
        // 0 -> 1 (x = 1), 0 -> 2 (x = 2), both -> 3 reads x.
        let g = graph(
            1,
            vec![
                block(vec![], &[(1, EdgeKind::WhenTrue), (2, EdgeKind::WhenFalse)]),
                block(vec![op(OpKind::Write, 0)], &[(3, EdgeKind::Always)]),
                block(vec![op(OpKind::Write, 0)], &[(3, EdgeKind::Always)]),
                block(vec![op(OpKind::Read, 0)], &[]),
            ],
        );
        let (defs, s) = reaching_definitions(&g);
        assert_eq!(defs, [(1, 0), (2, 0)]);
        assert_eq!(s.entry[3].iter().collect::<Vec<_>>(), [0, 1]);
    }

    #[test]
    fn definite_assignment_needs_every_path() {
        let g = graph(
            1,
            vec![
                block(vec![], &[(1, EdgeKind::WhenTrue), (2, EdgeKind::WhenFalse)]),
                block(vec![op(OpKind::Write, 0)], &[(2, EdgeKind::Always)]),
                block(vec![op(OpKind::Read, 0)], &[]),
            ],
        );
        let reach = reachability(&g, NEVER);
        assert_eq!(unassigned_reads(&g, &reach), [(2, 0)]);
        let ok = graph(
            1,
            vec![block(vec![op(OpKind::Write, 0), op(OpKind::Read, 0)], &[])],
        );
        assert!(unassigned_reads(&ok, &reachability(&ok, NEVER)).is_empty());
    }
}

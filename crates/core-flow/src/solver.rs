//! A generic worklist dataflow solver, forward or backward, over any lattice.

use crate::graph::{Block, BodyGraph, Edge};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Backward,
}

/// An analysis: a fact per program point, how blocks change it and how facts merge.
pub trait Lattice {
    type Fact: Clone + PartialEq;

    const DIRECTION: Direction;

    /// The fact at the boundary: the entry block (forward) or the blocks without successors
    /// (backward).
    fn boundary(&self, graph: &BodyGraph) -> Self::Fact;

    /// The fact every other block starts with: the lattice's bottom for a may-analysis, its top
    /// for a must-analysis.
    fn initial(&self, graph: &BodyGraph) -> Self::Fact;

    /// Merges `other` into `into`.
    fn join(&self, into: &mut Self::Fact, other: &Self::Fact);

    /// Carries a fact through a block: from its start to its end (forward) or from its end to its
    /// start (backward).
    fn transfer(&self, graph: &BodyGraph, index: usize, block: &Block, fact: &mut Self::Fact);

    /// What flows along `edge` out of `block` (`entry` and `exit` are the block's facts in
    /// program order), or nothing if the edge cannot be taken. The default: the block's end fact
    /// along every edge that a constant branch condition allows.
    fn along(
        &self,
        block: &Block,
        edge: &Edge,
        _entry: &Self::Fact,
        exit: &Self::Fact,
    ) -> Option<Self::Fact> {
        block.feasible(edge).then(|| exit.clone())
    }
}

/// The fixed point: the fact at the start and at the end of every block, in program order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solution<F> {
    pub entry: Vec<F>,
    pub exit: Vec<F>,
}

pub fn solve<L: Lattice>(graph: &BodyGraph, lattice: &L) -> Solution<L::Fact> {
    let n = graph.blocks.len();
    let initial = lattice.initial(graph);
    let mut entry = vec![initial.clone(); n];
    let mut exit = vec![initial; n];
    if n == 0 {
        return Solution { entry, exit };
    }
    let forward = L::DIRECTION == Direction::Forward;
    // Predecessors, for the backward direction's "flows from successors" and to requeue.
    let mut preds: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (b, block) in graph.blocks.iter().enumerate() {
        for e in &block.succ {
            if let Some(p) = preds.get_mut(e.to as usize) {
                p.push(b);
            }
        }
    }
    let mut work: Vec<usize> = (0..n).collect();
    if forward {
        work.reverse();
        entry[0] = lattice.boundary(graph);
    }
    let mut queued = vec![true; n];
    while let Some(b) = work.pop() {
        queued[b] = false;
        let block = &graph.blocks[b];
        if forward {
            let mut fact = entry[b].clone();
            lattice.transfer(graph, b, block, &mut fact);
            exit[b] = fact;
            for e in &block.succ {
                let to = e.to as usize;
                let Some(flow) = lattice.along(block, e, &entry[b], &exit[b]) else {
                    continue;
                };
                let mut joined = entry[to].clone();
                lattice.join(&mut joined, &flow);
                if joined != entry[to] {
                    entry[to] = joined;
                    if !queued[to] {
                        queued[to] = true;
                        work.push(to);
                    }
                }
            }
        } else {
            let mut fact = if block.succ.is_empty() {
                lattice.boundary(graph)
            } else {
                lattice.initial(graph)
            };
            for e in &block.succ {
                let to = e.to as usize;
                if let Some(flow) = lattice.along(block, e, &entry[to], &entry[to]) {
                    lattice.join(&mut fact, &flow);
                }
            }
            exit[b] = fact.clone();
            lattice.transfer(graph, b, block, &mut fact);
            if fact != entry[b] {
                entry[b] = fact;
                for &p in &preds[b] {
                    if !queued[p] {
                        queued[p] = true;
                        work.push(p);
                    }
                }
            }
        }
    }
    Solution { entry, exit }
}

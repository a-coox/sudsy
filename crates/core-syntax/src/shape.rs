//! Checks a tree against the bodies of its grammar: every child of a node must land in a slot of
//! the node's body ([`crate::ast::assign`], the routine the generated accessors use), and every
//! required item must be present. Conformance suites run it over real files, which is what keeps
//! a grammar's bodies honest.

use crate::ast::{Pattern, Slot, assign};
use crate::grammar::{Cardinality, Grammar, Target};
use crate::kind::SyntaxKind;
use crate::tree::{SyntaxElement, SyntaxNode};
use std::collections::HashMap;

/// A body resolved to kind lists.
struct Body {
    kinds: Vec<Vec<SyntaxKind>>,
    cardinality: Vec<Cardinality>,
}

pub struct ShapeChecker<'g> {
    grammar: &'g Grammar,
    skip: Vec<SyntaxKind>,
    bodies: HashMap<SyntaxKind, Body>,
}

impl<'g> ShapeChecker<'g> {
    pub fn new(grammar: &'g Grammar) -> Self {
        let mut skip: Vec<SyntaxKind> = std::iter::once(SyntaxKind::ERROR)
            .chain(
                grammar
                    .tokens()
                    .iter()
                    .chain(grammar.nodes())
                    .filter(|d| d.has_tag("trivia"))
                    .map(|d| d.kind),
            )
            .collect();
        skip.sort();
        let bodies = grammar
            .nodes()
            .iter()
            .map(|d| {
                let kinds = d
                    .body
                    .iter()
                    .map(|item| {
                        let mut kinds: Vec<SyntaxKind> = match &item.target {
                            Target::Kind(k) => vec![*k],
                            Target::Tag(t) => grammar.kinds_with_tag(t).map(|d| d.kind).collect(),
                        };
                        kinds.sort();
                        kinds
                    })
                    .collect();
                let cardinality = d.body.iter().map(|i| i.cardinality.clone()).collect();
                (d.kind, Body { kinds, cardinality })
            })
            .collect();
        ShapeChecker {
            grammar,
            skip,
            bodies,
        }
    }

    /// The first node whose children do not fit its body, described. Nodes that contain an `ERROR`
    /// child are skipped: recovery trees need not fit.
    pub fn check(&self, root: &SyntaxNode) -> Result<(), String> {
        for node in root.descendants() {
            self.check_node(&node)?;
        }
        Ok(())
    }

    fn check_node(&self, node: &SyntaxNode) -> Result<(), String> {
        let Some(body) = self.bodies.get(&node.kind()) else {
            return Ok(());
        };
        if node.children().any(|c| c.kind() == SyntaxKind::ERROR) {
            return Ok(());
        }
        let slots: Vec<Slot> = body
            .kinds
            .iter()
            .zip(&body.cardinality)
            .map(|(kinds, c)| Slot {
                kinds,
                many: *c == Cardinality::Many,
            })
            .collect();
        let pattern = Pattern {
            slots: &slots,
            skip: &self.skip,
        };
        let mut counts = vec![0usize; slots.len()];
        let mut assigned = assign(node, &pattern).peekable();
        let describe = |what: String| {
            format!(
                "{} at {:?}: {what}",
                self.grammar.name(node.kind()),
                node.text_range()
            )
        };
        for child in node.children_with_tokens() {
            if self.skip.binary_search(&child.kind()).is_ok() {
                continue;
            }
            match assigned.peek() {
                Some((index, c)) if *c == child => {
                    counts[*index] += 1;
                    assigned.next();
                }
                _ if self.is_separator(&child) => {}
                _ => {
                    return Err(describe(format!(
                        "child {} at {:?} fits no item of the body",
                        self.grammar.name(child.kind()),
                        child.text_range()
                    )));
                }
            }
        }
        for (i, (count, c)) in counts.iter().zip(&body.cardinality).enumerate() {
            let bad = match c {
                Cardinality::One => *count != 1,
                Cardinality::Optional => *count > 1,
                Cardinality::Many => false,
            };
            if bad {
                let item = &self.grammar.def(node.kind()).expect("has a body").body[i];
                return Err(describe(format!(
                    "item {} has {count} children, expected {}",
                    item.label.as_deref().unwrap_or("(unlabeled)"),
                    match c {
                        Cardinality::One => "1",
                        _ => "at most 1",
                    }
                )));
            }
        }
        Ok(())
    }

    fn is_separator(&self, child: &SyntaxElement) -> bool {
        self.grammar.has_tag(child.kind(), "separator")
    }
}

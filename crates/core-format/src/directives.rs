//! Layout directives: the fixed, versioned set of layout requests that rules can attach to
//! nodes. Rules never print layout themselves; the rule engine
//! (M2) fills a [`DirectiveTable`], and shapes read it.

use core_syntax::{SyntaxKind, SyntaxNode};
use std::collections::HashMap;

/// Bumped when a directive is added, removed or changes meaning.
pub const DIRECTIVES_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Directive {
    /// Break every item of a list or chain, one per line, even if it fits.
    BreakAll,
    /// Do not break inside this node unless it cannot fit.
    KeepTogether,
    /// Force a line break before this node.
    BreakBefore,
    /// Force a line break after this node.
    BreakAfter,
    /// Require this many blank lines before this node.
    BlankLineBefore(u8),
    /// Align the keys of the key-value children of this node.
    Align,
    /// Leave this node exactly as written.
    Preserve,
    /// Pick one of the node's built-in variants (read by provider formatting code).
    Style(String),
}

impl Directive {
    fn conflicts_with(&self, other: &Directive) -> bool {
        use Directive::*;
        match (self, other) {
            (BreakAll, KeepTogether) | (KeepTogether, BreakAll) => true,
            (Preserve, o) | (o, Preserve) => *o != Preserve,
            (BlankLineBefore(a), BlankLineBefore(b)) => a != b,
            (Style(a), Style(b)) => a != b,
            _ => false,
        }
    }
}

/// Identifies a node by position and kind, so tables survive re-parses of unchanged text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey {
    kind: SyntaxKind,
    start: u32,
    end: u32,
}

impl NodeKey {
    pub fn of(node: &SyntaxNode) -> Self {
        let r = node.text_range();
        NodeKey {
            kind: node.kind(),
            start: r.start().into(),
            end: r.end().into(),
        }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error(
    "rule `{first_rule}` ({first:?}) and rule `{second_rule}` ({second:?}) request conflicting layouts for the same {kind} at {start}..{end}"
)]
pub struct DirectiveConflict {
    pub kind: String,
    pub start: u32,
    pub end: u32,
    pub first_rule: String,
    pub first: Directive,
    pub second_rule: String,
    pub second: Directive,
}

#[derive(Clone, Debug, Default)]
pub struct DirectiveTable {
    by_node: HashMap<NodeKey, Vec<(String, Directive)>>,
}

impl DirectiveTable {
    /// Attaches `directive`, requested by `rule`. Conflicting directives on one node are an
    /// error, never resolved silently.
    pub fn add(
        &mut self,
        node: &SyntaxNode,
        rule: &str,
        directive: Directive,
    ) -> Result<(), Box<DirectiveConflict>> {
        let key = NodeKey::of(node);
        let entries = self.by_node.entry(key).or_default();
        if let Some((first_rule, first)) =
            entries.iter().find(|(_, d)| d.conflicts_with(&directive))
        {
            return Err(Box::new(DirectiveConflict {
                kind: format!("{:?}", node.kind()),
                start: key.start,
                end: key.end,
                first_rule: first_rule.clone(),
                first: first.clone(),
                second_rule: rule.into(),
                second: directive,
            }));
        }
        if !entries.iter().any(|(_, d)| *d == directive) {
            entries.push((rule.into(), directive));
        }
        Ok(())
    }

    pub fn of(&self, node: &SyntaxNode) -> NodeDirectives<'_> {
        NodeDirectives(
            self.by_node
                .get(&NodeKey::of(node))
                .map_or(&[][..], |v| v.as_slice()),
        )
    }
}

/// The directives on one node.
#[derive(Clone, Copy)]
pub struct NodeDirectives<'a>(&'a [(String, Directive)]);

impl NodeDirectives<'_> {
    pub fn has(&self, d: &Directive) -> bool {
        self.0.iter().any(|(_, x)| x == d)
    }

    pub fn blank_lines_before(&self) -> Option<usize> {
        self.0.iter().find_map(|(_, d)| match d {
            Directive::BlankLineBefore(n) => Some(*n as usize),
            _ => None,
        })
    }

    pub fn style(&self) -> Option<&str> {
        self.0.iter().find_map(|(_, d)| match d {
            Directive::Style(s) => Some(s.as_str()),
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core_syntax::{TreeBuilder, grammars};

    fn node() -> SyntaxNode {
        let g = grammars::json();
        let mut b = TreeBuilder::new();
        b.start_node(g.kind_by_name("Array").unwrap());
        b.token(g.kind_by_name("LBracket").unwrap(), "[");
        b.token(g.kind_by_name("RBracket").unwrap(), "]");
        b.finish_node();
        SyntaxNode::new_root(b.finish())
    }

    #[test]
    fn compatible_directives_coexist_and_duplicates_collapse() {
        let n = node();
        let mut t = DirectiveTable::default();
        t.add(&n, "a", Directive::BreakAll).unwrap();
        t.add(&n, "b", Directive::BreakAll).unwrap();
        t.add(&n, "b", Directive::BreakBefore).unwrap();
        t.add(&n, "b", Directive::BlankLineBefore(1)).unwrap();
        assert!(t.of(&n).has(&Directive::BreakAll));
        assert_eq!(t.of(&n).blank_lines_before(), Some(1));
        assert_eq!(t.of(&n).0.len(), 3);
    }

    #[test]
    fn conflicts_are_errors() {
        let n = node();
        let cases = [
            (Directive::BreakAll, Directive::KeepTogether),
            (Directive::Preserve, Directive::Align),
            (Directive::BlankLineBefore(1), Directive::BlankLineBefore(2)),
            (Directive::Style("a".into()), Directive::Style("b".into())),
        ];
        for (first, second) in cases {
            let mut t = DirectiveTable::default();
            t.add(&n, "x", first.clone()).unwrap();
            let e = t.add(&n, "y", second.clone()).unwrap_err();
            assert_eq!((e.first_rule.as_str(), e.second_rule.as_str()), ("x", "y"));
            assert_eq!((e.first, e.second), (first, second));
        }
    }
}

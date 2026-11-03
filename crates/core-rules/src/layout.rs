//! Layout directives from rules: a rule calls [`RuleContext::attach`] on the nodes it wants laid
//! out a certain way, and the formatter reads the resulting [`DirectiveTable`]. Rules request,
//! the core performs. The rules run in a pass of their own, so formatting depends on the layout
//! rules only, not on the rest of the lint results.

use crate::engine::{ActiveRule, traverse};
use crate::rule::{FileContext, RuleContext};
use crate::suppress::Suppressions;
use core_format::{Directive, DirectiveTable, NodeKey};
use core_syntax::{SyntaxNode, TextSize};

/// What a rule does with layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutRole {
    /// Reports diagnostics only.
    None,
    /// Reports diagnostics and attaches directives.
    Also,
    /// Attaches directives, reports nothing: it is not run when linting.
    Only,
}

impl LayoutRole {
    pub fn attaches(self) -> bool {
        self != LayoutRole::None
    }

    pub fn reports(self) -> bool {
        self != LayoutRole::Only
    }
}

/// A directive a rule attached, before suppressions and conflict checks.
pub struct Attach {
    pub key: NodeKey,
    /// Where the node starts, trivia excluded: the line a suppression comment applies to.
    pub at: TextSize,
    pub rule: String,
    pub directive: Directive,
}

#[derive(Default)]
pub(crate) struct LayoutSink {
    attached: Vec<Attach>,
    errors: Vec<String>,
}

impl RuleContext<'_> {
    /// Does this run collect layout (the formatting pass), as opposed to lint?
    pub fn laying_out(&self) -> bool {
        self.layout.is_some()
    }

    /// Asks for `directive` on `node`. Ignored when linting.
    pub fn attach(&mut self, node: &SyntaxNode, directive: Directive) {
        let at = crate::tree::significant_range(node, self.file.grammar).start();
        self.attach_at(NodeKey::of(node), at, directive);
    }

    /// [`RuleContext::attach`] for a node known by key and start (a plugin names nodes that way).
    pub fn attach_at(&mut self, key: NodeKey, at: TextSize, directive: Directive) {
        let rule = self.meta.id.to_string();
        if let Some(sink) = self.layout.as_deref_mut() {
            sink.attached.push(Attach {
                key,
                at,
                rule,
                directive,
            });
        }
    }

    /// The rule could not run (a plugin trapped): the formatting pass fails with `reason`.
    pub fn layout_failed(&mut self, reason: String) {
        if let Some(sink) = self.layout.as_deref_mut() {
            sink.errors.push(reason);
        }
    }
}

/// What the rules among `active` attach to the file, in document order, nothing silenced yet.
/// Errors: a rule that failed.
pub fn attached(file: &FileContext<'_>, active: &[ActiveRule<'_>]) -> Result<Vec<Attach>, String> {
    let mut sink = LayoutSink::default();
    traverse(file, active, false, Some(&mut sink));
    if sink.errors.is_empty() {
        Ok(sink.attached)
    } else {
        Err(sink.errors.join("; "))
    }
}

/// The directive table of the layout rules among `active`: what they attach, minus what
/// suppression comments silence. Errors: a rule that failed, or two rules asking for conflicting
/// layouts of one node.
pub fn run_layout(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
) -> Result<DirectiveTable, String> {
    let attached = attached(file, active)?;
    let suppressions = Suppressions::scan(file);
    let mut table = DirectiveTable::default();
    for a in attached {
        if !suppressions.suppresses_at(file, &a.rule, a.at) {
            let kind = a.key.kind();
            table.add_at(a.key, &a.rule, a.directive).map_err(|mut e| {
                e.kind = file.grammar.name(kind).to_string();
                e.to_string()
            })?;
        }
    }
    Ok(table)
}

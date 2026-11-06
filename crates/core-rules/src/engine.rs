//! The rule engine: one traversal per file, dispatching nodes to the rules subscribed to their
//! kind, then the file-scope and body-scope calls, then suppressions.

use crate::diagnostic::{Diagnostic, Severity};
use crate::layout::LayoutSink;
use crate::options::RuleOptions;
use crate::project::{ProjectContext, ProjectFacts};
use crate::rule::{BodyContext, FileContext, Rule, RuleContext};
use crate::suppress::Suppressions;
use core_semantics::Layers;
use core_syntax::SyntaxKind;
use std::path::PathBuf;

/// A rule that is on for a file: its severity and checked options.
pub struct ActiveRule<'a> {
    pub rule: &'a dyn Rule,
    pub severity: Severity,
    pub options: RuleOptions,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileReport {
    /// Sorted by position, then rule.
    pub diagnostics: Vec<Diagnostic>,
    /// What the rules collected for the project phase.
    pub facts: ProjectFacts,
}

/// The layers the active rules need: what the provider is asked to answer for the file.
pub fn layers_needed(active: &[ActiveRule<'_>]) -> Layers {
    let mut layers = Layers::default();
    for a in active {
        let meta = a.rule.meta();
        for &layer in meta.requires.iter().chain(meta.uses) {
            layers.insert(layer);
        }
    }
    layers
}

/// Runs `active` over one file and drops what a suppression comment silences. `collect` says
/// whether project facts are wanted (analysis depth `deep`).
pub fn run_file(file: &FileContext<'_>, active: &[ActiveRule<'_>], collect: bool) -> FileReport {
    let mut report = run_rules(file, active, collect);
    let suppressions = Suppressions::scan(file);
    report
        .diagnostics
        .retain(|d| !suppressions.suppresses(file, d));
    report
}

/// [`run_file`] without the suppressions: for a caller whose own caller applies them to the
/// result (a plugin, whose host does).
pub fn run_rules(file: &FileContext<'_>, active: &[ActiveRule<'_>], collect: bool) -> FileReport {
    traverse(file, active, collect, None)
}

/// The traversal behind [`run_rules`] and [`crate::layout::run_layout`]; `layout` is where the
/// rules' `attach` calls go (they are ignored without it).
pub(crate) fn traverse(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    collect: bool,
    mut layout: Option<&mut LayoutSink>,
) -> FileReport {
    let mut diagnostics = Vec::new();
    // Sorted by kind: a handful of kinds, looked up once per node, so a binary search beats hashing.
    let mut by_kind: Vec<(SyntaxKind, Vec<usize>)> = Vec::new();
    for (i, a) in active.iter().enumerate() {
        // A tag resolves through the file's grammar to the node kinds carrying it.
        let tagged = a
            .rule
            .tags()
            .iter()
            .flat_map(|t| file.grammar.nodes_with_tag(t));
        for &kind in a.rule.kinds().iter().chain(tagged) {
            match by_kind.binary_search_by_key(&kind, |e| e.0) {
                Ok(at) if by_kind[at].1.last() != Some(&i) => by_kind[at].1.push(i),
                Ok(_) => {}
                Err(at) => by_kind.insert(at, (kind, vec![i])),
            }
        }
    }
    let ranges = crate::tree::TraversalCache::default();
    let mut run = |index: usize, f: &mut dyn FnMut(&dyn Rule, &mut RuleContext<'_>)| {
        let a = &active[index];
        let mut ctx = RuleContext::new(
            file,
            &a.options,
            a.rule.meta(),
            a.severity,
            &mut diagnostics,
        );
        ctx.layout = layout.as_deref_mut();
        ctx.ranges = Some(&ranges);
        f(a.rule, &mut ctx);
    };
    for node in file.root.descendants() {
        if let Ok(at) = by_kind.binary_search_by_key(&node.kind(), |e| e.0) {
            for &i in &by_kind[at].1 {
                run(i, &mut |rule, ctx| rule.check(&node, ctx));
            }
        }
        if file.body_kinds.contains(&node.kind()) {
            let flow = file.semantics.as_ref().and_then(|view| {
                let range = crate::tree::significant_range(&node, file.grammar);
                view.body(range.start().into(), range.len().into())
            });
            let body = BodyContext {
                file,
                node: &node,
                flow,
            };
            for i in 0..active.len() {
                run(i, &mut |rule, ctx| rule.check_body(&body, ctx));
            }
        }
    }
    for i in 0..active.len() {
        run(i, &mut |rule, ctx| rule.check_file(file, ctx));
    }
    diagnostics.sort_by(|a, b| {
        (a.range.start(), a.range.end(), &a.rule).cmp(&(b.range.start(), b.range.end(), &b.rule))
    });

    let mut facts = ProjectFacts::default();
    if collect {
        for a in active {
            facts.begin(a.rule.meta().id, file.path);
            a.rule.collect(file, &mut facts);
        }
        facts.end();
    }
    FileReport { diagnostics, facts }
}

/// The project phase: every active rule sees the facts gathered from all files.
pub fn run_project(active: &[ActiveRule<'_>], facts: &ProjectFacts) -> Vec<(PathBuf, Diagnostic)> {
    let mut out = Vec::new();
    for a in active {
        let mut ctx = ProjectContext {
            options: &a.options,
            meta: a.rule.meta(),
            severity: a.severity,
            out: &mut out,
        };
        a.rule.finish(facts, &mut ctx);
    }
    out
}

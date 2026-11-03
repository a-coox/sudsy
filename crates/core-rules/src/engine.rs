//! The rule engine: one traversal per file, dispatching nodes to the rules subscribed to their
//! kind, then the file-scope and body-scope calls, then suppressions.

use crate::diagnostic::{Diagnostic, Severity};
use crate::options::RuleOptions;
use crate::project::{ProjectContext, ProjectFacts};
use crate::rule::{BodyContext, FileContext, Rule, RuleContext};
use crate::suppress::Suppressions;
use core_syntax::SyntaxKind;
use std::collections::HashMap;
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

/// Runs `active` over one file. `body_kinds` are the provider's methods, lambdas and local
/// functions; `collect` says whether project facts are wanted (analysis depth `deep`).
pub fn run_file(
    file: &FileContext<'_>,
    active: &[ActiveRule<'_>],
    body_kinds: &[SyntaxKind],
    collect: bool,
) -> FileReport {
    let mut diagnostics = Vec::new();
    let mut by_kind: HashMap<SyntaxKind, Vec<usize>> = HashMap::new();
    for (i, a) in active.iter().enumerate() {
        for &kind in a.rule.kinds() {
            by_kind.entry(kind).or_default().push(i);
        }
    }
    let mut run = |index: usize, f: &mut dyn FnMut(&dyn Rule, &mut RuleContext<'_>)| {
        let a = &active[index];
        let mut ctx = RuleContext::new(
            file,
            &a.options,
            a.rule.meta(),
            a.severity,
            &mut diagnostics,
        );
        f(a.rule, &mut ctx);
    };
    for node in file.root.descendants() {
        if let Some(indices) = by_kind.get(&node.kind()) {
            for &i in indices {
                run(i, &mut |rule, ctx| rule.check(&node, ctx));
            }
        }
        if body_kinds.contains(&node.kind()) {
            let body = BodyContext { file, node: &node };
            for i in 0..active.len() {
                run(i, &mut |rule, ctx| rule.check_body(&body, ctx));
            }
        }
    }
    for i in 0..active.len() {
        run(i, &mut |rule, ctx| rule.check_file(file, ctx));
    }
    let suppressions = Suppressions::scan(file);
    diagnostics.retain(|d| !suppressions.suppresses(file, d));
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

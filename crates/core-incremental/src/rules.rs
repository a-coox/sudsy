//! Rule results as queries (plan.md "Incremental engine"): `lint(file)` reads `parse` and the
//! resolved rule configuration, so an edit of a file or of the config re-runs only what depends on it.

use crate::db::Db;
use crate::failure::{Failure, Stage};
use crate::inputs::File;
use crate::queries::{config, parse, provider_of, selection};
use core_config::{Depth, FileInput, ResolvedRule};
use core_rules::engine::{ActiveRule, FileReport};
use core_rules::{
    Category, Diagnostic, FileContext, Layer, LineIndex, ProjectFacts, SYNTAX_ERROR, Severity,
    activate, run_file, run_project,
};
use core_syntax::{TextRange, TextSize};
use provider_api::{Capabilities, ParseError};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The diagnostics of a file and the facts its rules collected for the project phase.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Linted {
    pub diagnostics: Vec<Diagnostic>,
    pub facts: ProjectFacts,
    /// False when the file has syntax errors: they are the only diagnostics, since rules do not
    /// run on a tree that is not trustworthy, and fixes must not be applied.
    pub parses: bool,
}

fn syntax_error(e: &ParseError) -> Diagnostic {
    Diagnostic {
        rule: SYNTAX_ERROR.into(),
        category: Category::Correctness,
        severity: Severity::Error,
        message: format!("{}: {}", e.code, e.message),
        range: TextRange::at(TextSize::from(e.start), TextSize::from(e.len)),
        related: Vec::new(),
        fixes: Vec::new(),
    }
}

/// Is the layer answered at this depth? Syntax-only analysis (`fast`) has none; the project layer
/// needs `deep`.
fn has_layer(depth: Depth, caps: Capabilities, layer: Layer) -> bool {
    match (depth, layer) {
        (Depth::Fast, _) => false,
        (Depth::Standard, Layer::Project) => false,
        (_, Layer::Binding) => caps.binding,
        (_, Layer::Types) => caps.types,
        (_, Layer::Values) => caps.values,
        (_, Layer::Flow) => caps.flow,
        (_, Layer::Project) => caps.project,
    }
}

/// Rule levels and options of a file and the analysis depth. A separate query so that a config
/// edit that leaves them as they were (a format option, say) does not re-run the rules.
#[derive(Clone, Debug, PartialEq)]
pub struct LintSettings {
    pub rules: BTreeMap<String, ResolvedRule>,
    pub depth: Depth,
}

#[salsa::tracked(returns(ref))]
pub fn lint_settings(db: &dyn Db, file: File) -> Result<LintSettings, Failure> {
    let provider = provider_of(db, file)?;
    let config = config(db).as_ref().map_err(Clone::clone)?;
    let input = FileInput::language(&provider.grammar().language).with_path(file.path(db));
    let rules = config
        .rules(&input, provider.option_schema())
        .map_err(|e| Failure::new(Stage::Config, &e))?;
    Ok(LintSettings {
        rules,
        depth: config.depth(),
    })
}

/// The rules that run on `file`: the provider's and the shared ones, as configured.
fn active_rules(db: &dyn Db, file: File) -> Result<(Vec<ActiveRule<'static>>, Depth), Failure> {
    let provider = provider_of(db, file)?;
    let settings = lint_settings(db, file).as_ref().map_err(Clone::clone)?;
    let depth = settings.depth;
    let caps = provider.capabilities();
    let rules = provider
        .rules()
        .iter()
        .chain(core_rules::shared::RULES)
        .copied();
    let active = activate(rules, &settings.rules, |l| has_layer(depth, caps, l))
        .map_err(|e| Failure::new(Stage::Config, &e))?;
    Ok((active, depth))
}

/// Runs the rules on a file. A file with syntax errors reports those instead.
#[salsa::tracked(returns(ref))]
pub fn lint(db: &dyn Db, file: File) -> Result<Linted, Failure> {
    let provider = provider_of(db, file)?;
    let parsed = parse(db, file).as_ref().map_err(Clone::clone)?;
    if !parsed.errors.is_empty() {
        return Ok(Linted {
            diagnostics: parsed.errors.iter().map(syntax_error).collect(),
            facts: ProjectFacts::default(),
            parses: false,
        });
    }
    let (active, depth) = active_rules(db, file)?;
    let text = file.text(db);
    let root = parsed.root();
    let lines = LineIndex::new(text);
    let context = FileContext {
        path: file.path(db),
        text,
        root: &root,
        grammar: provider.grammar(),
        lines: &lines,
    };
    let FileReport { diagnostics, facts } = run_file(
        &context,
        &active,
        provider.body_kinds(),
        depth == Depth::Deep,
    );
    Ok(Linted {
        diagnostics,
        facts,
        parses: true,
    })
}

/// The project phase of the rules (`analysis: deep`): the facts of all `files` go to the rules of
/// their language. The options are those of the first file of each language, since a project
/// diagnostic belongs to no single file.
pub fn project_diagnostics(
    db: &dyn Db,
    files: &[File],
) -> Result<Vec<(PathBuf, Diagnostic)>, Failure> {
    let mut out = Vec::new();
    let mut done: Vec<usize> = Vec::new();
    for &file in files {
        let Some(sel) = selection(db, file).as_ref() else {
            continue;
        };
        if done.contains(&sel.provider) {
            continue;
        }
        done.push(sel.provider);
        let (active, depth) = active_rules(db, file)?;
        if depth != Depth::Deep {
            return Ok(out);
        }
        let mut facts = ProjectFacts::default();
        for &other in files {
            if selection(db, other).as_ref().map(|s| s.provider) == Some(sel.provider)
                && let Ok(linted) = lint(db, other)
            {
                facts.extend(&linted.facts);
            }
        }
        out.extend(run_project(&active, &facts));
    }
    Ok(out)
}

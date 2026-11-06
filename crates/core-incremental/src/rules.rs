//! Rule results as queries: `lint(file)` reads `parse` and the
//! resolved rule configuration, so an edit of a file or of the config re-runs only what depends on it.

use crate::db::Db;
use crate::failure::{Failure, Stage};
use crate::inputs::{ExtensionSet, File};
use crate::queries::{config, parse, parse_settings, provider_of, selection, source_file};
use core_config::{Depth, FileInput, ResolvedRule};
use core_format::DirectiveTable;
use core_rules::engine::{ActiveRule, FileReport};
use core_rules::{
    Category, Diagnostic, FileContext, Layer, LineIndex, ProjectFacts, SYNTAX_ERROR, Severity,
    activate, layers_needed, run_file, run_layout, run_project,
};
use core_semantics::{BehaviorSummaries, FileSemantics, Layers, SemanticView};
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
    /// Why the semantic model could not answer, when the file's rules needed it, or why a
    /// built-in pattern rule could not compile: those rules had nothing to look at.
    pub semantics_unavailable: Option<String>,
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
    /// The user's `semantics:` facts; the provider's built-in ones are added at lint time.
    pub summaries: BehaviorSummaries,
}

#[salsa::tracked(returns(ref))]
pub fn lint_settings(db: &dyn Db, file: File) -> Result<LintSettings, Failure> {
    let provider = provider_of(db, file)?;
    let config = config(db).as_ref().map_err(Clone::clone)?;
    let input = FileInput::language(provider.language()).with_path(file.path(db));
    let rules = config
        .rules(&input, provider.option_schema())
        .map_err(|e| Failure::new(Stage::Config, &e))?;
    let semantics = config.semantics(&input);
    Ok(LintSettings {
        rules,
        depth: config.depth(),
        summaries: BehaviorSummaries::new(&semantics.does_not_return, &semantics.pure),
    })
}

/// The rules that run on `file`: the provider's, the shared ones and the project's pattern and
/// plugin rules for its language, as configured.
fn active_rules(db: &dyn Db, file: File) -> Result<(Vec<ActiveRule<'static>>, Depth), Failure> {
    let provider = provider_of(db, file)?;
    let settings = lint_settings(db, file).as_ref().map_err(Clone::clone)?;
    let depth = settings.depth;
    let caps = provider.capabilities();
    let extensions = ExtensionSet::get(db).extensions(db);
    let rules = extensions.with_own(provider.rules(), provider.language());
    let mut active = activate(rules, &settings.rules, |l| has_layer(depth, caps, l))
        .map_err(|e| Failure::new(Stage::Config, &e))?;
    // A layout-only rule has nothing to report; `layout` runs it.
    active.retain(|a| a.rule.layout_role().reports());
    Ok((active, depth))
}

/// The layout directives the file's layout rules attach (see `core_rules::layout`), for
/// `format`. Syntax-only: a rule that needs semantic layers does not run here. With no layout
/// rule configured for the language this reads nothing but the extension set, so formatting
/// pays nothing for the feature. Conflicting directives and failing plugins are errors.
#[salsa::tracked(returns(ref))]
pub fn layout(db: &dyn Db, file: File) -> Result<DirectiveTable, Failure> {
    let provider = provider_of(db, file)?;
    let extensions = ExtensionSet::get(db).extensions(db);
    let language = provider.language();
    let layout_rules = || {
        extensions
            .with_own(provider.rust_rules(), language)
            .filter(|r| r.layout_role().attaches())
    };
    if layout_rules().next().is_none() {
        return Ok(DirectiveTable::default());
    }
    let parsed = parse(db, file).as_ref().map_err(Clone::clone)?;
    if !parsed.errors.is_empty() {
        return Ok(DirectiveTable::default());
    }
    let settings = lint_settings(db, file).as_ref().map_err(Clone::clone)?;
    let active = activate(layout_rules(), &settings.rules, |_| false)
        .map_err(|e| Failure::new(Stage::Config, &e))?;
    let text = file.text(db);
    let root = parsed.root();
    let lines = LineIndex::new(text);
    let context = FileContext {
        path: file.path(db),
        text,
        root: &root,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: provider.body_kinds(),
        hooks: provider.rule_hooks(),
        semantics: None,
    };
    run_layout(&context, &active).map_err(|e| Failure::message(Stage::Format, e))
}

/// The layers a file's rules ask the provider for, as far as the analysis depth and the
/// provider's capabilities allow.
fn wanted_layers(db: &dyn Db, file: File) -> Result<Layers, Failure> {
    let provider = provider_of(db, file)?;
    let (active, depth) = active_rules(db, file)?;
    let (needed, caps) = (layers_needed(&active), provider.capabilities());
    let allowed = |layer| has_layer(depth, caps, layer);
    Ok(Layers {
        binding: needed.binding && allowed(Layer::Binding),
        types: needed.types && allowed(Layer::Types),
        values: needed.values && allowed(Layer::Values),
        flow: needed.flow && allowed(Layer::Flow),
        project: needed.project && allowed(Layer::Project),
    })
}

/// What the provider's semantic model says about the file, for the layers its rules ask for
/// (`None` when they ask for none: `fast` analysis never reaches the worker's model). Depends on
/// the text and its version, which the worker's workspace is keyed on, like `parse`.
#[salsa::tracked(returns(ref))]
pub fn semantics(db: &dyn Db, file: File) -> Result<Option<FileSemantics>, Failure> {
    let layers = wanted_layers(db, file)?;
    if layers.is_empty() {
        return Ok(None);
    }
    let provider = provider_of(db, file)?;
    let source = source_file(db, file, parse_settings(db, file));
    provider
        .semantics(&source, layers)
        .map_err(|e| Failure::new(Stage::Semantics, &e))
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
            semantics_unavailable: None,
        });
    }
    let (active, depth) = active_rules(db, file)?;
    let (data, semantics_unavailable) = match semantics(db, file) {
        Ok(data) => (data.as_ref(), None),
        Err(failure) => (None, Some(failure.message.clone())),
    };
    let mut summaries = provider.summaries();
    summaries.merge(
        &lint_settings(db, file)
            .as_ref()
            .map_err(Clone::clone)?
            .summaries,
    );
    let text = file.text(db);
    let root = parsed.root();
    let lines = LineIndex::new(text);
    let context = FileContext {
        path: file.path(db),
        text,
        root: &root,
        grammar: provider.grammar(),
        lines: &lines,
        body_kinds: provider.body_kinds(),
        hooks: provider.rule_hooks(),
        semantics: data.map(|data| SemanticView {
            data,
            summaries: &summaries,
        }),
    };
    let FileReport { diagnostics, facts } = run_file(&context, &active, depth == Depth::Deep);
    let skipped = active.iter().find_map(|a| a.rule.unavailable());
    Ok(Linted {
        diagnostics,
        facts,
        parses: true,
        semantics_unavailable: semantics_unavailable.or(skipped),
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

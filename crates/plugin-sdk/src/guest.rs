//! The plugin side of the boundary: turns the host's call into a run of the core's own rule
//! engine over the file, so a rule behaves in a plugin exactly as it does built in.

use crate::PluginRule;
use crate::bindings::exports::sudsy::rules::rules as w;
use core_config::OptionValue;
use core_rules::engine::{ActiveRule, run_project, run_rules};
use core_rules::{Diagnostic, Fact, FileContext, LineIndex, ProjectFacts, RuleOptions, Severity};
use core_rules::{LayoutRole, attached};
use core_semantics::{BehaviorSummaries, FileSemantics, Layer, SemanticView};
use core_syntax::wire::{WIRE_VERSION, WireTree};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange, TextSize, grammars};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The interface version this SDK was built against.
pub const API: &str = "rules@0.2";

fn option_value(v: &core_rules::Opt) -> w::OptionValue {
    match *v {
        core_rules::Opt::Bool(b) => w::OptionValue::Flag(b),
        core_rules::Opt::Int(i) => w::OptionValue::Integer(i),
        core_rules::Opt::Str(s) => w::OptionValue::Text(s.into()),
        core_rules::Opt::List(l) => w::OptionValue::Texts(l.iter().map(|s| (*s).into()).collect()),
    }
}

fn layer_name(l: Layer) -> String {
    match l {
        Layer::Binding => "binding",
        Layer::Types => "types",
        Layer::Values => "values",
        Layer::Flow => "flow",
        Layer::Project => "project",
    }
    .into()
}

/// What the host asks first: the plugin's name and its rules.
pub fn info(name: &str, rules: &[PluginRule]) -> w::PluginInfo {
    w::PluginInfo {
        name: name.into(),
        api: API.into(),
        rules: rules
            .iter()
            .map(|r| {
                let m = r.rule.meta();
                w::RuleInfo {
                    id: m.id.into(),
                    language: r.language.into(),
                    category: m.category.as_str().into(),
                    description: m.description.into(),
                    example: m.example.into(),
                    default_level: m.default_level.as_str().into(),
                    fix: m.fix.map(|k| k.as_str().into()),
                    requires: m.requires.iter().map(|&l| layer_name(l)).collect(),
                    uses: m.uses.iter().map(|&l| layer_name(l)).collect(),
                    layout: match r.rule.layout_role() {
                        LayoutRole::None => w::LayoutRole::None,
                        LayoutRole::Also => w::LayoutRole::Also,
                        LayoutRole::Only => w::LayoutRole::Only,
                    },
                    options: m
                        .options
                        .iter()
                        .map(|o| w::OptionDecl {
                            name: o.name.into(),
                            description: o.description.into(),
                            default_value: option_value(&o.default),
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

fn grammar_of(language: &str) -> Result<&'static Grammar, String> {
    grammars::by_name(language).ok_or_else(|| format!("the SDK has no grammar for `{language}`"))
}

fn find<'a>(rules: &'a [PluginRule], id: &str) -> Result<&'a PluginRule, String> {
    rules
        .iter()
        .find(|r| r.rule.meta().id == id)
        .ok_or_else(|| format!("the plugin has no rule `{id}`"))
}

fn options_of(
    rule: &PluginRule,
    options: Vec<(String, w::OptionValue)>,
) -> Result<RuleOptions, String> {
    let configured: BTreeMap<String, OptionValue> = options
        .into_iter()
        .map(|(name, v)| {
            let v = match v {
                w::OptionValue::Flag(b) => OptionValue::Bool(b),
                w::OptionValue::Integer(i) => OptionValue::Int(i),
                w::OptionValue::Text(s) => OptionValue::String(s),
                w::OptionValue::Texts(l) => {
                    OptionValue::List(l.into_iter().map(OptionValue::String).collect())
                }
            };
            (name, v)
        })
        .collect();
    RuleOptions::resolve(rule.rule.meta().options, &configured)
}

/// The file as the host sent it, decoded.
struct Decoded {
    grammar: &'static Grammar,
    root: SyntaxNode,
    lines: LineIndex,
    body_kinds: Vec<SyntaxKind>,
    semantics: Option<(FileSemantics, BehaviorSummaries)>,
}

fn decode(file: &w::File) -> Result<Decoded, String> {
    let grammar = grammar_of(&file.language)?;
    if grammar.hash() != file.grammar_hash {
        return Err(format!(
            "the plugin was built against another {} grammar than the host's; rebuild it \
             against the same Sudsy version",
            file.language
        ));
    }
    let green = WireTree {
        version: WIRE_VERSION,
        kinds: file.tree_kinds.clone(),
        values: file.tree_values.clone(),
    }
    .decode(&file.text)
    .map_err(|e| e.to_string())?;
    let semantics = file
        .semantics
        .as_deref()
        .map(rmp_serde::from_slice)
        .transpose()
        .map_err(|e| format!("semantic data: {e}"))?;
    Ok(Decoded {
        grammar,
        root: SyntaxNode::new_root(green),
        lines: LineIndex::new(&file.text),
        body_kinds: file.body_kinds.iter().map(|&k| SyntaxKind(k)).collect(),
        semantics,
    })
}

/// Runs `f` on the decoded file with `rule` as the only active rule. Suppression comments are
/// the host's business: it applies them to what the plugin reports.
fn with_file<T>(
    rules: &[PluginRule],
    id: &str,
    options: Vec<(String, w::OptionValue)>,
    file: &w::File,
    f: impl FnOnce(&FileContext<'_>, &[ActiveRule<'_>]) -> T,
) -> Result<T, String> {
    let rule = find(rules, id)?;
    let decoded = decode(file)?;
    let context = FileContext {
        path: Path::new(&file.path),
        text: &file.text,
        root: &decoded.root,
        grammar: decoded.grammar,
        lines: &decoded.lines,
        body_kinds: &decoded.body_kinds,
        hooks: &core_rules::DEFAULT_HOOKS,
        embedded: &[],
        semantics: decoded
            .semantics
            .as_ref()
            .map(|(data, summaries)| SemanticView { data, summaries }),
    };
    let active = [ActiveRule {
        rule: rule.rule,
        // The host decides the severity.
        severity: Severity::Warning,
        options: options_of(rule, options)?,
    }];
    Ok(f(&context, &active))
}

fn span(r: TextRange) -> w::Span {
    w::Span {
        start: r.start().into(),
        end: r.end().into(),
    }
}

fn diagnostic(d: &Diagnostic) -> w::Diagnostic {
    w::Diagnostic {
        span: span(d.range),
        message: d.message.clone(),
        related: d
            .related
            .iter()
            .map(|r| w::Related {
                span: span(r.range),
                message: r.message.clone(),
            })
            .collect(),
        fixes: d
            .fixes
            .iter()
            .map(|f| w::Fix {
                title: f.title.clone(),
                kind: f.kind.as_str().into(),
                edits: f
                    .edits
                    .iter()
                    .map(|e| w::Edit {
                        span: span(e.range),
                        text: e.text.clone(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub fn check(
    rules: &[PluginRule],
    id: &str,
    options: Vec<(String, w::OptionValue)>,
    file: w::File,
) -> Result<Vec<w::Diagnostic>, String> {
    with_file(rules, id, options, &file, |context, active| {
        run_rules(context, active, false)
            .diagnostics
            .iter()
            .map(diagnostic)
            .collect()
    })
}

pub fn layout(
    rules: &[PluginRule],
    id: &str,
    options: Vec<(String, w::OptionValue)>,
    file: w::File,
) -> Result<Vec<w::LayoutRequest>, String> {
    with_file(rules, id, options, &file, |context, active| {
        Ok(attached(context, active)?
            .into_iter()
            .map(|a| {
                let (start, end) = a.key.range();
                w::LayoutRequest {
                    kind: a.key.kind().0,
                    start,
                    end,
                    at: a.at.into(),
                    directive: a.directive.to_string(),
                }
            })
            .collect())
    })?
}

pub fn collect(
    rules: &[PluginRule],
    id: &str,
    options: Vec<(String, w::OptionValue)>,
    file: w::File,
) -> Result<Vec<w::Fact>, String> {
    with_file(rules, id, options, &file, |context, active| {
        run_rules(context, active, true)
            .facts
            .facts()
            .iter()
            .map(|f| w::Fact {
                file: f.file.to_string_lossy().into_owned(),
                span: span(f.range),
                key: f.key.clone(),
                value: f.value.clone(),
            })
            .collect()
    })
}

pub fn finish(
    rules: &[PluginRule],
    id: &str,
    options: Vec<(String, w::OptionValue)>,
    facts: Vec<w::Fact>,
) -> Result<Vec<w::ProjectDiagnostic>, String> {
    let rule = find(rules, id)?;
    let meta = rule.rule.meta();
    let facts = ProjectFacts::from_facts(
        facts
            .into_iter()
            .map(|f| Fact {
                rule: meta.id,
                file: PathBuf::from(f.file),
                range: TextRange::new(TextSize::from(f.span.start), TextSize::from(f.span.end)),
                key: f.key,
                value: f.value,
            })
            .collect(),
    );
    let active = [ActiveRule {
        rule: rule.rule,
        severity: Severity::Warning,
        options: options_of(rule, options)?,
    }];
    Ok(run_project(&active, &facts)
        .into_iter()
        .map(|(file, d)| w::ProjectDiagnostic {
            file: file.to_string_lossy().into_owned(),
            diagnostic: diagnostic(&d),
        })
        .collect())
}

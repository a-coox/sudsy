//! Pattern rule files: ast-grep-style YAML, one or more rules per file (documents separated by
//! `---`).
//!
//! ```yaml
//! id: no-console-writeline
//! language: csharp
//! message: Use the logger instead of Console.WriteLine($$$ARGS).
//! severity: warn                  # off | warn | error; the level when the config is silent
//! category: suspicious            # correctness | suspicious | style | cleanup | performance
//! rule:
//!   pattern: Console.WriteLine($$$ARGS)
//! constraints:                    # optional checks on what a metavariable captured
//!   ARGS: { regex: "^\"" }
//! fix: _logger.LogInformation($$$ARGS)
//! fix-kind: unsafe                # safe | unsafe | suggestion; unsafe when omitted
//! layout:                         # layout directives for the formatter (message becomes optional)
//!   - break-all on $CHAIN         # `<directive> [argument] [on $METAVARIABLE]`
//! ```

use crate::expr::{Ctx, Expr, Spec};
use crate::matcher::{Captures, SnippetParser};
use crate::metavar::rewrite;
use core_config::Level;
use core_format::Directive;
use core_rules::tree::significant_range;
use core_rules::{Category, Edit, Fix, FixKind, LayoutRole, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode};
use regex::Regex;
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct PatternError(pub String);

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum CategorySpec {
    Correctness,
    Suspicious,
    Style,
    Cleanup,
    Performance,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum FixKindSpec {
    Safe,
    Unsafe,
    Suggestion,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Constraint {
    regex: Option<String>,
    #[serde(rename = "not-regex")]
    not_regex: Option<String>,
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSpec {
    id: String,
    language: String,
    message: Option<String>,
    severity: Option<Level>,
    category: Option<CategorySpec>,
    description: Option<String>,
    example: Option<String>,
    rule: Spec,
    #[serde(default)]
    constraints: BTreeMap<String, Constraint>,
    fix: Option<String>,
    #[serde(rename = "fix-title")]
    fix_title: Option<String>,
    #[serde(rename = "fix-kind")]
    fix_kind: Option<FixKindSpec>,
    #[serde(default)]
    layout: Vec<String>,
}

struct CompiledConstraint {
    regex: Option<Regex>,
    not_regex: Option<Regex>,
    kind: Option<SyntaxKind>,
}

/// One `layout:` entry: a directive for the matched node, or for the node a metavariable
/// captured.
struct LayoutEntry {
    directive: Directive,
    target: Option<String>,
}

fn layout_entry(text: &str) -> Result<LayoutEntry, String> {
    let (directive, target) = match text.rsplit_once(" on ") {
        Some((d, t)) => {
            let name = t.trim().strip_prefix('$').filter(|n| !n.is_empty());
            let name = name.ok_or_else(|| {
                format!("layout `{text}`: `on` takes a metavariable like `$ITEMS`")
            })?;
            (d, Some(name.to_string()))
        }
        None => (text, None),
    };
    let directive = Directive::parse(directive).map_err(|e| format!("layout `{text}`: {e}"))?;
    Ok(LayoutEntry { directive, target })
}

struct FixTemplate {
    template: String,
    title: String,
    kind: FixKind,
}

/// A rule read from a pattern file. It is an ordinary [`Rule`]: the engine, the config and the
/// fix loop treat it like a built-in one.
pub struct PatternRule {
    meta: &'static RuleMeta,
    kinds: Vec<SyntaxKind>,
    expr: Expr,
    /// `None`: a layout-only rule.
    message: Option<String>,
    constraints: BTreeMap<String, CompiledConstraint>,
    fix: Option<FixTemplate>,
    layout: Vec<LayoutEntry>,
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn compile_constraint(
    name: &str,
    c: &Constraint,
    parser: &dyn SnippetParser,
) -> Result<CompiledConstraint, String> {
    let regex = |text: &Option<String>| {
        text.as_deref()
            .map(|t| Regex::new(t).map_err(|e| format!("constraint {name}: regex `{t}`: {e}")))
            .transpose()
    };
    let kind =
        c.kind
            .as_deref()
            .map(|k| {
                parser.grammar().kind_by_name(k).ok_or_else(|| {
                    format!("constraint {name}: `{k}` is not a kind of the language")
                })
            })
            .transpose()?;
    Ok(CompiledConstraint {
        regex: regex(&c.regex)?,
        not_regex: regex(&c.not_regex)?,
        kind,
    })
}

fn compile(spec: FileSpec, parser: &dyn SnippetParser) -> Result<PatternRule, String> {
    let expr = Expr::compile(&spec.rule, parser)?;
    let mut captured = Vec::new();
    expr.metas(&mut captured);
    let known = |name: &str| captured.iter().any(|m| m.name == name);
    if spec.message.is_none() && (spec.layout.is_empty() || spec.fix.is_some()) {
        return Err(
            "give the rule a `message` (a layout-only rule needs `layout:` and no `fix`)".into(),
        );
    }
    let layout = spec
        .layout
        .iter()
        .map(|t| layout_entry(t))
        .collect::<Result<Vec<_>, _>>()?;
    for name in layout.iter().filter_map(|e| e.target.as_deref()) {
        if !known(name) {
            return Err(format!(
                "layout targets `${name}`, which no pattern outside inside/has/not captures"
            ));
        }
    }
    let template_uses = [spec.message.as_ref(), spec.fix.as_ref()];
    for text in template_uses.into_iter().flatten() {
        for meta in crate::metavar::names(text) {
            if meta.captures() && !known(&meta.name) {
                return Err(format!(
                    "`${}` is used in the message or fix but no pattern outside inside/has/not captures it",
                    meta.name
                ));
            }
        }
    }
    let mut constraints = BTreeMap::new();
    for (name, c) in &spec.constraints {
        if !known(name) {
            return Err(format!(
                "constraint on `${name}`, which no pattern captures"
            ));
        }
        constraints.insert(name.clone(), compile_constraint(name, c, parser)?);
    }
    let mut kinds = expr.kinds().ok_or(
        "the rule must say where it starts: give it a `pattern` or a `kind` (at the top, or in          every branch of `any`), so that it runs on nodes of those kinds only",
    )?;
    kinds.sort();
    kinds.dedup();
    let fix = spec.fix.map(|template| FixTemplate {
        title: spec
            .fix_title
            .unwrap_or_else(|| "Apply the rule's fix".into()),
        kind: match spec.fix_kind.unwrap_or(FixKindSpec::Unsafe) {
            FixKindSpec::Safe => FixKind::Safe,
            FixKindSpec::Unsafe => FixKind::Unsafe,
            FixKindSpec::Suggestion => FixKind::Suggestion,
        },
        template,
    });
    let summary = spec
        .message
        .clone()
        .unwrap_or_else(|| format!("Layout: {}", spec.layout.join(", ")));
    let description = spec.description.unwrap_or_else(|| summary.clone());
    let example = spec.example.unwrap_or_else(|| {
        let pattern = spec
            .rule
            .pattern
            .as_deref()
            .unwrap_or("(see the rule file)");
        format!("{pattern}\n{summary}")
    });
    let meta = Box::leak(Box::new(RuleMeta {
        id: leak(spec.id),
        category: match spec.category.unwrap_or(CategorySpec::Style) {
            CategorySpec::Correctness => Category::Correctness,
            CategorySpec::Suspicious => Category::Suspicious,
            CategorySpec::Style => Category::Style,
            CategorySpec::Cleanup => Category::Cleanup,
            CategorySpec::Performance => Category::Performance,
        },
        description: leak(description),
        example: leak(example),
        default_level: spec.severity.unwrap_or(Level::Warn),
        fix: fix.as_ref().map(|f| f.kind),
        requires: &[],
        uses: &[],
        options: &[],
    }));
    Ok(PatternRule {
        meta,
        kinds,
        expr,
        message: spec.message,
        constraints,
        fix,
        layout,
    })
}

/// Reads the rules of one file. `parser_of` gives the snippet parser of a language, `None` for
/// a language Sudsy does not have. Returns `(language, rule)` in file order.
pub fn load<'p>(
    source: &str,
    parser_of: &dyn Fn(&str) -> Option<&'p dyn SnippetParser>,
) -> Result<Vec<(String, PatternRule)>, PatternError> {
    let mut options = serde_saphyr::Options::default();
    options.strict_booleans = true;
    let specs: Vec<FileSpec> =
        serde_saphyr::from_multiple_with_options(source, options).map_err(|e| {
            PatternError(match e.location() {
                Some(l) => format!("{}:{}: {e}", l.line(), l.column()),
                None => e.to_string(),
            })
        })?;
    if specs.is_empty() {
        return Err(PatternError("the file holds no rule".into()));
    }
    specs
        .into_iter()
        .map(|spec| {
            let (id, language) = (spec.id.clone(), spec.language.clone());
            let parser = parser_of(&language).ok_or_else(|| {
                PatternError(format!("rule `{id}`: unknown language `{language}`"))
            })?;
            let rule =
                compile(spec, parser).map_err(|e| PatternError(format!("rule `{id}`: {e}")))?;
            Ok((language, rule))
        })
        .collect()
}

fn substitute(template: &str, captures: &Captures, text: &str) -> String {
    rewrite(template, |meta| {
        captures
            .get(&meta.name)
            .map_or("", |c| c.text(text))
            .to_string()
    })
}

impl PatternRule {
    /// Attaches the rule's directives: to the matched node, or to the single node a metavariable
    /// captured (a metavariable that captured a token or a run gets nothing).
    fn attach_layout(&self, node: &SyntaxNode, captures: &Captures, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        for entry in &self.layout {
            let target = match &entry.target {
                None => Some(node.clone()),
                Some(name) => captures.get(name).and_then(|c| {
                    let kind = c.kind?;
                    node.descendants()
                        .find(|n| n.kind() == kind && significant_range(n, grammar) == c.range)
                }),
            };
            if let Some(target) = target {
                ctx.attach(&target, entry.directive.clone());
            }
        }
    }

    fn constraints_hold(&self, captures: &Captures, text: &str) -> bool {
        self.constraints.iter().all(|(name, c)| {
            let Some(capture) = captures.get(name) else {
                return true;
            };
            let captured = capture.text(text);
            c.regex.as_ref().is_none_or(|re| re.is_match(captured))
                && c.not_regex.as_ref().is_none_or(|re| !re.is_match(captured))
                && c.kind.is_none_or(|k| capture.kind == Some(k))
        })
    }
}

impl Rule for PatternRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &self.kinds
    }

    fn layout_role(&self) -> LayoutRole {
        match (&self.message, self.layout.is_empty()) {
            (None, _) => LayoutRole::Only,
            (Some(_), false) => LayoutRole::Also,
            (Some(_), true) => LayoutRole::None,
        }
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let file = ctx.file;
        let cx = Ctx {
            grammar: file.grammar,
            text: file.text,
        };
        let Some(captures) = self.expr.matches(node, &cx) else {
            return;
        };
        if !self.constraints_hold(&captures, file.text) {
            return;
        }
        let range = significant_range(node, file.grammar);
        if ctx.laying_out() {
            self.attach_layout(node, &captures, ctx);
        } else if let Some(message) = &self.message {
            let diagnostic = ctx.report(range, substitute(message, &captures, file.text));
            if let Some(fix) = &self.fix {
                let replacement = substitute(&fix.template, &captures, file.text);
                diagnostic.fix(Fix::new(
                    fix.title.clone(),
                    fix.kind,
                    vec![Edit::replace(range, replacement)],
                ));
            }
        }
    }
}

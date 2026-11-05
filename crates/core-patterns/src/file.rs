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
//! report-on: first-token $X       # the diagnostic span: `$X`, `first-token` or `first-token $X`
//! fix-on: $X                      # the range the fix replaces (default: the matched node)
//! fix-trim-before: true           # the fix also takes the spaces and tabs before its range
//! options:                        # typed options, validated like a Rust rule's
//!   - { name: allowed, type: list, default: [], description: Names that are fine }
//! ```

use crate::expr::{Ctx, Expr, STEP_BUDGET, Spec};
use crate::matcher::{Captures, SnippetParser};
use crate::metavar::rewrite;
use core_config::Level;
use core_format::Directive;
use core_rules::tree::significant_range;
use core_rules::{
    Category, Check, Edit, Family, Fix, FixKind, Layer, LayoutRole, Opt, OptionDecl, Params, Rule,
    RuleContext, RuleMeta, RuleOptions,
};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode, TextRange};
use regex::Regex;
use serde::Deserialize;
use std::cell::Cell;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, OnceLock};

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

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum LayerSpec {
    Binding,
    Types,
    Values,
    Flow,
    Project,
}

fn layers(specs: &[LayerSpec]) -> &'static [Layer] {
    let layers: Vec<Layer> = specs
        .iter()
        .map(|l| match l {
            LayerSpec::Binding => Layer::Binding,
            LayerSpec::Types => Layer::Types,
            LayerSpec::Values => Layer::Values,
            LayerSpec::Flow => Layer::Flow,
            LayerSpec::Project => Layer::Project,
        })
        .collect();
    Box::leak(layers.into_boxed_slice())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Constraint {
    regex: Option<String>,
    #[serde(rename = "not-regex")]
    not_regex: Option<String>,
    kind: Option<String>,
    /// The captured text is an item of this list option (or equals this string option).
    #[serde(rename = "in-option")]
    in_option: Option<String>,
    /// The captured text matches the regex held by this `regex` option.
    #[serde(rename = "regex-option")]
    regex_option: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OptionType {
    Bool,
    Int,
    String,
    List,
    Regex,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DefaultSpec {
    Bool(bool),
    Int(i64),
    Str(String),
    List(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OptionSpec {
    name: String,
    #[serde(rename = "type")]
    ty: OptionType,
    default: DefaultSpec,
    /// The allowed values of a `string` option, or of each item of a `list` option.
    choices: Option<Vec<String>>,
    description: String,
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
    rule: Option<Spec>,
    /// Built-in rules only: the Rust family that implements the rule, with its `params`, in
    /// place of `rule`.
    family: Option<String>,
    #[serde(default)]
    params: BTreeMap<String, String>,
    #[serde(default)]
    requires: Vec<LayerSpec>,
    #[serde(default)]
    uses: Vec<LayerSpec>,
    #[serde(default)]
    constraints: BTreeMap<String, Constraint>,
    fix: Option<String>,
    #[serde(rename = "fix-title")]
    fix_title: Option<String>,
    #[serde(rename = "fix-kind")]
    fix_kind: Option<FixKindSpec>,
    #[serde(default)]
    layout: Vec<String>,
    #[serde(rename = "report-on")]
    report_on: Option<String>,
    #[serde(rename = "fix-on")]
    fix_on: Option<String>,
    #[serde(rename = "fix-trim-before", default)]
    fix_trim_before: bool,
    #[serde(default)]
    options: Vec<OptionSpec>,
}

struct CompiledConstraint {
    regex: Option<Regex>,
    not_regex: Option<Regex>,
    kind: Option<SyntaxKind>,
    /// The list option (true) or string option (false) the capture must be in or equal.
    in_option: Option<(String, bool)>,
    regex_option: Option<String>,
}

/// Where a diagnostic points: the matched node or what a metavariable captured, or the first
/// token of either.
struct ReportOn {
    first_token: bool,
    target: Option<String>,
}

fn report_on(text: &str) -> Result<ReportOn, String> {
    let text = text.trim();
    let (first_token, rest) = match text.strip_prefix("first-token") {
        Some(rest) => (true, rest.trim()),
        None => (false, text),
    };
    let target = match rest.strip_prefix('$') {
        _ if rest.is_empty() => None,
        Some(name) if !name.is_empty() => Some(name.to_string()),
        _ => {
            return Err(format!(
                "report-on `{text}`: write `$NAME`, `first-token` or `first-token $NAME`"
            ));
        }
    };
    Ok(ReportOn {
        first_token,
        target,
    })
}

fn declared_options(specs: &[OptionSpec]) -> Result<&'static [OptionDecl], String> {
    let mut out: Vec<OptionDecl> = Vec::new();
    for o in specs {
        if out.iter().any(|d| d.name == o.name) {
            return Err(format!("option `{}` is declared twice", o.name));
        }
        let list = |items: &[String]| -> &'static [&'static str] {
            let items: Vec<&'static str> = items.iter().map(|s| leak(s.clone())).collect();
            Box::leak(items.into_boxed_slice())
        };
        let default = match (o.ty, &o.default) {
            (OptionType::Bool, DefaultSpec::Bool(b)) => Opt::Bool(*b),
            (OptionType::Int, DefaultSpec::Int(i)) => Opt::Int(*i),
            (OptionType::String | OptionType::Regex, DefaultSpec::Str(s)) => {
                Opt::Str(leak(s.clone()))
            }
            (OptionType::List, DefaultSpec::List(l)) => Opt::List(list(l)),
            _ => {
                return Err(format!(
                    "option `{}`: the default does not fit its type",
                    o.name
                ));
            }
        };
        let check = match (o.ty, &o.choices) {
            (OptionType::Regex, None) => Check::Regex,
            (OptionType::String | OptionType::List, Some(c)) => Check::OneOf(list(c)),
            (OptionType::String | OptionType::List, None) => Check::Any,
            _ => {
                return Err(format!(
                    "option `{}`: `choices` go with `string` and `list` only, and a `regex` has none",
                    o.name
                ));
            }
        };
        out.push(OptionDecl {
            name: leak(o.name.clone()),
            default,
            check,
            description: leak(o.description.clone()),
        });
    }
    Ok(Box::leak(out.into_boxed_slice()))
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
    /// The metavariable whose capture the fix replaces; the matched node without it.
    on: Option<String>,
    trim_before: bool,
}

/// What compiling the patterns of a rule gives; needs the language's parser.
struct Body {
    kinds: Vec<SyntaxKind>,
    expr: Expr,
    /// Can matching cost more than linear (see [`STEP_BUDGET`])? Only then is it metered.
    metered: bool,
    constraints: BTreeMap<String, CompiledConstraint>,
    fix: Option<FixTemplate>,
    layout: Vec<LayoutEntry>,
    report_on: Option<ReportOn>,
}

/// A rule whose patterns are not compiled yet.
struct Pending {
    spec: FileSpec,
    parser: Arc<dyn SnippetParser + Send + Sync>,
}

/// A rule read from a pattern file. It is an ordinary [`Rule`]: the engine, the config and the
/// fix loop treat it like a built-in one.
pub struct PatternRule {
    meta: &'static RuleMeta,
    /// `None`: a layout-only rule.
    message: Option<String>,
    has_layout: bool,
    /// The compiled patterns: at once for a rule of a project's pattern file, on first use for
    /// a built-in rule (compiling parses snippets, which for C# starts the worker). A failed
    /// compile is not kept: the next file tries again, since the worker may be back.
    body: OnceLock<Body>,
    /// Why the last attempt to compile failed.
    failure: Mutex<Option<String>>,
    pending: Option<Pending>,
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}

fn compile_constraint(
    name: &str,
    c: &Constraint,
    parser: &dyn SnippetParser,
    options: &[OptionDecl],
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
    let option = |option: &Option<String>, what: &str| {
        option
            .as_deref()
            .map(|n| {
                options
                    .iter()
                    .find(|d| d.name == n)
                    .ok_or_else(|| format!("constraint {name}: {what} `{n}` is not declared"))
            })
            .transpose()
    };
    let in_option = option(&c.in_option, "in-option")?
        .map(|d| match d.default {
            Opt::List(_) => Ok((d.name.to_string(), true)),
            Opt::Str(_) if d.check != Check::Regex => Ok((d.name.to_string(), false)),
            _ => Err(format!(
                "constraint {name}: in-option `{}` must be a `list` or `string` option",
                d.name
            )),
        })
        .transpose()?;
    let regex_option = option(&c.regex_option, "regex-option")?
        .map(|d| {
            if d.check == Check::Regex {
                Ok(d.name.to_string())
            } else {
                Err(format!(
                    "constraint {name}: regex-option `{}` must be a `regex` option",
                    d.name
                ))
            }
        })
        .transpose()?;
    Ok(CompiledConstraint {
        regex: regex(&c.regex)?,
        not_regex: regex(&c.not_regex)?,
        kind,
        in_option,
        regex_option,
    })
}

fn compile_body(
    spec: &FileSpec,
    parser: &dyn SnippetParser,
    options: &[OptionDecl],
) -> Result<Body, String> {
    let rule = spec.rule.as_ref().ok_or("the rule has no `rule:`")?;
    let expr = Expr::compile(rule, parser)?;
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
    let report_on = spec.report_on.as_deref().map(report_on).transpose()?;
    if report_on.is_some() && spec.message.is_none() {
        return Err("`report-on` needs a `message`".into());
    }
    if (spec.fix_on.is_some() || spec.fix_trim_before) && spec.fix.is_none() {
        return Err("`fix-on` and `fix-trim-before` need a `fix`".into());
    }
    let fix_on = spec
        .fix_on
        .as_deref()
        .map(|t| {
            t.trim()
                .strip_prefix('$')
                .filter(|n| !n.is_empty())
                .map(str::to_string)
                .ok_or_else(|| format!("fix-on `{t}`: write a metavariable like `$NAME`"))
        })
        .transpose()?;
    let targets = report_on
        .iter()
        .filter_map(|r| r.target.as_deref())
        .chain(fix_on.as_deref());
    for name in targets {
        if !known(name) {
            return Err(format!(
                "`${name}` is used by report-on or fix-on but no pattern outside inside/has/not captures it"
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
        constraints.insert(name.clone(), compile_constraint(name, c, parser, options)?);
    }
    let mut kinds = expr.kinds().ok_or(
        "the rule must say where it starts: give it a `pattern` or a `kind` (at the top, or in every branch of `any`), so that it runs on nodes of those kinds only",
    )?;
    kinds.sort();
    kinds.dedup();
    let fix = spec.fix.as_ref().map(|template| FixTemplate {
        title: spec
            .fix_title
            .clone()
            .unwrap_or_else(|| "Apply the rule's fix".into()),
        kind: fix_kind(spec.fix_kind),
        template: template.clone(),
        on: fix_on,
        trim_before: spec.fix_trim_before,
    });
    Ok(Body {
        kinds,
        metered: expr.superlinear(),
        expr,
        constraints,
        fix,
        layout,
        report_on,
    })
}

fn fix_kind(spec: Option<FixKindSpec>) -> FixKind {
    match spec.unwrap_or(FixKindSpec::Unsafe) {
        FixKindSpec::Safe => FixKind::Safe,
        FixKindSpec::Unsafe => FixKind::Unsafe,
        FixKindSpec::Suggestion => FixKind::Suggestion,
    }
}

/// Everything a rule shows without its patterns: the metadata, with the options declared.
fn meta_of(spec: &FileSpec) -> Result<&'static RuleMeta, String> {
    let summary = spec
        .message
        .clone()
        .unwrap_or_else(|| format!("Layout: {}", spec.layout.join(", ")));
    let description = spec.description.clone().unwrap_or_else(|| summary.clone());
    let example = spec.example.clone().unwrap_or_else(|| {
        let pattern = spec
            .rule
            .as_ref()
            .and_then(|r| r.pattern.as_deref())
            .unwrap_or("(see the rule file)");
        format!("{pattern}\n{summary}")
    });
    Ok(Box::leak(Box::new(RuleMeta {
        id: leak(spec.id.clone()),
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
        fix: if spec.family.is_some() {
            spec.fix_kind.map(|k| fix_kind(Some(k)))
        } else {
            spec.fix.as_ref().map(|_| fix_kind(spec.fix_kind))
        },
        requires: layers(&spec.requires),
        uses: layers(&spec.uses),
        options: declared_options(&spec.options)?,
    })))
}

fn read_specs(source: &str) -> Result<Vec<FileSpec>, PatternError> {
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
    for s in &specs {
        let pattern_keys = s.rule.is_some()
            || s.fix.is_some()
            || !s.layout.is_empty()
            || s.report_on.is_some()
            || !s.constraints.is_empty();
        let err = |m: &str| Err(PatternError(format!("rule `{}`: {m}", s.id)));
        match (&s.family, pattern_keys) {
            (Some(_), true) => {
                return err(
                    "`family` takes `params`, not `rule`, `fix`, `layout`, `constraints` or `report-on`",
                );
            }
            (None, _) if !s.params.is_empty() => return err("`params` go with `family`"),
            (None, _) if s.rule.is_none() => {
                return err("give the rule a `rule:` (or a `family:`)");
            }
            _ => {}
        }
    }
    Ok(specs)
}

/// Reads the rules of one file. `parser_of` gives the snippet parser of a language, `None` for
/// a language Sudsy does not have. Returns `(language, rule)` in file order.
pub fn load<'p>(
    source: &str,
    parser_of: &dyn Fn(&str) -> Option<&'p dyn SnippetParser>,
) -> Result<Vec<(String, PatternRule)>, PatternError> {
    read_specs(source)?
        .into_iter()
        .map(|spec| {
            let (id, language) = (spec.id.clone(), spec.language.clone());
            if spec.family.is_some() {
                return Err(PatternError(format!(
                    "rule `{id}`: `family` is for built-in rules"
                )));
            }
            let parser = parser_of(&language).ok_or_else(|| {
                PatternError(format!("rule `{id}`: unknown language `{language}`"))
            })?;
            let rule = compile_eager(spec, parser)
                .map_err(|e| PatternError(format!("rule `{id}`: {e}")))?;
            Ok((language, rule))
        })
        .collect()
}

fn compile_eager(spec: FileSpec, parser: &dyn SnippetParser) -> Result<PatternRule, String> {
    let meta = meta_of(&spec)?;
    let body = compile_body(&spec, parser, meta.options)?;
    Ok(PatternRule {
        meta,
        message: spec.message,
        has_layout: !spec.layout.is_empty(),
        body: OnceLock::from(body),
        failure: Mutex::default(),
        pending: None,
    })
}

/// Reads the rules of a built-in rule file, of `language`, without compiling their patterns:
/// the metadata (options included) is there at once, and the patterns compile when the rule
/// first runs, through `parser`. Compiling parses snippets with the language's provider, which
/// for C# starts the worker, so a run that never lints C# never pays for it. A rule with `family`
/// is built by that one of `families` instead (it has no patterns).
pub fn load_lazy(
    source: &str,
    language: &str,
    parser: &Arc<dyn SnippetParser + Send + Sync>,
    families: &[Family],
) -> Result<Vec<Box<dyn Rule>>, PatternError> {
    read_specs(source)?
        .into_iter()
        .map(|spec| {
            let id = spec.id.clone();
            let err = |e: String| PatternError(format!("rule `{id}`: {e}"));
            if spec.language != language {
                return Err(err(format!("language must be `{language}`")));
            }
            let meta = meta_of(&spec).map_err(err)?;
            if let Some(name) = &spec.family {
                let family = families.iter().find(|f| f.name == *name);
                let family = family.ok_or_else(|| err(format!("unknown family `{name}`")))?;
                let mut params = spec.params.clone();
                params.extend(spec.message.clone().map(|m| ("message".to_string(), m)));
                return (family.build)(meta, Params::new(params)).map_err(err);
            }
            Ok(Box::new(PatternRule {
                meta,
                message: spec.message.clone(),
                has_layout: !spec.layout.is_empty(),
                body: OnceLock::new(),
                failure: Mutex::default(),
                pending: Some(Pending {
                    spec,
                    parser: parser.clone(),
                }),
            }) as Box<dyn Rule>)
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

/// Reported once per file, after which the rule skips the rest of the file.
const OVER_BUDGET: &str = "the pattern took too many steps on this node (several `$$$` in one pattern?); \nthis rule is off for the rest of the file";

/// The range of the first token that is not trivia, within `within`.
fn first_token_range(node: &SyntaxNode, within: TextRange, grammar: &Grammar) -> TextRange {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .find(|t| {
            !grammar.is_trivia(t.kind())
                && !t.text().is_empty()
                && within.contains_range(t.text_range())
        })
        .map_or(within, |t| t.text_range())
}

impl PatternRule {
    /// Compiles the patterns now (a built-in rule does it on first use): the error a broken
    /// built-in file would panic with.
    pub fn compile_now(&self) -> Result<(), String> {
        self.compiled().map(drop)
    }

    fn compiled(&self) -> Result<&Body, String> {
        if let Some(body) = self.body.get() {
            return Ok(body);
        }
        let p = self
            .pending
            .as_ref()
            .expect("an uncompiled rule is pending");
        let compiled = compile_body(&p.spec, &*p.parser, self.meta.options);
        *self.failure.lock().unwrap() = compiled.as_ref().err().cloned();
        compiled.map(|body| self.body.get_or_init(|| body))
    }

    /// The compiled patterns; callers have checked [`PatternRule::compiled`] succeeded.
    fn body(&self) -> &Body {
        self.body.get().expect("the rule compiled before it ran")
    }

    /// The span of the diagnostic.
    fn report_range(
        &self,
        node: &SyntaxNode,
        captures: &Captures,
        range: TextRange,
        grammar: &Grammar,
    ) -> TextRange {
        let Some(on) = &self.body().report_on else {
            return range;
        };
        let target = on
            .target
            .as_ref()
            .and_then(|n| captures.get(n))
            .map_or(range, |c| c.range);
        if on.first_token {
            first_token_range(node, target, grammar)
        } else {
            target
        }
    }

    /// Attaches the rule's directives: to the matched node, or to the single node a metavariable
    /// captured (a metavariable that captured a token or a run gets nothing).
    fn attach_layout(&self, node: &SyntaxNode, captures: &Captures, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        for entry in &self.body().layout {
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

    fn constraints_hold(&self, captures: &Captures, text: &str, options: &RuleOptions) -> bool {
        self.body().constraints.iter().all(|(name, c)| {
            let Some(capture) = captures.get(name) else {
                return true;
            };
            let captured = capture.text(text);
            c.regex.as_ref().is_none_or(|re| re.is_match(captured))
                && c.not_regex.as_ref().is_none_or(|re| !re.is_match(captured))
                && c.kind.is_none_or(|k| capture.kind == Some(k))
                && c.in_option.as_ref().is_none_or(|(option, is_list)| {
                    if *is_list {
                        options.list(option).contains(&captured)
                    } else {
                        options.string(option) == captured
                    }
                })
                && c.regex_option.as_ref().is_none_or(|option| {
                    Regex::new(options.string(option)).is_ok_and(|re| re.is_match(captured))
                })
        })
    }

    /// The edit of the fix: the matched node, or what `fix-on` captured, with the spaces and
    /// tabs before it when asked.
    fn fix_edit(fix: &FixTemplate, captures: &Captures, range: TextRange, text: &str) -> Edit {
        let mut target = fix
            .on
            .as_ref()
            .and_then(|n| captures.get(n))
            .map_or(range, |c| c.range);
        if fix.trim_before {
            let before = &text[..usize::from(target.start())];
            let start = before.trim_end_matches([' ', '\t']).len();
            target = TextRange::new((start as u32).into(), target.end());
        }
        Edit::replace(target, substitute(&fix.template, captures, text))
    }
}

impl Rule for PatternRule {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    /// Empty when the patterns cannot compile now (the worker is down): the rule is skipped
    /// for the file and [`Rule::unavailable`] says why.
    fn kinds(&self) -> &[SyntaxKind] {
        self.compiled().map_or(&[], |b| &b.kinds)
    }

    fn unavailable(&self) -> Option<String> {
        let why = self.failure.lock().unwrap().clone()?;
        Some(format!(
            "pattern rule `{}` could not compile: {why}",
            self.meta.id
        ))
    }

    fn layout_role(&self) -> LayoutRole {
        match (&self.message, self.has_layout) {
            (None, _) => LayoutRole::Only,
            (Some(_), true) => LayoutRole::Also,
            (Some(_), false) => LayoutRole::None,
        }
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let file = ctx.file;
        let body = self.body();
        if body.metered && ctx.has_reported(OVER_BUDGET) {
            return; // the rule is off for this file
        }
        let cx = Ctx {
            grammar: file.grammar,
            text: file.text,
            budget: Cell::new(STEP_BUDGET),
        };
        let found = body.expr.matches(node, &cx);
        if body.metered && cx.budget.get() == 0 {
            if !ctx.laying_out() {
                ctx.report(significant_range(node, file.grammar), OVER_BUDGET);
            }
            return;
        }
        let Some(captures) = found else {
            return;
        };
        if !self.constraints_hold(&captures, file.text, ctx.options) {
            return;
        }
        let range = significant_range(node, file.grammar);
        if ctx.laying_out() {
            self.attach_layout(node, &captures, ctx);
        } else if let Some(message) = &self.message {
            let at = self.report_range(node, &captures, range, file.grammar);
            let diagnostic = ctx.report(at, substitute(message, &captures, file.text));
            if let Some(fix) = &body.fix {
                let edit = Self::fix_edit(fix, &captures, range, file.text);
                diagnostic.fix(Fix::new(fix.title.clone(), fix.kind, vec![edit]));
            }
        }
    }
}

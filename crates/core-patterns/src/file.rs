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
use crate::matcher::{Capture, Captures, SnippetParser};
use crate::metavar::{Meta, names, template, transform};
use core_config::Level;
use core_format::Directive;
use core_rules::tree::{TraversalCache, significant_range};
use core_rules::{
    Category, Check, Edit, Family, Fix, FixKind, Layer, LayoutRole, Opt, OptionDecl, Params, Re,
    Rule, RuleContext, RuleMeta, RuleOptions,
};
use core_syntax::{Grammar, SyntaxElement, SyntaxKind, SyntaxNode, TextRange};
use serde::Deserialize;
use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
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

/// `token:`: report each own child (token or node) of the matched node that satisfies this, not the node.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenSpec {
    kind: Vec<String>,
    /// Characters: a node whose text has none of them is not read (a test cheaper than `regex`).
    #[serde(default)]
    needs: String,
    regex: Option<String>,
    #[serde(rename = "not-regex")]
    not_regex: Option<String>,
    /// The first own token of this kind is not read (a command's name).
    #[serde(rename = "skip-first")]
    skip_first: Option<String>,
    /// Read every descendant of the matched node, not only its own children.
    #[serde(default)]
    deep: bool,
}

struct TokenMatch {
    kinds: Vec<SyntaxKind>,
    needs: Vec<char>,
    regex: Option<Rx>,
    not_regex: Option<Rx>,
    skip_first: Option<SyntaxKind>,
    deep: bool,
    /// The names of the groups of `regex`, which a hit captures (`$1`, `$NAME`); `$0` is the child.
    groups: Vec<String>,
    /// Does a template or `report-on`/`fix-on` read `$0` or a group? Only then a hit builds them.
    hit_captures: bool,
}

/// A regex of a constraint or of `token:`. One with `${NAME}` in it (a capture of the rule,
/// escaped) is built for each match.
enum Rx {
    Fixed(Re),
    Template(String, RegexCache),
}

/// Regexes compiled once for each source text they take.
#[derive(Default)]
struct RegexCache(Mutex<HashMap<String, Option<Arc<Re>>>>);

impl RegexCache {
    fn is_match(&self, source: &str, text: &str) -> bool {
        let re = {
            let mut cache = self.0.lock().unwrap();
            match cache.get(source) {
                Some(re) => re.clone(),
                None => {
                    let re = Re::new(source).ok().map(Arc::new);
                    cache.insert(source.to_string(), re.clone());
                    re
                }
            }
        };
        re.is_some_and(|re| re.is_match(text))
    }
}

/// What a metavariable of a template stands for in `text`: the capture, transformed.
fn captured(meta: &Meta, captures: &Captures, text: &str) -> String {
    let text = captures.get(&meta.name).map_or("", |c| c.text(text));
    match meta.transform.as_deref() {
        Some(t) => transform(t, text).unwrap_or_else(|| text.to_string()),
        None => text.to_string(),
    }
}

impl Rx {
    fn compile(source: &str, known: &dyn Fn(&str) -> bool) -> Result<Rx, String> {
        if !source.contains("${") {
            return Re::new(source)
                .map(Rx::Fixed)
                .map_err(|e| format!("regex `{source}`: {e}"));
        }
        let metas = names(source, true);
        check_template(&metas, known)?;
        Re::new(&template(source, |_| "x".into())).map_err(|e| format!("regex `{source}`: {e}"))?;
        Ok(Rx::Template(source.to_string(), RegexCache::default()))
    }

    fn is_match(&self, subject: &str, ascii: bool, captures: &Captures, text: &str) -> bool {
        match self {
            Rx::Fixed(re) => re.is_match_in(subject, ascii),
            Rx::Template(source, cache) => {
                let resolved = template(source, |m| regex::escape(&captured(m, captures, text)));
                cache.is_match(&resolved, subject)
            }
        }
    }
}

/// Every name of a template is captured (`known`) and every transform exists.
fn check_template(metas: &[Meta], known: &dyn Fn(&str) -> bool) -> Result<(), String> {
    for meta in metas.iter().filter(|m| m.captures()) {
        if !known(&meta.name) {
            return Err(format!(
                "`${}` is used in the message, fix or a regex but no pattern outside has/child/not captures it",
                meta.name
            ));
        }
        if let Some(t) = &meta.transform
            && transform(t, "").is_none()
        {
            return Err(format!("`{t}` is not a transform (`upper-snake`)"));
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSpec {
    id: String,
    /// Required in a user's file; a built-in file may leave it out (its crate's language).
    #[serde(default)]
    language: String,
    message: Option<String>,
    severity: Option<Level>,
    category: Option<CategorySpec>,
    description: Option<String>,
    example: Option<String>,
    rule: Option<Spec>,
    token: Option<TokenSpec>,
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
    regex: Option<Rx>,
    not_regex: Option<Rx>,
    kind: Option<SyntaxKind>,
    /// The list option (true) or string option (false) the capture must be in or equal.
    in_option: Option<(String, bool)>,
    /// The name of the `regex` option a constraint matches against, compiled once for each value.
    regex_option: Option<(String, RegexCache)>,
}

/// Where a diagnostic points: the matched node or what a metavariable captured, or the first
/// token of either.
struct ReportOn {
    first_token: bool,
    /// An empty span at the start.
    start: bool,
    target: Option<String>,
}

fn report_on(text: &str) -> Result<ReportOn, String> {
    let text = text.trim();
    let (first_token, rest) = match text.strip_prefix("first-token") {
        Some(rest) => (true, rest.trim()),
        None => (false, text),
    };
    let (start, rest) = match rest.strip_prefix("start") {
        Some(rest) => (true, rest.trim()),
        None => (false, rest),
    };
    let target = match rest.strip_prefix('$') {
        _ if rest.is_empty() => None,
        Some(name) if !name.is_empty() => Some(name.to_string()),
        _ => {
            return Err(format!(
                "report-on `{text}`: write `$NAME`, `first-token`, `first-token $NAME`, `start` or `start $NAME`"
            ));
        }
    };
    Ok(ReportOn {
        first_token,
        start,
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
            (OptionType::Bool | OptionType::Int | OptionType::String | OptionType::List, None) => {
                Check::Any
            }
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
    token: Option<TokenMatch>,
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
    known: &dyn Fn(&str) -> bool,
) -> Result<CompiledConstraint, String> {
    let regex = |text: &Option<String>| {
        text.as_deref()
            .map(|t| Rx::compile(t, known).map_err(|e| format!("constraint {name}: {e}")))
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
                Ok((d.name.to_string(), RegexCache::default()))
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
    let expr = Expr::compile(rule, parser, options)?;
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
    let token = spec
        .token
        .as_ref()
        .map(|t| compile_token(t, parser, &known))
        .transpose()?;
    if token.is_some() && (spec.message.is_none() || !spec.layout.is_empty()) {
        return Err("`token` needs a `message` and goes without `layout`".into());
    }
    // What a hit of `token` captures besides the rule: the child (`$0`) and the groups of `regex`.
    let rule_known = known;
    let known = |name: &str| {
        rule_known(name)
            || token
                .as_ref()
                .is_some_and(|t| name == "0" || t.groups.iter().any(|g| g == name))
    };
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
        check_template(&names(text, true), &known)?;
    }
    let mut constraints = BTreeMap::new();
    for (name, c) in &spec.constraints {
        if !rule_known(name) {
            return Err(format!(
                "constraint on `${name}`, which no pattern captures"
            ));
        }
        let compiled = compile_constraint(name, c, parser, options, &rule_known)?;
        constraints.insert(name.clone(), compiled);
    }
    let mut token = token;
    if let Some(t) = &mut token {
        let of_hit = |n: &str| n == "0" || t.groups.iter().any(|g| g == n);
        let used = [spec.message.as_deref(), spec.fix.as_deref()]
            .into_iter()
            .flatten()
            .flat_map(|text| names(text, true))
            .map(|m| m.name)
            .chain(report_on.iter().filter_map(|r| r.target.clone()))
            .chain(fix_on.clone());
        t.hit_captures = used.into_iter().any(|n| of_hit(&n));
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
        token,
    })
}

fn compile_token(
    t: &TokenSpec,
    parser: &dyn SnippetParser,
    known: &dyn Fn(&str) -> bool,
) -> Result<TokenMatch, String> {
    let re = |r: &Option<String>| {
        r.as_deref()
            .map(|r| Rx::compile(r, known).map_err(|e| format!("token {e}")))
            .transpose()
    };
    let regex = re(&t.regex)?;
    let groups = match &regex {
        Some(Rx::Fixed(re)) => re.groups(),
        _ => Vec::new(),
    };
    let kind = |n: &String| {
        parser
            .grammar()
            .kind_by_name(n)
            .ok_or_else(|| format!("`{n}` is not a kind of the language"))
    };
    let kinds = t.kind.iter().map(kind).collect::<Result<_, _>>()?;
    Ok(TokenMatch {
        kinds,
        needs: t.needs.chars().collect(),
        regex,
        not_regex: re(&t.not_regex)?,
        skip_first: t.skip_first.as_ref().map(kind).transpose()?,
        deep: t.deep,
        groups,
        hit_captures: false,
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
            || s.token.is_some()
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
            if language.is_empty() {
                return Err(PatternError(format!(
                    "rule `{id}`: give the rule a `language:`"
                )));
            }
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
        .map(|mut spec| {
            let id = spec.id.clone();
            let err = |e: String| PatternError(format!("rule `{id}`: {e}"));
            if spec.language.is_empty() {
                spec.language = language.to_string();
            }
            if spec.language != language {
                return Err(err(format!("language must be `{language}`")));
            }
            if !spec.layout.is_empty() {
                return Err(err("a built-in rule cannot have `layout`".into()));
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

fn substitute(tpl: &str, captures: &Captures, text: &str) -> String {
    template(tpl, |meta| captured(meta, captures, text))
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
        let capture = on.target.as_ref().and_then(|n| captures.get(n));
        let target = capture.map_or(range, |c| c.range);
        if on.start {
            // Of the whole node, trivia before it included, when no capture is named.
            let at = capture
                .map_or_else(|| node.text_range(), |c| c.range)
                .start();
            TextRange::empty(at)
        } else if on.first_token {
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
            c.regex
                .as_ref()
                .is_none_or(|re| re.is_match(captured, false, captures, text))
                && c.not_regex
                    .as_ref()
                    .is_none_or(|re| !re.is_match(captured, false, captures, text))
                && c.kind.is_none_or(|k| capture.kind == Some(k))
                && c.in_option.as_ref().is_none_or(|(option, is_list)| {
                    if *is_list {
                        options.list(option).contains(&captured)
                    } else {
                        options.string(option) == captured
                    }
                })
                && c.regex_option
                    .as_ref()
                    .is_none_or(|(option, cache)| cache.is_match(options.string(option), captured))
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
        if let Some(t) = body.token.as_ref().filter(|t| !t.needs.is_empty()) {
            let r = node.text_range();
            let text = &file.text[usize::from(r.start())..usize::from(r.end())];
            if !t.needs.iter().any(|&c| text.contains(c)) {
                return;
            }
        }
        let local = TraversalCache::default();
        let ranges = ctx.ranges().unwrap_or(&local);
        let cx = Ctx {
            grammar: file.grammar,
            path: file.path,
            text: file.text,
            options: ctx.options,
            budget: Cell::new(STEP_BUDGET),
            ranges,
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
        if let Some(t) = &body.token {
            if ctx.laying_out() {
                return;
            }
            let ascii = ranges.ascii(file.text);
            if t.deep {
                self.token_hits(
                    node.descendants_with_tokens(),
                    node,
                    t,
                    ascii,
                    &captures,
                    ctx,
                );
            } else {
                self.token_hits(node.children_with_tokens(), node, t, ascii, &captures, ctx);
            }
            return;
        }
        let range = ranges.significant_range(node, file.grammar);
        if ctx.laying_out() {
            self.attach_layout(node, &captures, ctx);
        } else {
            self.report_hit(node, &captures, range, ctx);
        }
    }
}

impl PatternRule {
    /// Reports the elements of `children` that the rule's `token` asks for.
    fn token_hits(
        &self,
        children: impl Iterator<Item = SyntaxElement>,
        node: &SyntaxNode,
        t: &TokenMatch,
        ascii: bool,
        captures: &Captures,
        ctx: &mut RuleContext<'_>,
    ) {
        let mut skip = t.skip_first;
        for child in children {
            let kind = child.kind();
            if skip == Some(kind) {
                skip = None;
            } else if t.kinds.contains(&kind) {
                self.token_hit(node, t, child, ascii, captures, ctx);
            }
        }
    }

    /// Reports `child` when it is a token the rule asks for.
    #[inline(never)]
    fn token_hit(
        &self,
        node: &SyntaxNode,
        t: &TokenMatch,
        child: SyntaxElement,
        ascii: bool,
        captures: &Captures,
        ctx: &mut RuleContext<'_>,
    ) {
        let file = ctx.file;
        let range = match &child {
            SyntaxElement::Node(n) => significant_range(n, file.grammar),
            SyntaxElement::Token(t) => t.text_range(),
        };
        let text = &file.text[usize::from(range.start())..usize::from(range.end())];
        let mut spans = Vec::new();
        match t.regex.as_ref() {
            Some(Rx::Fixed(re)) if !t.groups.is_empty() => match re.groups_in(text, ascii) {
                Some(g) => spans = g,
                None => return,
            },
            Some(re) if !re.is_match(text, ascii, captures, file.text) => return,
            _ => {}
        }
        if t.not_regex
            .as_ref()
            .is_some_and(|r| r.is_match(text, ascii, captures, file.text))
        {
            return;
        }
        if !t.hit_captures {
            return self.report_hit(node, captures, range, ctx);
        }
        // `$0` is the child, `$1`... the groups of `regex`.
        let mut hit = captures.clone();
        hit.insert("0".into(), Capture::span(range));
        hit.extend(Capture::groups(&t.groups, spans, range.start()));
        self.report_hit(node, &hit, range, ctx);
    }

    /// Reports one match, at `range` unless `report-on` says otherwise, with the fix of the rule.
    fn report_hit(
        &self,
        node: &SyntaxNode,
        captures: &Captures,
        range: TextRange,
        ctx: &mut RuleContext<'_>,
    ) {
        let (file, body) = (ctx.file, self.body());
        if let Some(message) = &self.message {
            let at = self.report_range(node, captures, range, file.grammar);
            let diagnostic = ctx.report(at, substitute(message, captures, file.text));
            if let Some(fix) = &body.fix {
                let edit = Self::fix_edit(fix, captures, range, file.text);
                diagnostic.fix(Fix::new(fix.title.clone(), fix.kind, vec![edit]));
            }
        }
    }
}

//! The `rule:` of a pattern rule file: atoms that all must hold for a node (`pattern`, `kind`,
//! `regex`, and the relations `inside`, `not-inside`, `has`, `not-has`) and the combinators
//! `all`, `any` and `not`.

use crate::matcher::{Capture, Captures, Pattern, SnippetParser};
use crate::metavar::Meta;
use core_rules::tree::TraversalCache;
use core_rules::{Opt, OptionDecl, Re, RuleOptions};
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};
use serde::Deserialize;
use std::cell::Cell;
use std::path::Path;

/// The rule as written in the file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub pattern: Option<String>,
    pub kind: Option<String>,
    pub regex: Option<String>,
    /// The file path (with `/`) matches this regex.
    pub path: Option<String>,
    /// The node is the Nth (from 1) of the children of its parent that have its kind.
    pub nth: Option<usize>,
    pub inside: Option<Box<Spec>>,
    #[serde(rename = "not-inside")]
    pub not_inside: Option<Box<Spec>>,
    pub has: Option<Box<Spec>>,
    #[serde(rename = "not-has")]
    pub not_has: Option<Box<Spec>>,
    /// The direct parent / a direct child holds the rule (`inside`/`has` look at every level).
    pub parent: Option<Box<Spec>>,
    pub child: Option<Box<Spec>>,
    /// The bool option of this name is true.
    pub option: Option<String>,
    pub all: Option<Vec<Spec>>,
    pub any: Option<Vec<Spec>>,
    pub not: Option<Box<Spec>>,
}

/// What an expression is evaluated against.
pub struct Ctx<'a> {
    pub grammar: &'a Grammar,
    pub path: &'a Path,
    pub text: &'a str,
    pub options: &'a RuleOptions,
    /// Matching steps left for this evaluation (see [`STEP_BUDGET`]); 0 means it ran out and
    /// the result is not to be trusted.
    pub budget: Cell<u32>,
    /// Shared with the other rules run on the same node.
    pub ranges: &'a TraversalCache,
}

impl Ctx<'_> {
    /// Spends one step; false when none are left.
    fn spend(&self) -> bool {
        let left = self.budget.get().saturating_sub(1);
        self.budget.set(left);
        left > 0
    }
}

/// Steps one rule may spend on one node (its `inside`/`has` walks included): about 10 ms.
pub const STEP_BUDGET: u32 = 1_000_000;

#[derive(Debug)]
pub enum Expr {
    Pattern(Pattern),
    Kind(SyntaxKind),
    /// With the names of its capture groups.
    Regex(Re, Vec<String>),
    Path(Re),
    Nth(usize),
    Inside(Box<Expr>),
    Has(Box<Expr>),
    Parent(Box<Expr>),
    Child(Box<Expr>),
    Option(String),
    All(Vec<Expr>),
    Any(Vec<Expr>),
    Not(Box<Expr>),
}

impl Expr {
    pub fn compile(
        spec: &Spec,
        parser: &dyn SnippetParser,
        options: &[OptionDecl],
    ) -> Result<Expr, String> {
        let mut all = Vec::new();
        let sub = |s: &Spec| Expr::compile(s, parser, options);
        if let Some(text) = &spec.pattern {
            all.push(Expr::Pattern(Pattern::compile(text, parser)?));
        }
        if let Some(name) = &spec.kind {
            let kind = parser
                .grammar()
                .kind_by_name(name)
                .filter(|k| !k.is_token())
                .ok_or_else(|| format!("`{name}` is not a node kind of the language"))?;
            all.push(Expr::Kind(kind));
        }
        if let Some(text) = &spec.regex {
            let re = Re::new(text).map_err(|e| format!("regex `{text}`: {e}"))?;
            let groups = re.groups();
            all.push(Expr::Regex(re, groups));
        }
        if let Some(text) = &spec.path {
            all.push(Expr::Path(
                Re::new(text).map_err(|e| format!("path regex `{text}`: {e}"))?,
            ));
        }
        if let Some(n) = spec.nth {
            if n == 0 {
                return Err("nth counts from 1".into());
            }
            all.push(Expr::Nth(n));
        }
        if let Some(s) = &spec.inside {
            all.push(Expr::Inside(Box::new(sub(s)?)));
        }
        if let Some(s) = &spec.not_inside {
            all.push(Expr::Not(Box::new(Expr::Inside(Box::new(sub(s)?)))));
        }
        if let Some(s) = &spec.has {
            all.push(Expr::Has(Box::new(sub(s)?)));
        }
        if let Some(s) = &spec.not_has {
            all.push(Expr::Not(Box::new(Expr::Has(Box::new(sub(s)?)))));
        }
        if let Some(s) = &spec.parent {
            all.push(Expr::Parent(Box::new(sub(s)?)));
        }
        if let Some(s) = &spec.child {
            all.push(Expr::Child(Box::new(sub(s)?)));
        }
        if let Some(name) = &spec.option {
            let bool_option = |o: &&OptionDecl| o.name == name && matches!(o.default, Opt::Bool(_));
            if !options.iter().any(|o| bool_option(&o)) {
                return Err(format!("option `{name}` is not a declared `bool` option"));
            }
            all.push(Expr::Option(name.clone()));
        }
        if let Some(list) = &spec.all {
            all.push(Expr::All(list.iter().map(sub).collect::<Result<_, _>>()?));
        }
        if let Some(list) = &spec.any {
            all.push(Expr::Any(list.iter().map(sub).collect::<Result<_, _>>()?));
        }
        if let Some(s) = &spec.not {
            all.push(Expr::Not(Box::new(sub(s)?)));
        }
        match all.len() {
            0 => Err(
                "a rule needs at least one of pattern, kind, regex, path, nth, inside, not-inside, \
                      has, not-has, parent, child, option, all, any, not"
                    .into(),
            ),
            1 => Ok(all.remove(0)),
            _ => Ok(Expr::All(all)),
        }
    }

    /// The captures when the expression holds for `node`.
    pub fn matches(&self, node: &SyntaxNode, cx: &Ctx<'_>) -> Option<Captures> {
        match self {
            Expr::Pattern(p) => p.matches(node, cx.grammar, &cx.budget),
            Expr::Kind(k) => (node.kind() == *k).then(Captures::new),
            Expr::Regex(re, groups) => {
                let range = cx.ranges.significant_range(node, cx.grammar);
                let text = &cx.text[range];
                let ascii = cx.ranges.ascii(cx.text);
                if groups.is_empty() {
                    return re.is_match_in(text, ascii).then(Captures::new);
                }
                let spans = re.groups_in(text, ascii)?;
                Some(Capture::groups(groups, spans, range.start()).collect())
            }
            Expr::Path(re) => {
                let path = cx.path.to_string_lossy().replace('\\', "/");
                re.is_match(&path).then(Captures::new)
            }
            Expr::Nth(n) => {
                let mut same = node
                    .parent()?
                    .children()
                    .filter(|c| c.kind() == node.kind());
                (same.nth(n - 1).as_ref() == Some(node)).then(Captures::new)
            }
            Expr::Inside(e) => node
                .ancestors()
                .skip(1)
                .find_map(|a| if cx.spend() { e.matches(&a, cx) } else { None }),
            Expr::Has(e) => node
                .descendants()
                .skip(1)
                .any(|d| cx.spend() && e.matches(&d, cx).is_some())
                .then(Captures::new),
            Expr::Parent(e) => e.matches(&node.parent()?, cx),
            Expr::Child(e) => node
                .children()
                .any(|c| cx.spend() && e.matches(&c, cx).is_some())
                .then(Captures::new),
            Expr::Option(name) => cx.options.bool(name).then(Captures::new),
            Expr::All(list) => {
                let mut captures = Captures::new();
                for e in list {
                    for (name, capture) in e.matches(node, cx)? {
                        captures.entry(name).or_insert(capture);
                    }
                }
                Some(captures)
            }
            Expr::Any(list) => list.iter().find_map(|e| e.matches(node, cx)),
            Expr::Not(e) => e.matches(node, cx).is_none().then(Captures::new),
        }
    }

    /// Can evaluating this cost more than linear in the node's size: a `$$$` run (all splits are
    /// tried) or an `inside`/`has` walk?
    pub fn superlinear(&self) -> bool {
        match self {
            Expr::Pattern(p) => p.metas().iter().any(|m| m.multi),
            Expr::Inside(_) | Expr::Has(_) => true,
            Expr::All(list) | Expr::Any(list) => list.iter().any(Expr::superlinear),
            Expr::Not(e) | Expr::Parent(e) | Expr::Child(e) => e.superlinear(),
            Expr::Kind(_) | Expr::Regex(..) | Expr::Path(_) | Expr::Nth(_) | Expr::Option(_) => {
                false
            }
        }
    }

    /// The node kinds a match can start at, when the expression pins them down (the engine then
    /// calls the rule for those kinds only).
    pub fn kinds(&self) -> Option<Vec<SyntaxKind>> {
        match self {
            Expr::Pattern(p) => Some(vec![p.kind()]),
            Expr::Kind(k) => Some(vec![*k]),
            Expr::All(list) => list.iter().find_map(Expr::kinds),
            Expr::Any(list) => {
                let mut kinds = Vec::new();
                for e in list {
                    kinds.extend(e.kinds()?);
                }
                Some(kinds)
            }
            _ => None,
        }
    }

    /// Every metavariable a match can capture: those of the patterns outside `has`, `child` and
    /// `not`, whose captures are dropped, and the groups of a `regex` (`$1`).
    pub fn metas(&self, out: &mut Vec<Meta>) {
        match self {
            Expr::Pattern(p) => out.extend(p.metas().iter().cloned()),
            Expr::Regex(_, groups) => {
                out.extend(groups.iter().filter(|n| !n.is_empty()).map(|name| Meta {
                    multi: false,
                    name: name.clone(),
                    transform: None,
                }))
            }
            Expr::All(list) | Expr::Any(list) => list.iter().for_each(|e| e.metas(out)),
            Expr::Inside(e) | Expr::Parent(e) => e.metas(out),
            Expr::Kind(_)
            | Expr::Path(_)
            | Expr::Nth(_)
            | Expr::Option(_)
            | Expr::Has(_)
            | Expr::Child(_)
            | Expr::Not(_) => {}
        }
    }
}

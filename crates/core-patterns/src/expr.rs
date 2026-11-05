//! The `rule:` of a pattern rule file: atoms that all must hold for a node (`pattern`, `kind`,
//! `regex`, and the relations `inside`, `not-inside`, `has`, `not-has`) and the combinators
//! `all`, `any` and `not`.

use crate::matcher::{Captures, Pattern, SnippetParser};
use crate::metavar::Meta;
use core_rules::tree::significant_range;
use core_syntax::{Grammar, SyntaxKind, SyntaxNode};
use regex::Regex;
use serde::Deserialize;
use std::cell::Cell;

/// The rule as written in the file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub pattern: Option<String>,
    pub kind: Option<String>,
    pub regex: Option<String>,
    pub inside: Option<Box<Spec>>,
    #[serde(rename = "not-inside")]
    pub not_inside: Option<Box<Spec>>,
    pub has: Option<Box<Spec>>,
    #[serde(rename = "not-has")]
    pub not_has: Option<Box<Spec>>,
    pub all: Option<Vec<Spec>>,
    pub any: Option<Vec<Spec>>,
    pub not: Option<Box<Spec>>,
}

/// What an expression is evaluated against.
pub struct Ctx<'a> {
    pub grammar: &'a Grammar,
    pub text: &'a str,
    /// Matching steps left for this evaluation (see [`STEP_BUDGET`]); 0 means it ran out and
    /// the result is not to be trusted.
    pub budget: Cell<u32>,
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
    Regex(Regex),
    Inside(Box<Expr>),
    Has(Box<Expr>),
    All(Vec<Expr>),
    Any(Vec<Expr>),
    Not(Box<Expr>),
}

impl Expr {
    pub fn compile(spec: &Spec, parser: &dyn SnippetParser) -> Result<Expr, String> {
        let mut all = Vec::new();
        let sub = |s: &Spec| Expr::compile(s, parser);
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
            all.push(Expr::Regex(
                Regex::new(text).map_err(|e| format!("regex `{text}`: {e}"))?,
            ));
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
                "a rule needs at least one of pattern, kind, regex, inside, not-inside, \
                      has, not-has, all, any, not"
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
            Expr::Regex(re) => {
                let range = significant_range(node, cx.grammar);
                let text = &cx.text[range];
                re.is_match(text).then(Captures::new)
            }
            Expr::Inside(e) => node
                .ancestors()
                .skip(1)
                .any(|a| cx.spend() && e.matches(&a, cx).is_some())
                .then(Captures::new),
            Expr::Has(e) => node
                .descendants()
                .skip(1)
                .any(|d| cx.spend() && e.matches(&d, cx).is_some())
                .then(Captures::new),
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
            Expr::Not(e) => e.superlinear(),
            Expr::Kind(_) | Expr::Regex(_) => false,
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

    /// Every metavariable a match can capture: those of the patterns outside `inside`, `has` and
    /// `not`, whose captures are dropped.
    pub fn metas(&self, out: &mut Vec<Meta>) {
        match self {
            Expr::Pattern(p) => out.extend(p.metas().iter().cloned()),
            Expr::All(list) | Expr::Any(list) => list.iter().for_each(|e| e.metas(out)),
            Expr::Kind(_) | Expr::Regex(_) | Expr::Inside(_) | Expr::Has(_) | Expr::Not(_) => {}
        }
    }
}

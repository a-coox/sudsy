//! The `names` family: a check on the names of one kind of thing (properties, units, functions,
//! at-rules, parts of selectors, ...). Params:
//! - `target`: what the names are, see [`Target`];
//! - `mode`: `allowed` and `disallowed` (option `list`: names, or `/regex/`), `pattern` (each
//!   name must match option `pattern`), `lower` (each name is written in lowercase), `unique`
//!   (a name repeats within the node).
//!
//! The message has `{name}`.

use super::model::{self, Part, plain, split_dimension};
use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::unique::Repeats;
use core_rules::{Family, Params, Re, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt, TextRange};
use std::sync::Mutex;

pub const FAMILY: Family = Family {
    name: "names",
    build,
};

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Property,
    CustomProperty,
    Function,
    Unit,
    AtRule,
    MediaFeature,
    KeyframesName,
    CustomMedia,
    Layer,
    Container,
    Combinator,
    Selector,
    AttributeOperator,
    FontFamily,
    Part(Part),
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Allowed,
    Disallowed,
    Pattern,
    Lower,
    Unique,
}

struct Names {
    meta: &'static RuleMeta,
    target: Target,
    mode: Mode,
    message: String,
    kinds: &'static [SyntaxKind],
    /// The compiled `pattern` option, by its source.
    cache: Mutex<Option<(String, Option<Re>)>>,
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    use Target::*;
    let target = p.choice(
        "target",
        true,
        &[
            ("property", Property),
            ("custom-property", CustomProperty),
            ("function", Function),
            ("unit", Unit),
            ("at-rule", AtRule),
            ("media-feature", MediaFeature),
            ("keyframes-name", KeyframesName),
            ("custom-media", CustomMedia),
            ("layer", Layer),
            ("container", Container),
            ("combinator", Combinator),
            ("selector", Selector),
            ("attribute-operator", AttributeOperator),
            ("font-family", FontFamily),
            ("class", Part(model::Part::Class)),
            ("id", Part(model::Part::Id)),
            ("attribute", Part(model::Part::Attribute)),
            ("pseudo-class", Part(model::Part::PseudoClass)),
            ("pseudo-element", Part(model::Part::PseudoElement)),
            ("type", Part(model::Part::Type)),
        ],
    )?;
    let mode = p.choice(
        "mode",
        true,
        &[
            ("allowed", Mode::Allowed),
            ("disallowed", Mode::Disallowed),
            ("pattern", Mode::Pattern),
            ("lower", Mode::Lower),
            ("unique", Mode::Unique),
        ],
    )?;
    let kinds: &'static [SyntaxKind] = match target {
        Property | FontFamily => &[k::DECLARATION],
        CustomProperty => &[k::DECLARATION, k::AT_RULE],
        Function => &[k::FUNCTION],
        Unit => &[k::VALUE, k::PRELUDE],
        AtRule | MediaFeature | KeyframesName | CustomMedia | Layer | Container => &[k::AT_RULE],
        Combinator | Selector | AttributeOperator | Part(_) => &[k::RULE],
    };
    let rule = Names {
        meta,
        target,
        mode,
        message: p.text("message")?,
        kinds,
        cache: Mutex::new(None),
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Names {
    /// The names in `node` with where each is.
    fn collect(&self, node: &SyntaxNode, ctx: &RuleContext<'_>) -> Vec<(String, TextRange)> {
        let range = |n: &SyntaxNode| significant_range(n, ctx.file.grammar);
        let mut out = Vec::new();
        match self.target {
            Target::Property => {
                if let Some((name, at)) = model::property(node)
                    && !model::is_custom(&name)
                {
                    out.push((name, at));
                }
            }
            // The names without `--`: declared, used in `var()` and named by `@property`.
            Target::CustomProperty if node.kind() == k::AT_RULE => {
                if model::at_name(node) == "property"
                    && let Some(p) = node.child(k::PRELUDE)
                {
                    out.push((plain(&p).trim_start_matches("--").to_string(), range(&p)));
                }
            }
            Target::CustomProperty => {
                if let Some((name, at)) = model::property(node)
                    && model::is_custom(&name)
                {
                    out.push((name[2..].to_string(), at));
                }
                for f in node.descendants().filter(|n| n.kind() == k::FUNCTION) {
                    let var = model::function_name(&f)
                        .is_some_and(|t| t.text().eq_ignore_ascii_case("var"));
                    let first = f.child(k::GROUP).and_then(|g| model::tokens(&g).nth(1));
                    if let (true, Some(t)) = (var, first)
                        && t.text().starts_with("--")
                    {
                        out.push((t.text()[2..].to_string(), t.text_range()));
                    }
                }
            }
            Target::Function => {
                if let (false, Some(t)) = (after_colon(node), model::function_name(node)) {
                    out.push((t.text().to_string(), t.text_range()));
                }
            }
            Target::Unit => {
                if node.kind() == k::PRELUDE && node.parent().is_none_or(|p| p.kind() != k::AT_RULE)
                {
                    return out;
                }
                for t in model::tokens(node) {
                    match t.kind() {
                        k::DIMENSION => {
                            out.push((split_dimension(t.text()).1.to_string(), t.text_range()))
                        }
                        k::PERCENTAGE => out.push(("%".into(), t.text_range())),
                        _ => {}
                    }
                }
            }
            Target::AtRule => {
                if let Some(t) = node.token(k::AT_KEYWORD) {
                    let name = model::at_name(node);
                    out.push((model::unprefixed(&name).to_string(), t.text_range()));
                }
            }
            Target::MediaFeature => {
                if model::at_name(node) == "media"
                    && let Some(prelude) = node.child(k::PRELUDE)
                {
                    for g in prelude.descendants().filter(|n| n.kind() == k::GROUP) {
                        if let Some(t) = model::tokens(&g).nth(1)
                            && t.kind() == k::IDENT
                            && model::tokens(&g)
                                .next()
                                .is_some_and(|o| o.kind() == k::L_PAREN)
                        {
                            out.push((t.text().to_string(), t.text_range()));
                        }
                    }
                }
            }
            Target::KeyframesName => {
                if model::is_keyframes(node)
                    && let Some(p) = node.child(k::PRELUDE)
                {
                    out.push((
                        model::plain(&p).trim_matches(['"', '\'']).to_string(),
                        range(&p),
                    ));
                }
            }
            Target::CustomMedia => {
                if model::at_name(node) == "custom-media"
                    && let Some(t) = node
                        .child(k::PRELUDE)
                        .and_then(|p| model::tokens(&p).next())
                {
                    out.push((
                        t.text().trim_start_matches("--").to_string(),
                        t.text_range(),
                    ));
                }
            }
            Target::Layer => {
                if model::at_name(node) == "layer"
                    && let Some(p) = node.child(k::PRELUDE)
                {
                    for part in model::tokens(&p)
                        .collect::<Vec<_>>()
                        .split(|t| t.kind() == k::COMMA)
                    {
                        if let (Some(a), Some(b)) = (part.first(), part.last()) {
                            let name = part.iter().map(|t| t.text()).collect();
                            out.push((name, a.text_range().cover(b.text_range())));
                        }
                    }
                }
            }
            Target::Container => {
                if model::at_name(node) == "container"
                    && let Some(t) = node
                        .child(k::PRELUDE)
                        .and_then(|p| model::tokens(&p).next())
                    && t.kind() == k::IDENT
                    && !matches!(t.text().to_ascii_lowercase().as_str(), "not" | "and" | "or")
                {
                    out.push((t.text().to_string(), t.text_range()));
                }
            }
            Target::FontFamily => {
                if let Some(families) = super::font::families(node) {
                    out.extend(families.into_iter().map(|f| (f.name, f.range)));
                }
            }
            Target::Combinator | Target::Selector | Target::AttributeOperator | Target::Part(_) => {
                let Some(prelude) = node.child(k::PRELUDE) else {
                    return out;
                };
                // Keyframe selectors are not selectors, except for their case.
                if model::in_keyframes(node) && self.mode != Mode::Lower {
                    return out;
                }
                if self.target == Target::Selector {
                    // The whole list, as stylelint reads it.
                    let list: Vec<String> = model::selectors(&prelude)
                        .into_iter()
                        .map(|s| s.text)
                        .collect();
                    out.push((list.join(", "), range(&prelude)));
                    return out;
                }
                for sel in model::selectors(&prelude) {
                    match self.target {
                        Target::AttributeOperator => {
                            for p in sel.pieces().filter(|p| p.part == Part::Attribute) {
                                let text = &ctx.file.text
                                    [usize::from(p.range.start())..usize::from(p.range.end())];
                                let after =
                                    &text[text.find(&*p.name).map_or(0, |i| i + p.name.len())..];
                                let op: String = after
                                    .trim_start()
                                    .chars()
                                    .take_while(|c| "~|^$*=".contains(*c))
                                    .collect();
                                if !op.is_empty() {
                                    out.push((op, p.range));
                                }
                            }
                        }
                        Target::Combinator => {
                            out.extend(sel.combinators.iter().map(|(c, r)| (c.clone(), *r)))
                        }
                        Target::Part(part) => out.extend(
                            sel.pieces()
                                .filter(|p| p.part == part)
                                .map(|p| (p.name.clone(), p.range)),
                        ),
                        _ => {}
                    }
                }
            }
        }
        out
    }

    fn insensitive(&self) -> bool {
        !matches!(
            self.target,
            Target::Part(Part::Class | Part::Id | Part::Attribute | Part::Type)
                | Target::KeyframesName
                | Target::Selector
                | Target::FontFamily
                | Target::Function
        )
    }

    fn pattern_match(&self, source: &str, name: &str) -> bool {
        let mut cache = self.cache.lock().unwrap();
        if cache.as_ref().is_none_or(|(s, _)| s != source) {
            *cache = Some((source.to_string(), Re::lazy(source).ok()));
        }
        cache
            .as_ref()
            .and_then(|(_, re)| re.as_ref())
            .is_none_or(|re| re.is_match(name))
    }
}

/// Whether a `:` comes just before `node` (a pseudo-class, not a function).
pub(super) fn after_colon(node: &SyntaxNode) -> bool {
    prev_token(node).is_some_and(|t| t.kind() == k::COLON)
}

fn prev_token(node: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    let mut e = node.prev_sibling_or_token();
    while let Some(el) = e {
        match el {
            core_syntax::SyntaxElement::Token(t) if model::is_trivia(t.kind()) => {
                e = t.prev_sibling_or_token()
            }
            core_syntax::SyntaxElement::Token(t) => return Some(t),
            core_syntax::SyntaxElement::Node(_) => return None,
        }
    }
    None
}

/// Whether `entry` (a name, or `/regex/`) selects `name`.
pub(super) fn selects(entry: &str, name: &str, insensitive: bool) -> bool {
    match entry.strip_prefix('/').and_then(|e| e.strip_suffix('/')) {
        Some(re) => Re::lazy(re).is_ok_and(|re| re.is_match(name)),
        None if insensitive => entry.eq_ignore_ascii_case(name),
        None => entry == name,
    }
}

/// `translateX` and the like keep their capitals.
const CAMEL: &[&str] = &[
    "translateX",
    "translateY",
    "translateZ",
    "scaleX",
    "scaleY",
    "scaleZ",
    "rotateX",
    "rotateY",
    "rotateZ",
    "skewX",
    "skewY",
];

impl Rule for Names {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let names = self.collect(node, ctx);
        if names.is_empty() {
            return;
        }
        let list = ctx.options.list("list");
        let pattern = ctx.options.string("pattern");
        let insensitive = self.insensitive();
        let mut seen = Repeats::default();
        for (name, range) in names {
            let hit = match self.mode {
                Mode::Allowed => {
                    !list.is_empty() && !list.iter().any(|e| selects(e, &name, insensitive))
                }
                Mode::Disallowed => list.iter().any(|e| selects(e, &name, insensitive)),
                Mode::Pattern => !pattern.is_empty() && !self.pattern_match(pattern, &name),
                Mode::Lower => {
                    name.chars().any(|c| c.is_uppercase())
                        && !(self.target == Target::Function && CAMEL.contains(&name.as_str()))
                }
                Mode::Unique => {
                    let message = || self.message.replace("{name}", &name);
                    seen.check(
                        ctx,
                        name.to_ascii_lowercase(),
                        range,
                        message,
                        "first used here",
                    );
                    continue;
                }
            };
            if hit {
                ctx.report(range, self.message.replace("{name}", &name));
            }
        }
    }
}

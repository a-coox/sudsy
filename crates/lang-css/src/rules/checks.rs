//! The `check` family: rules that each read the tree their own way. Param `check` names the
//! function; the message has `{name}`.

use super::font;
use super::model::{self, group_items, is_trivia, plain};
use super::values as v;
use crate::generated::kind as k;
use core_rules::tree::significant_range;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxNodeExt, TextRange};
use std::collections::HashMap;

pub const FAMILY: Family = Family {
    name: "check",
    build,
};

pub(super) struct Check {
    meta: &'static RuleMeta,
    message: String,
    kinds: &'static [SyntaxKind],
    run: fn(&Check, &SyntaxNode, &mut RuleContext<'_>),
}

type Entry = (
    &'static str,
    &'static [SyntaxKind],
    fn(&Check, &SyntaxNode, &mut RuleContext<'_>),
);

const CHECKS: &[Entry] = &[
    ("duplicate-properties", &[k::BLOCK], duplicate_properties),
    (
        "duplicate-custom-properties",
        &[k::BLOCK],
        duplicate_custom_properties,
    ),
    (
        "duplicate-selectors",
        &[k::STYLESHEET, k::BLOCK],
        duplicate_selectors,
    ),
    (
        "duplicate-keyframe-selectors",
        &[k::AT_RULE],
        duplicate_keyframe_selectors,
    ),
    ("duplicate-imports", &[k::STYLESHEET], duplicate_imports),
    ("import-position", &[k::AT_RULE], import_position),
    ("calc-operators", &[k::FUNCTION], calc_operators),
    (
        "linear-gradient-direction",
        &[k::FUNCTION],
        linear_gradient_direction,
    ),
    (
        "missing-generic-family",
        &[k::DECLARATION],
        missing_generic_family,
    ),
    ("shorthand-overrides", &[k::BLOCK], shorthand_overrides),
    (
        "descending-specificity",
        &[k::STYLESHEET],
        v::descending_specificity,
    ),
    (
        "declaration-position",
        &[k::DECLARATION],
        v::declaration_position,
    ),
    (
        "missing-var-function",
        &[k::STYLESHEET],
        v::missing_var_function,
    ),
    ("grid-areas", &[k::DECLARATION], v::grid_areas),
    ("hex-length", &[k::VALUE], v::hex_length),
    (
        "keyframe-selector-notation",
        &[k::AT_RULE],
        v::keyframe_selector_notation,
    ),
    ("redundant-values", &[k::DECLARATION], v::redundant_values),
    (
        "font-family-quotes",
        &[k::DECLARATION],
        v::font_family_quotes,
    ),
    ("nth-unmatchable", &[k::FUNCTION], v::nth_unmatchable),
    ("qualifying-type", &[k::RULE], v::qualifying_type),
];

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    let name = p.text("check")?;
    let Some((_, kinds, run)) = CHECKS.iter().find(|(n, ..)| *n == name) else {
        return Err(format!("param `check`: unknown check `{name}`"));
    };
    let rule = Check {
        meta,
        message: p.text("message")?,
        kinds,
        run: *run,
    };
    p.done()?;
    Ok(Box::new(rule))
}

impl Rule for Check {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        (self.run)(self, node, ctx);
    }
}

impl Check {
    pub(super) fn hit<'a>(
        &self,
        ctx: &'a mut RuleContext<'_>,
        range: TextRange,
        name: &str,
    ) -> &'a mut core_rules::Diagnostic {
        ctx.report(range, self.message.replace("{name}", name))
    }
}

fn declarations(block: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    block.children().filter(|c| c.kind() == k::DECLARATION)
}

fn is_important(decl: &SyntaxNode) -> bool {
    model::value(decl).is_some_and(|v| plain(&v).to_ascii_lowercase().ends_with("!important"))
}

/// Each repeat of a property reports the declaration before it (the later one when only the
/// earlier is `!important`). With the option, a repeat right after the same property with another
/// value is a fallback and left alone.
fn duplicate_properties(c: &Check, block: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let fallbacks = ctx.options.bool("ignore_consecutive_different_values");
    let value = |d: &SyntaxNode| model::value(d).map(|v| plain(&v));
    let mut last: HashMap<String, SyntaxNode> = HashMap::new();
    let mut previous = String::new();
    for decl in declarations(block) {
        let Some((name, at)) = model::property(&decl) else {
            continue;
        };
        if model::is_custom(&name) || name == "src" {
            continue;
        }
        let consecutive = std::mem::replace(&mut previous, name.clone()) == name;
        let Some(before) = last.get(&name).cloned() else {
            last.insert(name, decl);
            continue;
        };
        if fallbacks && consecutive && value(&before) != value(&decl) {
            continue;
        }
        if is_important(&before) && !is_important(&decl) {
            c.hit(ctx, at, &name);
        } else {
            let range = model::property(&before).map_or(before.text_range(), |p| p.1);
            c.hit(ctx, range, &name);
            last.insert(name.clone(), decl);
        }
    }
}

fn duplicate_custom_properties(c: &Check, block: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut seen = HashMap::new();
    for decl in declarations(block) {
        let Some(p) = decl.child(k::PROPERTY) else {
            continue;
        };
        let name = plain(&p);
        if model::is_custom(&name) && seen.insert(name.clone(), ()).is_some() {
            c.hit(ctx, significant_range(&p, ctx.file.grammar), &name);
        }
    }
}

/// A rule's selector list, sorted and whitespace-collapsed.
fn selector_key(rule: &SyntaxNode) -> Option<String> {
    let mut list: Vec<String> = model::selectors(&rule.child(k::PRELUDE)?)
        .into_iter()
        .map(|s| s.text)
        .collect();
    list.sort();
    Some(list.join(", "))
}

fn duplicate_selectors(c: &Check, container: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut seen: HashMap<String, TextRange> = HashMap::new();
    for rule in container.children().filter(|n| n.kind() == k::RULE) {
        let Some(key) = selector_key(&rule) else {
            continue;
        };
        let range = significant_range(&rule.child(k::PRELUDE).unwrap(), ctx.file.grammar);
        if let Some(first) = seen.get(&key) {
            c.hit(ctx, range, &key).related(*first, "first used here");
        } else {
            seen.insert(key, range);
        }
    }
}

fn duplicate_keyframe_selectors(c: &Check, at: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !model::is_keyframes(at) {
        return;
    }
    let mut seen = HashMap::new();
    for rule in model::items(at).filter(|n| n.kind() == k::RULE) {
        let Some(prelude) = rule.child(k::PRELUDE) else {
            continue;
        };
        for sel in model::selectors(&prelude) {
            let key = sel.text.to_ascii_lowercase();
            if seen.insert(key, ()).is_some() {
                c.hit(
                    ctx,
                    significant_range(&prelude, ctx.file.grammar),
                    &sel.text,
                );
            }
        }
    }
}

fn duplicate_imports(c: &Check, sheet: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut seen = HashMap::new();
    for at in sheet.children().filter(|n| n.kind() == k::AT_RULE) {
        if model::at_name(&at) != "import" {
            continue;
        }
        let Some(prelude) = at.child(k::PRELUDE) else {
            continue;
        };
        let key: String = plain(&prelude)
            .replace("url(", "")
            .replace([')', '"', '\''], "")
            .to_ascii_lowercase();
        if seen.insert(key.clone(), ()).is_some() {
            c.hit(ctx, significant_range(&at, ctx.file.grammar), &key);
        }
    }
}

/// `@import` comes before everything but `@charset`, `@layer` statements and other imports.
fn import_position(c: &Check, at: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    // Sass and Less compile `@import` wherever it stands.
    if model::at_name(at) != "import"
        || at.parent().is_none_or(|p| p.kind() != k::STYLESHEET)
        || super::table::dialect(ctx).is_some()
    {
        return;
    }
    let mut before = at.prev_sibling();
    while let Some(n) = before {
        let allowed = n.kind() == k::EMPTY
            || (n.kind() == k::AT_RULE && {
                let name = model::at_name(&n);
                name == "charset"
                    || name == "import"
                    || (name == "layer" && n.child(k::BLOCK).is_none())
            });
        if !allowed {
            c.hit(ctx, significant_range(at, ctx.file.grammar), "");
            return;
        }
        before = n.prev_sibling();
    }
}

/// The items of a group without trivia.
fn spaced_items(group: &SyntaxNode) -> Vec<SyntaxElement> {
    group_items_with_trivia(group)
        .into_iter()
        .filter(|e| !is_trivia(e.kind()))
        .collect()
}

fn group_items_with_trivia(group: &SyntaxNode) -> Vec<SyntaxElement> {
    let mut all: Vec<_> = group.children_with_tokens().collect();
    if all.len() >= 2 {
        all.remove(0);
        all.pop();
    }
    all
}

/// Whether an element can end an operand: a number, a group, a function or a name.
fn is_operand(e: &SyntaxElement) -> bool {
    match e {
        SyntaxElement::Node(_) => true,
        SyntaxElement::Token(t) => matches!(
            t.kind(),
            k::NUMBER | k::DIMENSION | k::PERCENTAGE | k::IDENT | k::VARIABLE
        ),
    }
}

fn check_calc_group(c: &Check, group: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let items = spaced_items(group);
    // Whitespace after a function or group belongs to that node, so spacing is read from the text.
    let spaced = |at: usize, before: bool| {
        let text = ctx.file.text;
        let c = if before {
            text[..at].chars().next_back()
        } else {
            text[at..].chars().next()
        };
        c.is_none_or(char::is_whitespace)
    };
    for (i, e) in items.iter().enumerate() {
        match e {
            SyntaxElement::Token(t) if t.kind() == k::DELIM && matches!(t.text(), "+" | "-") => {
                let (start, end) = (
                    usize::from(t.text_range().start()),
                    usize::from(t.text_range().end()),
                );
                if (i > 0 && !spaced(start, true)) || (i + 1 < items.len() && !spaced(end, false)) {
                    c.hit(ctx, t.text_range(), t.text());
                }
            }
            SyntaxElement::Token(t)
                if matches!(t.kind(), k::NUMBER | k::DIMENSION | k::PERCENTAGE)
                    && t.text().starts_with(['+', '-'])
                    && i > 0
                    && is_operand(&items[i - 1]) =>
            {
                c.hit(ctx, t.text_range(), &t.text()[..1]);
            }
            SyntaxElement::Node(n) if n.kind() == k::GROUP => check_calc_group(c, n, ctx),
            SyntaxElement::Node(n) if n.kind() == k::FUNCTION => {
                if let Some(g) = n.child(k::GROUP) {
                    check_calc_group(c, &g, ctx);
                }
            }
            _ => {}
        }
    }
}

fn calc_operators(c: &Check, f: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let named = model::function_name(f)
        .is_some_and(|t| model::unprefixed(&t.text().to_ascii_lowercase()) == "calc");
    // Nested calcs are reached from their outer one.
    let nested = f.ancestors().skip(1).any(|a| {
        a.kind() == k::FUNCTION
            && model::function_name(&a)
                .is_some_and(|t| model::unprefixed(&t.text().to_ascii_lowercase()) == "calc")
    });
    if let (true, false, Some(g)) = (named, nested, f.child(k::GROUP)) {
        check_calc_group(c, &g, ctx);
    }
}

fn linear_gradient_direction(c: &Check, f: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(name) = model::function_name(f) else {
        return;
    };
    let name = name.text().to_ascii_lowercase();
    if name != "linear-gradient" && name != "repeating-linear-gradient" {
        return;
    }
    let Some(group) = f.child(k::GROUP) else {
        return;
    };
    let items = group_items(&group);
    if let Some(SyntaxElement::Token(t)) = items.first()
        && t.kind() == k::IDENT
        && matches!(
            t.text().to_ascii_lowercase().as_str(),
            "top" | "left" | "bottom" | "right"
        )
    {
        c.hit(ctx, t.text_range(), t.text());
    }
}

fn missing_generic_family(c: &Check, decl: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if decl
        .ancestors()
        .skip(1)
        .any(|a| a.kind() == k::AT_RULE && model::at_name(&a) == "font-face")
    {
        return;
    }
    let Some(families) = font::of(decl) else {
        return;
    };
    let keyword = |f: &font::Family| {
        !f.quoted
            && (font::GENERIC.contains(&f.name.to_ascii_lowercase().as_str())
                || matches!(
                    f.name.to_ascii_lowercase().as_str(),
                    "inherit" | "initial" | "unset" | "revert" | "revert-layer"
                ))
    };
    if let Some(last) = families.last()
        && !families.iter().any(keyword)
    {
        c.hit(ctx, last.range, &last.name);
    }
}

/// Shorthand properties and the longhands they reset.
const SHORTHANDS: &[(&str, &str)] = &[
    (
        "margin",
        "margin-top margin-right margin-bottom margin-left",
    ),
    (
        "padding",
        "padding-top padding-right padding-bottom padding-left",
    ),
    (
        "border",
        "border-width border-style border-color border-top border-right border-bottom border-left border-top-width border-right-width border-bottom-width border-left-width border-top-style border-right-style border-bottom-style border-left-style border-top-color border-right-color border-bottom-color border-left-color",
    ),
    (
        "border-width",
        "border-top-width border-right-width border-bottom-width border-left-width",
    ),
    (
        "border-style",
        "border-top-style border-right-style border-bottom-style border-left-style",
    ),
    (
        "border-color",
        "border-top-color border-right-color border-bottom-color border-left-color",
    ),
    (
        "border-top",
        "border-top-width border-top-style border-top-color",
    ),
    (
        "border-right",
        "border-right-width border-right-style border-right-color",
    ),
    (
        "border-bottom",
        "border-bottom-width border-bottom-style border-bottom-color",
    ),
    (
        "border-left",
        "border-left-width border-left-style border-left-color",
    ),
    (
        "border-radius",
        "border-top-left-radius border-top-right-radius border-bottom-right-radius border-bottom-left-radius",
    ),
    (
        "background",
        "background-color background-image background-repeat background-position background-size background-attachment background-origin background-clip",
    ),
    (
        "font",
        "font-style font-variant font-weight font-stretch font-size line-height font-family",
    ),
    (
        "list-style",
        "list-style-type list-style-position list-style-image",
    ),
    ("outline", "outline-width outline-style outline-color"),
    ("overflow", "overflow-x overflow-y"),
    (
        "text-decoration",
        "text-decoration-line text-decoration-style text-decoration-color text-decoration-thickness",
    ),
    (
        "transition",
        "transition-property transition-duration transition-timing-function transition-delay",
    ),
    (
        "animation",
        "animation-name animation-duration animation-timing-function animation-delay animation-iteration-count animation-direction animation-fill-mode animation-play-state",
    ),
    ("flex", "flex-grow flex-shrink flex-basis"),
    ("flex-flow", "flex-direction flex-wrap"),
    ("grid-gap", "grid-row-gap grid-column-gap"),
    ("gap", "row-gap column-gap"),
    ("columns", "column-width column-count"),
    (
        "column-rule",
        "column-rule-width column-rule-style column-rule-color",
    ),
    ("inset", "top right bottom left"),
];

/// A shorthand declared after one of its longhands overrides it.
fn shorthand_overrides(c: &Check, block: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let mut earlier: Vec<String> = Vec::new();
    for decl in declarations(block) {
        let Some((name, at)) = model::property(&decl) else {
            continue;
        };
        let bare = model::unprefixed(&name);
        let prefix = &name[..name.len() - bare.len()];
        if let Some((_, longhands)) = SHORTHANDS.iter().find(|(s, _)| *s == bare)
            && let Some(hit) = earlier.iter().find(|e| {
                longhands
                    .split(' ')
                    .any(|l| e.strip_prefix(prefix) == Some(l))
            })
        {
            c.hit(ctx, at, hit);
        }
        earlier.push(name);
    }
}

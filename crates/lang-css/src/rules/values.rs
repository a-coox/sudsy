//! Checks of the `check` family that read values and selectors in more detail.

use super::checks::Check;
use super::font;
use super::model::{self, is_trivia, plain};
use crate::generated::kind as k;
use core_rules::RuleContext;
use core_rules::tree::significant_range;
use core_syntax::{SyntaxElement, SyntaxNode, SyntaxNodeExt};
use std::collections::HashMap;

/// A later rule that has a lower specificity than an earlier one with the same last compound
/// (without its pseudo-classes) in the same at-rule is reported. Nested rules are left out.
pub fn descending_specificity(c: &Check, sheet: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    type Seen = HashMap<String, Vec<(usize, usize, usize)>>;
    let mut contexts: HashMap<Option<u32>, Seen> = HashMap::new();
    for rule in sheet.descendants().filter(|n| n.kind() == k::RULE) {
        let in_rule = rule.ancestors().skip(1).any(|a| a.kind() == k::RULE);
        let at = rule.ancestors().skip(1).find(|a| a.kind() == k::AT_RULE);
        if in_rule || !has_declaration(&rule) || at.as_ref().is_some_and(model::is_keyframes) {
            continue;
        }
        let Some(prelude) = rule.child(k::PRELUDE) else {
            continue;
        };
        let seen = contexts
            .entry(at.map(|a| a.text_range().start().into()))
            .or_default();
        for sel in model::selectors(&prelude) {
            let key: String = sel.compounds.last().map_or(String::new(), |comp| {
                comp.pieces
                    .iter()
                    .filter(|p| p.part != model::Part::PseudoClass)
                    .map(|p| {
                        &ctx.file.text[usize::from(p.range.start())..usize::from(p.range.end())]
                    })
                    .collect()
            });
            if key.is_empty() {
                continue;
            }
            let spec = sel.specificity();
            let prior = seen.entry(key).or_default();
            if prior.iter().any(|p| spec < *p) {
                c.hit(
                    ctx,
                    significant_range(&prelude, ctx.file.grammar),
                    &sel.text,
                );
            }
            prior.push(spec);
        }
    }
}

fn has_declaration(node: &SyntaxNode) -> bool {
    node.child(k::BLOCK).is_some_and(|b| {
        b.children()
            .any(|c| c.kind() == k::DECLARATION || (c.kind() == k::AT_RULE && has_declaration(&c)))
    })
}

/// Declarations are valid in a rule, in an at-rule that holds them, and below a nesting-capable
/// at-rule only inside a rule.
pub fn declaration_position(c: &Check, decl: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if model::property(decl).is_none() {
        return; // a SCSS or Less variable is valid anywhere
    }
    const NESTING: &[&str] = &[
        "media",
        "supports",
        "container",
        "layer",
        "scope",
        "starting-style",
        "document",
    ];
    for a in decl.ancestors().skip(1) {
        match a.kind() {
            k::RULE => return,
            k::AT_RULE if !NESTING.contains(&model::at_name(&a).as_str()) => return,
            k::STYLESHEET => {
                c.hit(ctx, significant_range(decl, ctx.file.grammar), "");
                return;
            }
            _ => {}
        }
    }
}

/// A custom property that the file declares, written without `var()` in a value.
pub fn missing_var_function(c: &Check, sheet: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    // Properties that take a custom identifier.
    const IGNORED: &[&str] = &[
        "anchor-name",
        "anchor-scope",
        "animation",
        "animation-name",
        "animation-timeline",
        "container-name",
        "counter-increment",
        "counter-reset",
        "counter-set",
        "grid-column",
        "grid-column-end",
        "grid-column-start",
        "grid-row",
        "grid-row-end",
        "grid-row-start",
        "list-style",
        "list-style-type",
        "position-anchor",
        "position-try",
        "position-try-fallbacks",
        "timeline-scope",
        "transition",
        "transition-property",
        "view-transition-name",
        "will-change",
    ];
    let mut known: HashMap<String, ()> = HashMap::new();
    for n in sheet.descendants() {
        match n.kind() {
            k::DECLARATION => {
                if let Some(p) = n
                    .child(k::PROPERTY)
                    .map(|p| plain(&p))
                    .filter(|p| model::is_custom(p))
                {
                    known.insert(p, ());
                }
            }
            k::AT_RULE if model::at_name(&n) == "property" => {
                if let Some(p) = n.child(k::PRELUDE) {
                    known.insert(plain(&p), ());
                }
            }
            _ => {}
        }
    }
    for decl in sheet.descendants().filter(|n| n.kind() == k::DECLARATION) {
        let Some(value) = model::value(&decl) else {
            continue;
        };
        if model::property(&decl).is_some_and(|(p, _)| IGNORED.contains(&p.as_str())) {
            continue;
        }
        for t in model::tokens(&value) {
            let inside_var = t.parent_ancestors().take_while(|a| *a != value).any(|a| {
                a.kind() == k::FUNCTION
                    && model::function_name(&a)
                        .is_some_and(|n| n.text().eq_ignore_ascii_case("var"))
            });
            if t.kind() == k::IDENT && !inside_var && known.contains_key(t.text()) {
                c.hit(ctx, t.text_range(), t.text());
            }
        }
    }
}

/// The areas of a `grid-template-areas` (or `grid-template`, `grid`) must be rectangles over rows
/// of the same width.
pub fn grid_areas(c: &Check, decl: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let grid = model::property(decl).is_some_and(|(p, _)| {
        matches!(p.as_str(), "grid-template-areas" | "grid-template" | "grid")
    });
    let Some(value) = model::value(decl).filter(|_| grid) else {
        return;
    };
    let rows: Vec<_> = value.tokens().filter(|t| t.kind() == k::STRING).collect();
    let cells: Vec<Vec<&str>> = rows
        .iter()
        .map(|t| t.text()[1..t.text().len() - 1].split_whitespace().collect())
        .collect();
    for (i, row) in cells.iter().enumerate() {
        if row.is_empty() || row.len() != cells[0].len() {
            c.hit(ctx, rows[i].text_range(), "");
            return;
        }
    }
    let mut names: Vec<&str> = cells
        .iter()
        .flatten()
        .copied()
        .filter(|n| !n.starts_with('.'))
        .collect();
    names.sort_unstable();
    names.dedup();
    for name in names {
        let at: Vec<(usize, usize)> = cells
            .iter()
            .enumerate()
            .flat_map(|(r, row)| {
                row.iter()
                    .enumerate()
                    .filter(move |(_, n)| **n == name)
                    .map(move |(col, _)| (r, col))
            })
            .collect();
        let span = |f: fn(&(usize, usize)) -> usize| {
            let (lo, hi) = (
                at.iter().map(f).min().unwrap(),
                at.iter().map(f).max().unwrap(),
            );
            hi - lo + 1
        };
        if at.len() != span(|p| p.0) * span(|p| p.1) {
            c.hit(ctx, rows[0].text_range(), name);
            return;
        }
    }
}

/// `#aabbcc` that can be `#abc` (or the other way round with style `long`).
pub fn hex_length(c: &Check, value: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let long = ctx.options.string("style") == "long";
    for t in model::tokens(value).filter(|t| t.kind() == k::HASH) {
        let hex = &t.text()[1..];
        if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            continue;
        }
        let shortable = matches!(hex.len(), 6 | 8)
            && hex
                .as_bytes()
                .chunks(2)
                .all(|p| p[0].eq_ignore_ascii_case(&p[1]));
        if (long && matches!(hex.len(), 3 | 4)) || (!long && shortable) {
            c.hit(ctx, t.text_range(), t.text());
        }
    }
}

/// `from` and `to` are written `0%` and `100%` unless the block has nothing but keywords.
pub fn keyframe_selector_notation(c: &Check, at: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if !model::is_keyframes(at) {
        return;
    }
    let selectors: Vec<_> = model::items(at)
        .filter(|n| n.kind() == k::RULE)
        .filter_map(|r| r.child(k::PRELUDE))
        .flat_map(|p| model::selectors(&p))
        .collect();
    let keyword =
        |s: &model::Complex| matches!(s.text.to_ascii_lowercase().as_str(), "from" | "to");
    if selectors.iter().all(keyword) {
        return;
    }
    for s in selectors.iter().filter(|s| keyword(s)) {
        if let Some(range) = s.range {
            c.hit(ctx, range, &s.text);
        }
    }
}

/// A shorthand of one to four sides that repeats what the shorter form implies.
pub fn redundant_values(c: &Check, decl: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some((name, _)) = model::property(decl) else {
        return;
    };
    let sides = matches!(
        model::unprefixed(&name),
        "margin" | "padding" | "border-color" | "border-style" | "border-width" | "border-radius"
    );
    let Some(value) = model::value(decl).filter(|_| sides) else {
        return;
    };
    let mut words: Vec<String> = Vec::new();
    for e in value.children_with_tokens() {
        match e {
            SyntaxElement::Token(t) if is_trivia(t.kind()) => {}
            SyntaxElement::Token(t) if t.text() == "!" => break,
            SyntaxElement::Token(t) if t.text() == "/" => return,
            SyntaxElement::Token(t) => words.push(t.text().to_ascii_lowercase()),
            SyntaxElement::Node(n) => words.push(plain(&n).to_ascii_lowercase()),
        }
    }
    let redundant = match words.len() {
        4 => words[1] == words[3],
        3 => words[0] == words[2],
        2 => words[0] == words[1],
        _ => false,
    };
    if redundant && let Some(v) = value.first_token() {
        c.hit(ctx, v.text_range(), &words.join(" "));
    }
}

/// Quotes around a family name that is valid without them.
pub fn font_family_quotes(c: &Check, decl: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(families) = font::of(decl) else {
        return;
    };
    let ident = |w: &str| {
        let w = w.strip_prefix('-').unwrap_or(w);
        w.chars()
            .next()
            .is_some_and(|c| c.is_alphabetic() || c == '_' || !c.is_ascii())
            && w.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || !c.is_ascii())
    };
    for f in families.iter().filter(|f| f.quoted && !f.name.is_empty()) {
        if f.name.split(' ').all(ident) {
            c.hit(ctx, f.range, &f.name);
        }
    }
}

/// `:nth-child(0)`, `0n`, `-n` and the like match nothing.
pub fn nth_unmatchable(c: &Check, f: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(name) = model::function_name(f) else {
        return;
    };
    let name = name.text().to_ascii_lowercase();
    let nth = matches!(
        name.as_str(),
        "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type"
    );
    let Some(group) = f.child(k::GROUP).filter(|_| nth) else {
        return;
    };
    let text = plain(&group).to_ascii_lowercase();
    let body = text[1..text.len() - 1]
        .split("of")
        .next()
        .unwrap_or("")
        .to_string();
    let parsed = match body.split_once('n') {
        Some((a, b)) => {
            let a = match a {
                "" | "+" => Some(1),
                "-" => Some(-1),
                n => n.parse::<i64>().ok(),
            };
            let b = if b.is_empty() {
                Some(0)
            } else {
                b.parse::<i64>().ok()
            };
            a.zip(b)
        }
        None => body.parse::<i64>().ok().map(|b| (0, b)),
    };
    if parsed.is_some_and(|(a, b)| a <= 0 && b <= 0) {
        c.hit(ctx, f.text_range(), &body);
    }
}

/// A type selector in the same compound as a class, id or attribute selector (`a.b`).
pub fn qualifying_type(c: &Check, rule: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let Some(prelude) = rule
        .child(k::PRELUDE)
        .filter(|_| !model::in_keyframes(rule))
    else {
        return;
    };
    for sel in model::selectors(&prelude) {
        for comp in &sel.compounds {
            let qualified = comp.pieces.iter().any(|p| {
                matches!(
                    p.part,
                    model::Part::Class | model::Part::Id | model::Part::Attribute
                )
            });
            let ty = comp.pieces.iter().find(|p| p.part == model::Part::Type);
            if let (true, Some(t)) = (qualified, ty) {
                c.hit(ctx, t.range, &t.name);
            }
        }
    }
}

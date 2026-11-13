//! The `table` family: names looked up in the known-name table (`data.rs`). Params:
//! - `target`: what is checked, see [`Target`];
//! - `mode`: `unknown` (the name is not in the table `data`), `listed` (it is) and `prefixed` (a
//!   vendor-prefixed name whose unprefixed name is);
//! - `data`: the table section.
//!
//! The message has `{name}`; the option `ignore` lists names or `/regex/` that are left out.
//! Names are compared in lowercase. SCSS and Less constructs are not unknown: their at-rules are
//! in the tables `at-rule-scss` and `at-rule-less`, and their functions are not checked.

use super::model::{self, Complex, Part, Piece, split_dimension};
use super::names::selects;
use crate::data;
use crate::generated::kind as k;
use core_rules::{Family, Params, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxNodeExt, TextRange};

pub const FAMILY: Family = Family {
    name: "table",
    build,
};

#[derive(Clone, Copy, PartialEq)]
enum Target {
    Property,
    AtRule,
    Unit,
    Function,
    MediaFeature,
    MediaType,
    /// The keywords and functions of a declaration's value.
    Value,
    /// `:name`, `::name` and type selectors, as written in a selector.
    Selector,
    PseudoClass,
    PseudoElement,
    Type,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Unknown,
    Listed,
    Prefixed,
}

struct Table {
    meta: &'static RuleMeta,
    target: Target,
    mode: Mode,
    data: String,
    message: String,
    kinds: &'static [SyntaxKind],
}

fn build(meta: &'static RuleMeta, mut p: Params) -> Result<Box<dyn Rule>, String> {
    use Target::*;
    let target = p.choice(
        "target",
        true,
        &[
            ("property", Property),
            ("at-rule", AtRule),
            ("unit", Unit),
            ("function", Function),
            ("media-feature", MediaFeature),
            ("media-type", MediaType),
            ("value", Value),
            ("selector", Selector),
            ("pseudo-class", PseudoClass),
            ("pseudo-element", PseudoElement),
            ("type", Type),
        ],
    )?;
    let mode = p.choice(
        "mode",
        true,
        &[
            ("unknown", Mode::Unknown),
            ("listed", Mode::Listed),
            ("prefixed", Mode::Prefixed),
        ],
    )?;
    let kinds: &'static [SyntaxKind] = match target {
        Property | Value => &[k::DECLARATION],
        AtRule | MediaFeature | MediaType => &[k::AT_RULE],
        Unit => &[k::VALUE, k::PRELUDE],
        Function => &[k::FUNCTION],
        Selector | PseudoClass | PseudoElement | Type => &[k::RULE],
    };
    let rule = Table {
        meta,
        target,
        mode,
        data: p.text("data")?,
        message: p.text("message")?,
        kinds,
    };
    p.done()?;
    Ok(Box::new(rule))
}

/// The at-rules whose declarations are descriptors, not properties.
const DESCRIPTOR_AT_RULES: &[&str] = &[
    "font-face",
    "counter-style",
    "property",
    "font-palette-values",
    "font-feature-values",
    "position-try",
    "view-transition",
];

/// Pseudo-classes and elements whose arguments are not selectors.
fn opaque(piece: &Piece) -> bool {
    let name = piece.name.to_ascii_lowercase();
    !matches!(piece.part, Part::PseudoClass | Part::PseudoElement)
        || matches!(
            name.as_str(),
            "lang" | "dir" | "state" | "part" | "highlight" | "active-view-transition-type"
        )
        || name.starts_with("nth-")
        || name.starts_with("view-transition-")
}

pub(super) fn dialect(ctx: &RuleContext<'_>) -> Option<&'static str> {
    match ctx.file.path.extension()?.to_str()? {
        "scss" => Some("scss"),
        "less" => Some("less"),
        _ => None,
    }
}

impl Table {
    /// The candidate names in `node` with where each is.
    fn collect(&self, node: &SyntaxNode, ctx: &RuleContext<'_>) -> Vec<(String, TextRange)> {
        let unknown = self.mode == Mode::Unknown;
        let mut out = Vec::new();
        match self.target {
            Target::Property => {
                if let Some((name, at)) = model::property(node)
                    && !model::is_custom(&name)
                    && !(unknown
                        && (name.starts_with('-')
                            || name.contains(['{', '$', '@', '&'])
                            || is_descriptor(node)))
                {
                    out.push((name, at));
                }
            }
            Target::AtRule => {
                if let Some(t) = node.token(k::AT_KEYWORD) {
                    out.push((model::at_name(node), t.text_range()));
                }
            }
            Target::Unit => self.units(node, &mut out),
            Target::Function => {
                let in_value = node.ancestors().any(|a| a.kind() == k::VALUE);
                if let (true, None, Some(t)) = (in_value, dialect(ctx), model::function_name(node))
                    && !t.text().starts_with("--")
                    && !super::names::after_colon(node)
                {
                    out.push((t.text().to_ascii_lowercase(), t.text_range()));
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
                            && !(unknown && t.text().starts_with('-'))
                            // `(iv.$tablet - 1px)` is an expression, not a feature
                            && model::tokens(&g).nth(2).is_none_or(|n| {
                                matches!(n.text(), ":" | ")" | "<" | ">" | "=")
                            })
                        {
                            out.push((t.text().to_ascii_lowercase(), t.text_range()));
                        }
                    }
                }
            }
            Target::MediaType => {
                if model::at_name(node) == "media"
                    && let Some(prelude) = node.child(k::PRELUDE)
                {
                    let tokens: Vec<_> = model::tokens(&prelude).collect();
                    for query in tokens.split(|t| t.kind() == k::COMMA) {
                        let first = query.iter().find(|t| {
                            !matches!(t.text().to_ascii_lowercase().as_str(), "not" | "only")
                        });
                        if let Some(t) = first
                            && t.kind() == k::IDENT
                        {
                            out.push((t.text().to_ascii_lowercase(), t.text_range()));
                        }
                    }
                }
            }
            Target::Value => {
                if model::property(node).is_some()
                    && let Some(value) = model::value(node)
                {
                    for t in model::tokens(&value).filter(|t| t.kind() == k::IDENT) {
                        out.push((t.text().to_ascii_lowercase(), t.text_range()));
                    }
                }
            }
            Target::Selector | Target::PseudoClass | Target::PseudoElement | Target::Type => {
                self.selector_names(node, ctx, &mut out)
            }
        }
        out
    }

    fn units(&self, node: &SyntaxNode, out: &mut Vec<(String, TextRange)>) {
        let parent = node.parent();
        let in_media = parent
            .as_ref()
            .is_some_and(|p| p.kind() == k::AT_RULE && model::at_name(p) == "media");
        if node.kind() == k::PRELUDE && !in_media {
            return;
        }
        let declaration = parent.filter(|p| p.kind() == k::DECLARATION);
        let context = declaration.as_ref().unwrap_or(node).text().to_string();
        let context = context.to_ascii_lowercase();
        if context.starts_with("unicode-range") {
            return;
        }
        // shortcut: `x` is a resolution only in `image-set()`, `image-resolution` and
        // `resolution` media features, read from the text around it, not from its position.
        let resolution = ["image-set", "resolution"]
            .iter()
            .any(|w| context.contains(w));
        for t in model::tokens(node).filter(|t| t.kind() == k::DIMENSION) {
            let unit = split_dimension(t.text()).1.to_ascii_lowercase();
            if unit != "x" || !resolution {
                out.push((unit, t.text_range()));
            }
        }
    }

    fn selector_names(
        &self,
        node: &SyntaxNode,
        ctx: &RuleContext<'_>,
        out: &mut Vec<(String, TextRange)>,
    ) {
        let Some(prelude) = node.child(k::PRELUDE) else {
            return;
        };
        let text = model::plain(&prelude);
        let dialect = dialect(ctx);
        if self.mode == Mode::Unknown
            && (text.contains("#{")
                || text.contains("@{")
                || text.contains("//")
                || dialect.is_some()
                    && (text.contains(['%', '$'])
                        || model::tokens(&prelude).any(|t| t.text() == "when")))
        {
            return;
        }
        for complex in model::selectors(&prelude) {
            self.complex_names(&complex, model::in_keyframes(node), out);
        }
    }

    fn complex_names(
        &self,
        complex: &Complex,
        keyframes: bool,
        out: &mut Vec<(String, TextRange)>,
    ) {
        let unknown = self.mode == Mode::Unknown;
        let pieces: Vec<&Piece> = complex.pieces().collect();
        let scrollbar = pieces
            .iter()
            .any(|p| p.part == Part::PseudoElement && p.name.starts_with("-webkit-scrollbar"));
        // Mixin definitions and calls of Less and SCSS have arguments that are not selectors.
        let inside = |p: &Piece| {
            pieces.iter().any(|f| {
                f.range.start() < p.range.start() && p.range.end() <= f.range.end() && opaque(f)
            })
        };
        for (i, p) in pieces.iter().enumerate() {
            let name = p.name.to_ascii_lowercase();
            let keep = match (self.target, p.part) {
                (Target::PseudoClass, Part::PseudoClass) => {
                    let legacy = matches!(
                        name.as_str(),
                        "before" | "after" | "first-line" | "first-letter"
                    );
                    !name.starts_with('-')
                        && !legacy
                        && !(scrollbar && data::has("scrollbar-pseudo-class", &name))
                }
                (Target::PseudoElement, Part::PseudoElement) => !name.starts_with('-'),
                (Target::Type, Part::Type) => {
                    let after_nesting = i > 0
                        && pieces[i - 1].part == Part::Nesting
                        && pieces[i - 1].range.end() == p.range.start();
                    let frame = keyframes && matches!(p.name.as_str(), "from" | "to");
                    name.starts_with(|c: char| c.is_ascii_alphabetic()) && !after_nesting && !frame
                }
                (Target::Selector, Part::PseudoClass | Part::PseudoElement | Part::Type) => true,
                _ => false,
            };
            if !keep || (unknown && inside(p)) {
                continue;
            }
            let shown = match (self.target, p.part) {
                (Target::Selector, Part::PseudoClass) => format!(":{name}"),
                (Target::Selector, Part::PseudoElement) => format!("::{name}"),
                _ => name,
            };
            out.push((shown, p.range));
        }
    }

    fn known(&self, name: &str, ctx: &RuleContext<'_>) -> bool {
        if data::has(&self.data, name) {
            return true;
        }
        match self.target {
            Target::AtRule => {
                // shortcut: Less at-rules (detached ruleset calls, `@plugin`) are not told apart.
                name.starts_with('-')
                    || match dialect(ctx) {
                        Some("scss") => data::has("at-rule-scss", name),
                        Some(_) => true,
                        None => false,
                    }
            }
            Target::Unit => name == "%",
            Target::MediaFeature => name.starts_with('-'),
            _ => false,
        }
    }
}

fn is_descriptor(decl: &SyntaxNode) -> bool {
    decl.ancestors()
        .find(|a| a.kind() == k::AT_RULE)
        .is_some_and(|a| DESCRIPTOR_AT_RULES.contains(&model::at_name(&a).as_str()))
}

impl Rule for Table {
    fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    fn kinds(&self) -> &[SyntaxKind] {
        self.kinds
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let ignore = ctx.options.list("ignore");
        for (name, range) in self.collect(node, ctx) {
            let hit = match self.mode {
                Mode::Unknown => !self.known(&name, ctx),
                Mode::Listed => data::has(&self.data, &name),
                Mode::Prefixed => {
                    name.starts_with('-')
                        && !model::is_custom(&name)
                        && data::has(&self.data, model::unprefixed(&name))
                }
            };
            if hit && !ignore.iter().any(|e| selects(e, &name, true)) {
                ctx.report(range, self.message.replace("{name}", &name));
            }
        }
    }
}

//! Opt-in rules on plain values: `no-empty-values`, `float-style` and `no-octal-values`.

use crate::generated::{BlockMappingEntry, BlockSequenceEntry, FlowPair, Scalar, kind};
use crate::rules::has_tag;
use core_config::Level;
use core_rules::{
    Category, Check, Edit, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta,
};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextSize};
use regex::Regex;
use std::sync::LazyLock;

pub struct NoEmptyValues;
pub struct FloatStyle;
pub struct NoOctalValues;

const fn flag(name: &'static str, default: bool, description: &'static str) -> OptionDecl {
    OptionDecl {
        name,
        default: Opt::Bool(default),
        check: Check::Any,
        description,
    }
}

static NO_EMPTY_VALUES: RuleMeta = RuleMeta {
    id: "no-empty-values",
    category: Category::Style,
    description: "A mapping key or sequence entry without a value (an implicit null). Values \
                  with an anchor or tag are not empty. Reported at the `:` or `-`. yamllint \
                  `empty-values`.",
    example: "key:      # reported; write `key: null`\nlist:\n  -       # reported",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[
        flag("block_mappings", true, "Check block mappings."),
        flag("flow_mappings", true, "Check flow mappings (`{a: }`)."),
        flag("block_sequences", true, "Check block sequences."),
    ],
};

impl Rule for NoEmptyValues {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_VALUES
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[
            kind::BLOCK_MAPPING_ENTRY,
            kind::FLOW_PAIR,
            kind::BLOCK_SEQUENCE_ENTRY,
        ]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (marker, empty, option, what) = match node.kind() {
            kind::BLOCK_MAPPING_ENTRY => (
                kind::COLON,
                BlockMappingEntry::cast(node.clone()).is_some_and(|e| e.value().is_none()),
                "block_mappings",
                "block mapping",
            ),
            kind::FLOW_PAIR => (
                kind::COLON,
                FlowPair::cast(node.clone()).is_some_and(|e| e.value().is_none()),
                "flow_mappings",
                "flow mapping",
            ),
            _ => (
                kind::DASH,
                BlockSequenceEntry::cast(node.clone()).is_some_and(|e| e.value().is_none()),
                "block_sequences",
                "block sequence",
            ),
        };
        if !empty || !ctx.options.bool(option) {
            return;
        }
        if let Some(token) = node
            .children_with_tokens()
            .filter_map(|c| c.into_token())
            .find(|t| t.kind() == marker)
        {
            ctx.report(token.text_range(), format!("empty value in {what}"));
        }
    }
}

/// The plain, untagged scalar of `node` (not a quoted one, not a key's alias).
fn plain_untagged(node: &SyntaxNode) -> Option<SyntaxToken> {
    let token = Scalar::cast(node.clone())?.value()?;
    (token.kind() == kind::PLAIN && !has_tag(node)).then_some(token)
}

fn pattern(source: &str) -> Regex {
    Regex::new(source).expect("the patterns are valid")
}

static NUMERAL_BEFORE_DECIMAL: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^[-+]?\.[0-9]+([eE][-+]?[0-9]+)?$"));
static SCIENTIFIC: LazyLock<Regex> =
    LazyLock::new(|| pattern(r"^[-+]?(\.[0-9]+|[0-9]+(\.[0-9]*)?)[eE][-+]?[0-9]+$"));
static INF: LazyLock<Regex> = LazyLock::new(|| pattern(r"^[-+]?\.(inf|Inf|INF)$"));
static NAN: LazyLock<Regex> = LazyLock::new(|| pattern(r"^\.(nan|NaN|NAN)$"));

static FLOAT_STYLE: RuleMeta = RuleMeta {
    id: "float-style",
    category: Category::Style,
    description: "Restricts how plain floats are written. Every option is off until set: \
                  `.5` without a leading digit, `1e3` scientific notation, `.nan` and `.inf`. \
                  Quoted and tagged scalars are not checked. The fix for `.5` is `0.5`. yamllint \
                  `float-values`.",
    example: "x: .5   # with require_numeral_before_decimal; becomes 0.5",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[
        flag(
            "require_numeral_before_decimal",
            false,
            "Report `.5`, `-.5e3`: a digit must come before the decimal point.",
        ),
        flag(
            "forbid_scientific_notation",
            false,
            "Report `1e3`, `1.5E-2`, `.5e3`.",
        ),
        flag("forbid_nan", false, "Report `.nan`, `.NaN`, `.NAN`."),
        flag("forbid_inf", false, "Report `.inf`, `-.Inf`, `+.INF`."),
    ],
};

impl Rule for FloatStyle {
    fn meta(&self) -> &'static RuleMeta {
        &FLOAT_STYLE
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::SCALAR]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(token) = plain_untagged(node) else {
            return;
        };
        let (text, range) = (token.text(), token.text_range());
        let o = ctx.options;
        if o.bool("forbid_nan") && NAN.is_match(text) {
            ctx.report(range, format!("forbidden not a number value `{text}`"));
        }
        if o.bool("forbid_inf") && INF.is_match(text) {
            ctx.report(range, format!("forbidden infinite value `{text}`"));
        }
        if o.bool("forbid_scientific_notation") && SCIENTIFIC.is_match(text) {
            ctx.report(range, format!("forbidden scientific notation `{text}`"));
        }
        if o.bool("require_numeral_before_decimal") && NUMERAL_BEFORE_DECIMAL.is_match(text) {
            let at = range.start() + TextSize::new(u32::from(text.starts_with(['-', '+'])));
            ctx.report(
                range,
                format!("decimal `{text}` is missing the 0 before the point"),
            )
            .fix(Fix::new(
                "Add the 0",
                FixKind::Safe,
                vec![Edit::insert(at, "0")],
            ));
        }
    }
}

static NO_OCTAL_VALUES: RuleMeta = RuleMeta {
    id: "no-octal-values",
    category: Category::Suspicious,
    description: "A plain `0755` (octal to YAML 1.1 parsers, decimal 755 to YAML 1.2) or `0o755` \
                  (octal to YAML 1.2, a string to YAML 1.1). Quoted and tagged scalars are not \
                  checked. No fix: the intended value is not known. yamllint `octal-values`.",
    example: "mode: 0755   # quote it (a string) or write the decimal value",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[
        flag("implicit", true, "Report `0755`."),
        flag("explicit", true, "Report `0o755`."),
    ],
};

fn octal_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| (b'0'..=b'7').contains(&b))
}

impl Rule for NoOctalValues {
    fn meta(&self) -> &'static RuleMeta {
        &NO_OCTAL_VALUES
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::SCALAR]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(token) = plain_untagged(node) else {
            return;
        };
        let text = token.text();
        let message = if ctx.options.bool("implicit")
            && text.len() > 1
            && text.starts_with('0')
            && octal_digits(&text[1..])
        {
            "implicit"
        } else if ctx.options.bool("explicit") && text.starts_with("0o") && octal_digits(&text[2..])
        {
            "explicit"
        } else {
            return;
        };
        ctx.report(
            token.text_range(),
            format!("forbidden {message} octal value `{text}`"),
        );
    }
}

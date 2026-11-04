//! YAML rules: `ambiguous-booleans` and `document-start`. Anchor rules are in `anchor_rules`,
//! rules on values in `value_rules` and `quoted_strings`.

use crate::generated::{BlockMappingEntry, FlowPair, Scalar, kind};
use crate::{anchor_rules, quoted_strings, value_rules};
use core_config::Level;
use core_rules::tree::significant_range;
use core_rules::{
    Category, Check, Edit, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta,
};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, TextSize};

pub static RULES: &[&dyn Rule] = &[
    &anchor_rules::NoUndefinedAlias,
    &anchor_rules::NoDuplicateAnchors,
    &anchor_rules::NoUnusedAnchors,
    &value_rules::NoEmptyValues,
    &value_rules::FloatStyle,
    &value_rules::NoOctalValues,
    &quoted_strings::QuotedStrings,
    &AmbiguousBooleans,
    &DocumentStart,
];

struct AmbiguousBooleans;

static AMBIGUOUS_BOOLEANS: RuleMeta = RuleMeta {
    id: "ambiguous-booleans",
    category: Category::Suspicious,
    description: "A plain `yes`, `no`, `on`, `off`, `True`, `TRUE`, `False` or `FALSE` (in the usual \
                  casings): a boolean to YAML 1.1 parsers and a string to YAML 1.2 parsers. \
                  Lowercase `true` and `false` are fine.",
    example: "enabled: yes   # becomes enabled: \"yes\"",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[
        OptionDecl {
            name: "check_keys",
            default: Opt::Bool(true),
            check: Check::Any,
            description: "Also check mapping keys. Turn it off to keep GitHub workflow `on:`.",
        },
        OptionDecl {
            name: "allowed",
            default: Opt::List(&[]),
            check: Check::Any,
            description: "Spellings that are not reported, for example `[\"on\"]`.",
        },
    ],
};

const AMBIGUOUS: [&str; 16] = [
    "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off", "OFF", "True", "TRUE",
    "False", "FALSE",
];

impl Rule for AmbiguousBooleans {
    fn meta(&self) -> &'static RuleMeta {
        &AMBIGUOUS_BOOLEANS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::SCALAR]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(token) = Scalar::cast(node.clone()).and_then(|s| s.value()) else {
            return;
        };
        if token.kind() != kind::PLAIN
            || !AMBIGUOUS.contains(&token.text())
            || ctx.options.list("allowed").contains(&token.text())
        {
            return;
        }
        if !ctx.options.bool("check_keys") && is_key(node) {
            return;
        }
        // `!!str yes` says what it means.
        if has_tag(node) {
            return;
        }
        let text = token.text();
        ctx.report(
            token.text_range(),
            format!("`{text}` is a boolean in YAML 1.1 and a string in YAML 1.2"),
        )
        .fix(Fix::new(
            format!("Quote it: \"{text}\""),
            FixKind::Unsafe,
            vec![Edit::replace(token.text_range(), format!("\"{text}\""))],
        ));
    }
}

/// Is a tag among the properties of the scalar?
pub(crate) fn has_tag(scalar: &SyntaxNode) -> bool {
    scalar.parent().is_some_and(|p| {
        p.kind() == kind::PROPERTY_NODE && p.children_with_tokens().any(|c| c.kind() == kind::TAG)
    })
}

/// Is the scalar (possibly inside a property node) the key of a mapping entry?
pub(crate) fn is_key(scalar: &SyntaxNode) -> bool {
    let mut value = scalar.clone();
    if let Some(p) = value.parent().filter(|p| p.kind() == kind::PROPERTY_NODE) {
        value = p;
    }
    let Some(entry) = value.parent() else {
        return false;
    };
    let key = match entry.kind() {
        kind::BLOCK_MAPPING_ENTRY => BlockMappingEntry::cast(entry).and_then(|e| e.key()),
        kind::FLOW_PAIR => FlowPair::cast(entry).and_then(|e| e.key()),
        _ => None,
    };
    key.is_some_and(|k| *k.syntax() == value)
}

struct DocumentStart;

static DOCUMENT_START: RuleMeta = RuleMeta {
    id: "document-start",
    category: Category::Style,
    description: "The first document does not start with `---`.",
    example: "a: 1   # becomes ---\\na: 1",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for DocumentStart {
    fn meta(&self) -> &'static RuleMeta {
        &DOCUMENT_START
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::STREAM]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(document) = node.children().find(|c| c.kind() == kind::DOCUMENT) else {
            return;
        };
        let has = |k: SyntaxKind| document.children_with_tokens().any(|c| c.kind() == k);
        // A document with directives can only start with `---` already.
        if has(kind::DOC_START) || has(kind::DIRECTIVE) {
            return;
        }
        let Some(body) = document.children().next() else {
            return;
        };
        ctx.report(
            significant_range(&body, ctx.file.grammar),
            "the document does not start with `---`",
        )
        .fix(Fix::new(
            "Add `---`",
            FixKind::Safe,
            vec![Edit::insert(TextSize::from(0), "---\n")],
        ));
    }
}

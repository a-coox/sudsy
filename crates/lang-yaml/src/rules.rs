//! YAML rules: `no-duplicate-keys`, `ambiguous-booleans`, `document-start` and the opt-in
//! `sorted-keys`. Anchor rules are in `anchor_rules`, rules on values in `value_rules` and
//! `quoted_strings`.

use crate::generated::{
    AnyValue, BlockMapping, BlockMappingEntry, FlowMapping, FlowPair, Scalar, kind,
};
use crate::scalar::{double_quoted_value, plain_value, single_quoted_value};
use crate::{anchor_rules, quoted_strings, value_rules};
use core_config::Level;
use core_rules::sorting::{self, Sorting};
use core_rules::tree::{entry_range, reorder_edits, significant_range};
use core_rules::{
    Category, Check, Edit, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta,
};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};
use std::collections::HashMap;

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicateKeys,
    &anchor_rules::NoUndefinedAlias,
    &anchor_rules::NoDuplicateAnchors,
    &anchor_rules::NoUnusedAnchors,
    &value_rules::NoEmptyValues,
    &value_rules::FloatStyle,
    &value_rules::NoOctalValues,
    &quoted_strings::QuotedStrings,
    &AmbiguousBooleans,
    &DocumentStart,
    &SortedKeys,
];

/// What identifies a key: the value of a scalar, else the text of the node without comments.
fn key_identity(key: &AnyValue, ctx: &RuleContext<'_>) -> String {
    if let Some(scalar) = Scalar::cast(key.syntax().clone())
        && let Some(token) = scalar.value()
    {
        let text = token.text();
        match token.kind() {
            kind::PLAIN => return plain_value(text),
            kind::SINGLE_QUOTED => return single_quoted_value(text),
            kind::DOUBLE_QUOTED => {
                if let Some(v) = double_quoted_value(text) {
                    return v;
                }
            }
            _ => {}
        }
    }
    let range = significant_range(key.syntax(), ctx.file.grammar);
    ctx.file.text[usize::from(range.start())..usize::from(range.end())].to_string()
}

struct NoDuplicateKeys;

static NO_DUPLICATE_KEYS: RuleMeta = RuleMeta {
    id: "no-duplicate-keys",
    category: Category::Correctness,
    description: "A mapping has the same key twice; parsers disagree on which value wins.",
    example: "a: 1\na: 2   # reported at the second `a`",
    default_level: Level::Error,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "merge_keys",
        default: Opt::Str("allow"),
        check: Check::OneOf(&["allow", "forbid"]),
        description: "`allow` skips `<<` merge keys; `forbid` reports a second `<<` in a mapping like any other duplicate key.",
    }],
};

impl Rule for NoDuplicateKeys {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DUPLICATE_KEYS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::BLOCK_MAPPING, kind::FLOW_MAPPING]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let keys: Vec<AnyValue> = if let Some(m) = BlockMapping::cast(node.clone()) {
            m.entries().filter_map(|e| e.key()).collect()
        } else if let Some(m) = FlowMapping::cast(node.clone()) {
            m.entries().filter_map(|e| e.key()).collect()
        } else {
            return;
        };
        let forbid_merge_keys = ctx.options.string("merge_keys") == "forbid";
        let mut seen: HashMap<String, TextRange> = HashMap::new();
        for key in &keys {
            let name = key_identity(key, ctx);
            // `<<` merges mappings and may appear more than once in practice.
            if name == "<<" && !forbid_merge_keys {
                continue;
            }
            let range = significant_range(key.syntax(), ctx.file.grammar);
            match seen.get(&name) {
                Some(&first) => {
                    ctx.report(range, format!("duplicate key `{name}`"))
                        .related(first, "first defined here");
                }
                None => {
                    seen.insert(name, range);
                }
            }
        }
    }
}

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

struct SortedKeys;

static SORTED_KEYS: RuleMeta = RuleMeta {
    id: "sorted-keys",
    category: Category::Style,
    description: "The keys of a block mapping are not in alphabetical order. Comments on lines of \
                  their own stay where they are, a comment after an entry moves with it; moving \
                  an anchor below its alias breaks the document, so the fix is unsafe.",
    example: "b: 1\na: 2   # becomes a: 2\\nb: 1",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[
        sorting::ORDER,
        sorting::DIRECTION,
        sorting::COMPARE,
        sorting::IGNORED_KEYS,
        sorting::MIN_KEYS,
        sorting::BLANK_LINE_GROUPS,
    ],
};

impl Rule for SortedKeys {
    fn meta(&self) -> &'static RuleMeta {
        &SORTED_KEYS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::BLOCK_MAPPING]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(mapping) = BlockMapping::cast(node.clone()) else {
            return;
        };
        let entries: Vec<(BlockMappingEntry, AnyValue)> = mapping
            .entries()
            .filter_map(|e| e.key().map(|k| (e, k)))
            .collect();
        let names: Vec<String> = entries.iter().map(|(_, k)| key_identity(k, ctx)).collect();
        let range = |i: usize| entry_range(entries[i].0.syntax(), ctx.file.grammar);
        let text = ctx.file.text;
        let sorting = Sorting::new(ctx.options, "min_keys");
        let blank = |i: usize| {
            sorting::has_blank_line(
                &text[usize::from(range(i - 1).end())..usize::from(range(i).start())],
            )
        };
        let Some(plan) = sorting.plan_keys(&names, blank) else {
            return;
        };
        let ranges: Vec<TextRange> = (0..entries.len()).map(range).collect();
        let edits = reorder_edits(ctx.file.text, &ranges, &plan.sorted);
        // Every key that sorts before the one above it is reported, like yamllint; the first
        // report carries the fix for the whole mapping.
        let mut fix = Some(Fix::new(
            "Sort the keys of this mapping",
            FixKind::Unsafe,
            edits,
        ));
        for i in plan.misplaced {
            let at = significant_range(entries[i].1.syntax(), ctx.file.grammar);
            let report = ctx.report(at, "keys are not sorted");
            if let Some(f) = fix.take() {
                report.fix(f);
            }
        }
    }
}

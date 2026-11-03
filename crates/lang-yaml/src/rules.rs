//! YAML rules (plan.md "Initial built-in rules"): `no-duplicate-keys`, `ambiguous-booleans`,
//! `document-start` and the opt-in `sorted-keys`.

use crate::generated::{AnyValue, BlockMapping, BlockMappingEntry, FlowMapping, Scalar, kind};
use crate::scalar::{double_quoted_value, plain_value, single_quoted_value};
use core_config::Level;
use core_rules::tree::{entry_range, reorder_edits, significant_range};
use core_rules::{Category, Edit, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, TextRange, TextSize};
use std::collections::HashMap;

pub static RULES: &[&dyn Rule] = &[
    &NoDuplicateKeys,
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
    options: &[],
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
        let mut seen: HashMap<String, TextRange> = HashMap::new();
        for key in &keys {
            let name = key_identity(key, ctx);
            // `<<` merges mappings and may appear more than once in practice.
            if name == "<<" {
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
    description: "A plain `yes`, `no`, `on` or `off`: a boolean to YAML 1.1 parsers and a string to \
                  YAML 1.2 parsers.",
    example: "enabled: yes   # becomes enabled: \"yes\"",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[],
};

const AMBIGUOUS: [&str; 12] = [
    "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off", "OFF",
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
        if token.kind() != kind::PLAIN || !AMBIGUOUS.contains(&token.text()) {
            return;
        }
        // `!!str yes` says what it means.
        let tagged = node.parent().is_some_and(|p| {
            p.kind() == kind::PROPERTY_NODE
                && p.children_with_tokens().any(|c| c.kind() == kind::TAG)
        });
        if tagged {
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
    options: &[OptionDecl {
        name: "order",
        default: Opt::List(&[]),
        description: "Keys that come first, in this order; the rest follow alphabetically.",
    }],
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
        let order = ctx.options.list("order");
        let names: Vec<String> = entries.iter().map(|(_, k)| key_identity(k, ctx)).collect();
        let mut sorted: Vec<usize> = (0..entries.len()).collect();
        sorted.sort_by_cached_key(|&i| {
            let name = names[i].as_str();
            (
                order.iter().position(|o| *o == name).unwrap_or(order.len()),
                name.to_string(),
            )
        });
        let Some(first_wrong) = (0..entries.len()).find(|&i| sorted[i] != i) else {
            return;
        };
        let ranges: Vec<TextRange> = entries
            .iter()
            .map(|(e, _)| entry_range(e.syntax(), ctx.file.grammar))
            .collect();
        let edits = reorder_edits(ctx.file.text, &ranges, &sorted);
        let at = significant_range(entries[first_wrong].1.syntax(), ctx.file.grammar);
        ctx.report(at, "keys are not sorted").fix(Fix::new(
            "Sort the keys of this mapping",
            FixKind::Unsafe,
            edits,
        ));
    }
}

//! YAML rules: `ambiguous-booleans` and `document-start`. Anchor rules are in `anchor_rules`,
//! rules on values in `value_rules` and `quoted_strings`.

use crate::generated::{BlockMappingEntry, FlowPair, Scalar, kind};
use crate::{anchor_rules, quoted_strings, value_rules};
use core_rules::rule;
use core_rules::tree::significant_range;
use core_rules::{Check, Edit, Opt, OptionDecl, Rule, RuleContext};
use core_syntax::ast::AstNode;
use core_syntax::{SyntaxKind, SyntaxNode, TextSize};

pub static RULES: &[&dyn Rule] = &[
    &anchor_rules::NoUndefinedAlias,
    &anchor_rules::NoDuplicateAnchors,
    &anchor_rules::NoUnusedAnchors,
    &value_rules::NoEmptyValues,
    &value_rules::FloatStyle,
    &quoted_strings::QuotedStrings,
    &AmbiguousBooleans,
    &DocumentStart,
    &DocumentEnd,
];

const AMBIGUOUS: [&str; 16] = [
    "yes", "Yes", "YES", "no", "No", "NO", "on", "On", "ON", "off", "Off", "OFF", "True", "TRUE",
    "False", "FALSE",
];

rule! {
    AmbiguousBooleans {
        id: "ambiguous-booleans",
        category: Suspicious,
        default: Off,
        fix: Unsafe,
        description: "A plain `yes`, `no`, `on`, `off`, `True`, `TRUE`, `False` or `FALSE` (in the usual \
                  casings): a boolean to YAML 1.1 parsers and a string to YAML 1.2 parsers. \
                  Lowercase `true` and `false` are fine.",
        example: "enabled: yes   # becomes enabled: \"yes\"",
        options: [
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
            OptionDecl {
                name: "report_true_false",
                default: Opt::Bool(false),
                check: Check::Any,
                description: "Also report `true` and `false` (unless listed in `allowed`), for \
                          code that wants every boolean quoted or spelled out.",
            },
    ],
        kinds: &[kind::SCALAR],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(token) = Scalar::cast(node.clone()).and_then(|s| s.value()) else {
            return;
        };
        if token.kind() != kind::PLAIN
            || !(AMBIGUOUS.contains(&token.text())
                || (ctx.options.bool("report_true_false")
                    && matches!(token.text(), "true" | "false")))
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
        let edit = Edit::replace(token.text_range(), format!("\"{text}\""));
        ctx.report_fix(
            token.text_range(),
            format!("`{text}` is a boolean in YAML 1.1 and a string in YAML 1.2"),
            (format!("Quote it: \"{text}\""), vec![edit]),
        );
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

rule! {
    DocumentStart {
        id: "document-start",
        category: Style,
        default: Off,
        fix: Safe,
        description: "The first document does not start with `---`.",
        example: "a: 1   # becomes ---\\na: 1",
        kinds: &[kind::STREAM],
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
        ctx.report_fix(
            significant_range(&body, ctx.file.grammar),
            "the document does not start with `---`",
            ("Add `---`", vec![Edit::insert(TextSize::from(0), "---\n")]),
        );
    }
}

rule! {
    DocumentEnd {
        id: "document-end",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A document without the `...` end marker (or, with `present: false`, one that has it).",
        example: "a: 1   # becomes a: 1\\n...",
        options: [
            OptionDecl {
                name: "present",
                default: Opt::Bool(true),
                check: Check::Any,
                description: "`true` requires `...` after every document (fixable); `false` forbids it.",
            },
        ],
        kinds: &[kind::DOCUMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let end = node
            .children_with_tokens()
            .filter_map(|c| c.into_token())
            .find(|t| t.kind() == kind::DOC_END);
        if !ctx.options.bool("present") {
            if let Some(e) = end {
                ctx.report(e.text_range(), "the document ends with `...`");
            }
            return;
        }
        let Some(body) = node.children().next().filter(|_| end.is_none()) else {
            return;
        };
        let range = significant_range(&body, ctx.file.grammar);
        let at = range.end();
        // A block scalar's range already ends with its line break.
        let line_end = ctx.file.text[..usize::from(at)].ends_with('\n');
        // Reported, like yamllint, at the start of the next document's line, or of the last line
        // at the end of the file.
        let line = match node.next_sibling().filter(|n| n.kind() == kind::DOCUMENT) {
            Some(n) => ctx
                .file
                .lines
                .line(significant_range(&n, ctx.file.grammar).start()),
            None => ctx
                .file
                .lines
                .line(TextSize::of(ctx.file.text))
                .saturating_sub(1),
        };
        let start = ctx.file.lines.line_range(ctx.file.text, line).0;
        let edit = Edit::insert(at, if line_end { "...\n" } else { "\n..." });
        ctx.report_fix(
            core_syntax::TextRange::empty(TextSize::from(start as u32)),
            "the document does not end with `...`",
            ("Add `...`", vec![edit]),
        );
    }
}

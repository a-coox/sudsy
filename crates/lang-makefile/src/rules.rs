//! The rules of checkmake (minphony, phonydeclared, timestampexpanded, maxbodylength,
//! uniquetargets) and the rules of the Shell language on the commands of a recipe.

use crate::generated::kind as k;
use core_rules::{Check, Opt, OptionDecl, Rule, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxToken, TextRange};

pub static RULES: &[&dyn Rule] = &[
    &MinPhony,
    &PhonyDeclared,
    &TimestampExpanded,
    &MaxBodyLength,
    &UniqueTargets,
];

/// The words of a `Targets` or `Prereqs` node (references and quoted strings count as words).
fn words(node: &SyntaxNode) -> Vec<SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| matches!(t.kind(), k::WORD | k::MAKE_REF))
        .collect()
}

/// The target names of a rule with the range of each.
fn targets(rule: &SyntaxNode) -> Vec<SyntaxToken> {
    rule.children()
        .find(|n| n.kind() == k::TARGETS)
        .map(|n| words(&n))
        .unwrap_or_default()
}

fn rules(doc: &SyntaxNode) -> Vec<SyntaxNode> {
    doc.descendants().filter(|n| n.kind() == k::RULE).collect()
}

/// The names a `.PHONY` rule declares.
fn phony(doc: &SyntaxNode) -> Vec<String> {
    rules(doc)
        .iter()
        .filter(|r| targets(r).iter().any(|t| t.text() == ".PHONY"))
        .flat_map(|r| r.children().filter(|n| n.kind() == k::PREREQS))
        .flat_map(|p| words(&p))
        .map(|t| t.text().to_string())
        .collect()
}

/// The commands of a rule: its recipe lines, those of the same recipe's conditionals included.
fn body(rule: &SyntaxNode) -> impl Iterator<Item = SyntaxNode> {
    rule.children()
        .find(|n| n.kind() == k::RECIPE)
        .into_iter()
        .flat_map(|r| r.descendants().filter(|n| n.kind() == k::RECIPE_LINE))
}

/// The line of the first target: where checkmake reports a rule.
fn head(rule: &SyntaxNode) -> Option<TextRange> {
    targets(rule).first().map(|t| t.text_range())
}

rule! {
    pub MinPhony {
        id: "min-phony",
        category: Suspicious,
        default: Off,
        description: "A target that must exist and be declared `.PHONY` (checkmake `minphony`). \
                  Reported on the `.PHONY` line, or the last rule's when there is none.",
        example: ".PHONY: all   # `clean` and `test` are missing",
        options: [OptionDecl {
            name: "required",
            default: Opt::List(&["all", "clean", "test"]),
            check: Check::Any,
            description: "The targets every Makefile must declare `.PHONY`.",
        }],
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let all = rules(doc);
        let declared = phony(doc);
        let at = all
            .iter()
            .find(|r| targets(r).iter().any(|t| t.text() == ".PHONY"))
            .or(all.last())
            .and_then(head)
            .unwrap_or_default();
        for name in ctx.options.list("required") {
            let defined = all.iter().any(|r| targets(r).iter().any(|t| t.text() == name));
            if !defined {
                ctx.report(at, format!("required target `{name}` is missing"));
            } else if !declared.iter().any(|d| d == name) {
                ctx.report(at, format!("required target `{name}` is not declared `.PHONY`"));
            }
        }
    }
}

rule! {
    pub PhonyDeclared {
        id: "phony-declared",
        category: Suspicious,
        default: Warn,
        description: "A target with no recipe that is not declared `.PHONY` (checkmake \
                  `phonydeclared`); targets that start with a `.` are special and skipped.",
        example: "all: build   # `all` is not in .PHONY",
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let declared = phony(doc);
        for r in rules(doc) {
            if body(&r).next().is_some() || r.descendants_with_tokens().any(|e| e.kind() == k::SEMI) {
                continue;
            }
            for t in targets(&r) {
                if !t.text().starts_with('.') && !declared.iter().any(|d| d == t.text()) {
                    ctx.report(t.text_range(), format!("target `{}` should be declared `.PHONY`", t.text()));
                }
            }
        }
    }
}

rule! {
    pub TimestampExpanded {
        id: "timestamp-expanded",
        category: Suspicious,
        default: Warn,
        description: "A variable whose value runs `date` and that is not simply expanded (`:=`, \
                  `::=`): every use runs `date` again and the values differ within one build \
                  (checkmake `timestampexpanded`).",
        example: "STAMP = $(shell date +%s)   # use :=",
        kinds: &[k::ASSIGN],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mut tokens = node.children_with_tokens().filter_map(|e| e.into_token());
        let (Some(name), Some(op), Some(value)) = (
            tokens.find(|t| t.kind() == k::WORD),
            tokens.find(|t| t.kind() == k::OP),
            tokens.find(|t| t.kind() == k::VALUE),
        ) else {
            return;
        };
        if !matches!(op.text(), ":=" | "::=") && value.text().trim().contains(" date") {
            ctx.report(
                name.text_range(),
                format!("variable `{}` possibly contains a timestamp and should be simply expanded", name.text()),
            );
        }
    }
}

rule! {
    pub MaxBodyLength {
        id: "max-body-length",
        category: Cleanup,
        default: Off,
        description: "A rule whose recipe has more lines than allowed (checkmake `maxbodylength`). \
                  Every physical line counts, a continued command and a tab-indented comment \
                  included.",
        example: "build:   # 6 lines with the default of 5",
        options: [OptionDecl {
            name: "max",
            default: Opt::Int(5),
            check: Check::Any,
            description: "The most lines a recipe may have.",
        }],
        kinds: &[k::RULE],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let lines: usize = body(node)
            .map(|l| l.text().to_string().trim().matches('\n').count() + 1)
            .sum();
        let max = ctx.options.int("max") as usize;
        if let (true, Some(at)) = (lines > max, head(node)) {
            ctx.report(at, format!("recipe has {lines} lines, more than the {max} allowed"));
        }
    }
}

rule! {
    pub UniqueTargets {
        id: "unique-targets",
        category: Suspicious,
        default: Warn,
        description: "A target defined by more than one rule, which merges prerequisites and \
                  overrides recipes (checkmake `uniquetargets`). Double-colon rules, `.PHONY` and \
                  the targets of `ignore` are skipped.",
        example: "build:\nbuild:   # defined twice",
        options: [OptionDecl {
            name: "ignore",
            default: Opt::List(&[]),
            check: Check::Any,
            description: "Targets that may be defined more than once.",
        }],
        kinds: &[k::DOCUMENT],
    }

    fn check(&self, doc: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let ignore = ctx.options.list("ignore");
        let mut seen: Vec<String> = Vec::new();
        for r in rules(doc) {
            if r.children_with_tokens().any(|e| e.kind() == k::COLON && e.to_string() == "::") {
                continue;
            }
            for t in targets(&r) {
                let name = t.text();
                if name == ".PHONY" || ignore.contains(&name) {
                    continue;
                }
                if seen.iter().any(|s| s == name) {
                    ctx.report(t.text_range(), format!("target `{name}` is defined more than once"));
                } else {
                    seen.push(name.to_string());
                }
            }
        }
    }
}

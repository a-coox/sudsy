//! The Rust rules of HCL (tflint's terraform ruleset): the checks that read several nodes or build
//! a fix. The checks on one node's own text are pattern files in `rules/`.

use crate::generated::kind as k;
use core_rules::diagnostic::Edit;
use core_rules::options::{Check, Opt, OptionDecl};
use core_rules::rule::FileContext;
use core_rules::{Rule, RuleContext, rule};
use core_syntax::{SyntaxKind, SyntaxNode, SyntaxToken, TextRange};
use std::collections::HashSet;

pub static RULES: &[&dyn Rule] = &[
    &DeprecatedInterpolation,
    &DeprecatedIndex,
    &CommentSyntax,
    &NamingConvention,
    &UnusedDeclarations,
];

fn tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.children_with_tokens().filter_map(|e| e.into_token())
}

fn child(node: &SyntaxNode, kind: SyntaxKind) -> Option<SyntaxNode> {
    node.children().find(|c| c.kind() == kind)
}

/// The labels of a block (`resource "a" "b"`: `a`, `b`) as written between the quotes, and the
/// token of each.
fn labels(block: &SyntaxNode) -> Vec<(String, SyntaxToken)> {
    tokens(block)
        .skip(1)
        .take_while(|t| t.kind() != k::PUNCT)
        .filter(|t| matches!(t.kind(), k::STRING | k::IDENT))
        .map(|t| (t.text().trim_matches('"').to_string(), t))
        .collect()
}

/// Nomad job files fill `"${attr.kernel.name}"` and `"${NOMAD_PORT_http}"` in at run time: the
/// Terraform rules about interpolation and indexes leave them alone.
fn is_nomad(path: &std::path::Path) -> bool {
    path.extension().is_some_and(|e| e == "nomad")
        || path.to_str().is_some_and(|p| p.ends_with(".nomad.hcl"))
}

fn block_type(block: &SyntaxNode) -> Option<String> {
    tokens(block)
        .find(|t| t.kind() == k::IDENT)
        .map(|t| t.text().to_string())
}

rule! {
    pub DeprecatedInterpolation {
        id: "deprecated-interpolation",
        category: Style,
        default: Warn,
        fix: Safe,
        description: "A string that is one interpolation, `\"${var.a}\"`, where the expression alone \
                  does (tflint terraform_deprecated_interpolation). The fix writes the expression. \
                  Not in Nomad job files (`.nomad`, `.nomad.hcl`), whose agent fills those in at run time.",
        example: "name = \"${var.name}\"   // name = var.name",
        kinds: &[k::QUOTED_TEMPLATE],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if is_nomad(ctx.file.path) {
            return;
        }
        let only = tokens(node).all(|t| t.kind() != k::TEMPLATE_TEXT);
        let mut parts = node.children();
        let (Some(interp), None) = (parts.next(), parts.next()) else { return };
        if !only || interp.kind() != k::INTERPOLATION || tokens(&interp).any(|t| t.kind() == k::STRIP) {
            return;
        }
        let Some(expr) = child(&interp, k::EXPR) else { return };
        // The node's range includes the trivia after the closing quote.
        let quotes: Vec<_> = tokens(node).filter(|t| t.kind() == k::PUNCT).collect();
        let (Some(open), Some(close)) = (quotes.first(), quotes.last()) else { return };
        let range = TextRange::new(open.text_range().start(), close.text_range().end());
        let message = "interpolation-only expressions are deprecated";
        // The fix needs the string to be a whole expression (an operand would change meaning) and
        // a key to stay one: `{ "${a}" = 1 }` is `{ (a) = 1 }`.
        let whole = node.parent().filter(|p| p.kind() == k::EXPR && p.children().count() == 1);
        let Some(whole) = whole else {
            ctx.report(range, message);
            return;
        };
        let is_key = whole.parent().is_some_and(|o| o.kind() == k::OBJECT_ELEM && o.first_child().as_ref() == Some(&whole));
        let inner = expr.text().to_string().trim().to_string();
        let text = if is_key { format!("({inner})") } else { inner };
        ctx.report_fix(range, message, ("Remove the interpolation", vec![Edit::replace(range, text)]));
    }
}

rule! {
    pub DeprecatedIndex {
        id: "deprecated-index",
        category: Style,
        default: Warn,
        fix: Safe,
        description: "A list item read with the legacy dot syntax, `a.0` (tflint terraform_deprecated_index). \
                  The fix writes `a[0]`.",
        example: "x = aws_instance.web.0.id   // aws_instance.web[0].id",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        if is_nomad(file.path) {
            return;
        }
        for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() == k::LEGACY_INDEX {
                let fixed = format!("[{}]", &t.text()[1..]);
                ctx.report_fix(
                    t.text_range(),
                    "list items should be accessed using square brackets",
                    ("Use square brackets", vec![Edit::replace(t.text_range(), fixed)]),
                );
            }
        }
    }
}

rule! {
    pub CommentSyntax {
        id: "comment-syntax",
        category: Style,
        default: Off,
        fix: Safe,
        description: "A `//` comment where `#` is the convention (tflint terraform_comment_syntax). \
                  The fix writes `#`.",
        example: "// note   # note",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for t in file.root.descendants_with_tokens().filter_map(|e| e.into_token()) {
            if t.kind() == k::COMMENT && t.text().starts_with("//") {
                let marker = TextRange::at(t.text_range().start(), 2.into());
                ctx.report_fix(
                    marker,
                    "single line comments should begin with #",
                    ("Use #", vec![Edit::replace(marker, "#")]),
                );
            }
        }
    }
}

/// `snake_case`, or `mixed_snake_case` with capitals allowed.
fn well_named(name: &str, mixed: bool) -> bool {
    let word = |w: &str| {
        !w.is_empty()
            && w.chars().all(|c| {
                c.is_ascii_digit() || c.is_ascii_lowercase() || mixed && c.is_ascii_uppercase()
            })
    };
    name.starts_with(|c: char| c.is_ascii_alphabetic() && (mixed || c.is_ascii_lowercase()))
        && name.split('_').all(word)
}

rule! {
    pub NamingConvention {
        id: "naming-convention",
        category: Style,
        default: Off,
        description: "The name of a variable, output, module, resource, data source or local does not \
                  follow the format (tflint terraform_naming_convention): the first label of \
                  `variable`, `output` and `module`, the second of `resource` and `data`, and the \
                  names assigned in `locals`.",
        example: "variable \"myVar\" {}   // my_var",
        options: [
            OptionDecl {
                name: "format",
                default: Opt::Str("snake_case"),
                check: Check::OneOf(&["snake_case", "mixed_snake_case"]),
                description: "`snake_case` (lower case letters, digits, single underscores) or \
                          `mixed_snake_case` (upper case letters too).",
            },
        ],
        kinds: &[k::BLOCK],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let mixed = ctx.options.string("format") == "mixed_snake_case";
        let Some(kind) = block_type(node) else { return };
        let names: Vec<(String, TextRange)> = match kind.as_str() {
            "variable" | "output" | "module" => labels(node).into_iter().take(1).map(|(n, t)| (n, t.text_range())).collect(),
            "resource" | "data" => labels(node).into_iter().skip(1).take(1).map(|(n, t)| (n, t.text_range())).collect(),
            "locals" => child(node, k::BODY)
                .into_iter()
                .flat_map(|b| b.children().collect::<Vec<_>>())
                .filter(|a| a.kind() == k::ATTRIBUTE)
                .filter_map(|a| tokens(&a).find(|t| t.kind() == k::IDENT))
                .map(|t| (t.text().to_string(), t.text_range()))
                .collect(),
            _ => return,
        };
        for (name, range) in names {
            if !well_named(&name, mixed) {
                ctx.report(range, format!("`{name}` is not in {}", ctx.options.string("format")));
            }
        }
    }
}

/// Names referenced as `var.x`, `local.x` and `data.t.x` in the file.
fn references(root: &SyntaxNode) -> HashSet<String> {
    let mut found = HashSet::new();
    for var in root.descendants().filter(|n| n.kind() == k::VARIABLE_EXPR) {
        let head = var.text().to_string();
        let mut attrs = std::iter::successors(var.next_sibling(), |n| n.next_sibling())
            .take_while(|n| n.kind() == k::GET_ATTR)
            .map(|n| {
                tokens(&n)
                    .find(|t| t.kind() == k::IDENT)
                    .map(|t| t.text().to_string())
                    .unwrap_or_default()
            });
        let name = match head.as_str() {
            "var" | "local" => attrs.next().map(|a| format!("{head}.{a}")),
            "data" => attrs
                .next()
                .zip(attrs.next())
                .map(|(t, n)| format!("data.{t}.{n}")),
            _ => None,
        };
        found.extend(name);
    }
    found
}

rule! {
    pub UnusedDeclarations {
        id: "unused-declarations",
        category: Suspicious,
        default: Off,
        description: "A variable, local value or data source that nothing in the same file refers to \
                  (tflint terraform_unused_declarations, within one file: a module that uses its \
                  variables in other files needs the rule off or those files in one).",
        example: "variable \"unused\" {}",
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let used = references(file.root);
        for block in file.root.descendants().filter(|n| n.kind() == k::BLOCK) {
            let Some(kind) = block_type(&block) else { continue };
            let declared: Vec<(String, TextRange)> = match kind.as_str() {
                "variable" => labels(&block).into_iter().take(1).map(|(n, t)| (format!("var.{n}"), t.text_range())).collect(),
                "data" => match &labels(&block)[..] {
                    [(t, _), (n, tok)] => vec![(format!("data.{t}.{n}"), tok.text_range())],
                    _ => vec![],
                },
                "locals" => child(&block, k::BODY)
                    .into_iter()
                    .flat_map(|b| b.children().collect::<Vec<_>>())
                    .filter(|a| a.kind() == k::ATTRIBUTE)
                    .filter_map(|a| tokens(&a).find(|t| t.kind() == k::IDENT))
                    .map(|t| (format!("local.{}", t.text()), t.text_range()))
                    .collect(),
                _ => continue,
            };
            for (name, range) in declared {
                if !used.contains(&name) {
                    ctx.report(range, format!("`{name}` is declared but not used"));
                }
            }
        }
    }
}

//! Rules about declarations: `separate-field-declarations` and `suppression-justification`.

use super::{has_comment_in, kind, line_separator, text_of};
use core_rules::rule;
use core_rules::tree::{code_start, significant_range};
use core_rules::{Edit, RuleContext};
use core_syntax::{SyntaxNode, SyntaxNodeExt, TextRange};

rule! {
    pub SeparateFieldDeclarations {
        id: "separate-field-declarations",
        category: Style,
        default: Off,
        fix: Safe,
        description: "One field or event per declaration: `int a, b;` becomes `int a;` and `int b;`, \
                  each with the attributes, modifiers and type of the original. The fix is not \
                  offered when the declaration has a comment in it.",
        example: "int a, b;   // write one declaration for each",
        kinds: &[kind::FIELD_DECLARATION, kind::EVENT_FIELD_DECLARATION],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(declaration) = node
            .child(kind::VARIABLE_DECLARATION)
        else {
            return;
        };
        let grammar = ctx.file.grammar;
        let declarators: Vec<SyntaxNode> = declaration
            .children()
            .filter(|c| c.kind() == kind::VARIABLE_DECLARATOR)
            .collect();
        let Some(first) = declarators.first().filter(|_| declarators.len() > 1) else {
            return;
        };
        let range = TextRange::new(
            code_start(node, grammar),
            significant_range(node, grammar).end(),
        );
        let prefix = TextRange::new(range.start(), significant_range(first, grammar).start());
        let fix = (!has_comment_in(node, range, ctx)).then(|| {
            let head = &ctx.file.text[prefix];
            let separator = line_separator(range, ctx);
            let split: Vec<String> = declarators
                .iter()
                .map(|d| format!("{head}{};", text_of(d, ctx)))
                .collect();
            let edit = Edit::replace(range, split.join(&separator));
            ("Split into one declaration per variable", vec![edit])
        });
        ctx.report_fix(
            prefix,
            "do not combine fields: declare each in its own statement",
            fix,
        );
    }
}

/// Does `value` say nothing as a justification?
fn is_empty_justification(value: &SyntaxNode, ctx: &RuleContext<'_>) -> bool {
    match value.kind() {
        kind::NULL_LITERAL_EXPRESSION => true,
        kind::STRING_LITERAL_EXPRESSION => {
            let text = text_of(value, ctx);
            let inner = text.trim_start_matches('@').trim_matches('"');
            inner.trim().is_empty() || inner == "<Pending>"
        }
        _ => false,
    }
}

rule! {
    pub SuppressionJustification {
        id: "suppression-justification",
        category: Style,
        default: Off,
        description: "`SuppressMessage` needs a `Justification` that says something: not missing, \
                  `null`, empty, blank or `\"<Pending>\"`. Matches the attribute by name. No \
                  fix.",
        example: "[SuppressMessage(\"Style\", \"X1\")]   // add Justification = \"why\"",
        kinds: &[kind::ATTRIBUTE],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(name) = node.children().next() else {
            return;
        };
        let name_text = text_of(&name, ctx);
        let last = name_text.rsplit('.').next().unwrap_or(name_text);
        if !matches!(last, "SuppressMessage" | "SuppressMessageAttribute") {
            return;
        }
        let justification = node
            .child(kind::ATTRIBUTE_ARGUMENT_LIST)
            .into_iter()
            .flat_map(|list| list.children().collect::<Vec<_>>())
            .find(|argument| {
                argument.children().any(|c| {
                    c.kind() == kind::NAME_EQUALS && text_of(&c, ctx).starts_with("Justification")
                })
            });
        let grammar = ctx.file.grammar;
        match justification {
            None => {
                ctx.report(
                    significant_range(&name, grammar),
                    "the suppression has no justification",
                );
            }
            Some(argument) => {
                let empty = argument
                    .children()
                    .last()
                    .is_some_and(|value| is_empty_justification(&value, ctx));
                if empty {
                    ctx.report(
                        significant_range(&argument, grammar),
                        "the suppression's justification is empty",
                    );
                }
            }
        }
    }
}

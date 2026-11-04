//! Rules about how a construct is written, each with a safe fix: `use-string-empty`,
//! `separate-attributes`, `no-empty-attribute-parens` and `no-empty-delegate-parens`.

use super::{kind, line_separator, text_of};
use core_config::Level;
use core_rules::tree::significant_range;
use core_rules::{Category, Edit, Fix, FixKind, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

pub struct UseStringEmpty;
pub struct SeparateAttributes;
pub struct NoEmptyAttributeParens;
pub struct NoEmptyDelegateParens;

static USE_STRING_EMPTY: RuleMeta = RuleMeta {
    id: "use-string-empty",
    category: Category::Style,
    description: "`string.Empty` instead of `\"\"` (or `@\"\"`). Not reported where a constant is \
                  required: `const`, attribute arguments, default parameter values, `case` \
                  labels and patterns.",
    example: "var s = \"\";   // write: var s = string.Empty;",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

static SEPARATE_ATTRIBUTES: RuleMeta = RuleMeta {
    id: "separate-attributes",
    category: Category::Style,
    description: "One attribute per attribute list: `[A, B]` becomes `[A]` and `[B]`. Lists on \
                  parameters and type parameters are left alone. The fix is not offered when the \
                  list has a comment in it.",
    example: "[Serializable, Obsolete]   // write one list for each",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

static NO_EMPTY_ATTRIBUTE_PARENS: RuleMeta = RuleMeta {
    id: "no-empty-attribute-parens",
    category: Category::Style,
    description: "An attribute with empty parentheses: `[Foo()]` is `[Foo]`.",
    example: "[Obsolete()]   // write: [Obsolete]",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[],
};

static NO_EMPTY_DELEGATE_PARENS: RuleMeta = RuleMeta {
    id: "no-empty-delegate-parens",
    category: Category::Style,
    description: "An anonymous method with an empty parameter list: `delegate() { }` is \
                  `delegate { }`. The fix is unsafe: without a parameter list the method also \
                  converts to delegate types that have parameters, which can change which \
                  overload is chosen.",
    example: "Action a = delegate() { };   // write: delegate { }",
    default_level: Level::Off,
    fix: Some(FixKind::Unsafe),
    requires: &[],
    uses: &[],
    options: &[],
};

/// Walking up from an empty-string literal: does a constant have to stand here?
fn constant_required(literal: &SyntaxNode) -> bool {
    for ancestor in literal.ancestors().skip(1) {
        match ancestor.kind() {
            kind::ATTRIBUTE_ARGUMENT
            | kind::CONSTANT_PATTERN
            | kind::CASE_SWITCH_LABEL
            | kind::PARAMETER => return true,
            kind::FIELD_DECLARATION | kind::LOCAL_DECLARATION_STATEMENT => {
                return ancestor
                    .children_with_tokens()
                    .any(|e| e.kind() == kind::CONST_KEYWORD);
            }
            _ => {}
        }
    }
    false
}

impl Rule for UseStringEmpty {
    fn meta(&self) -> &'static RuleMeta {
        &USE_STRING_EMPTY
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::STRING_LITERAL_EXPRESSION]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let range = significant_range(node, ctx.file.grammar);
        let text = &ctx.file.text[usize::from(range.start())..usize::from(range.end())];
        if (text != "\"\"" && text != "@\"\"") || constant_required(node) {
            return;
        }
        ctx.report(range, "use string.Empty for an empty string")
            .fix(Fix::new(
                "Use string.Empty",
                FixKind::Safe,
                vec![Edit::replace(range, "string.Empty")],
            ));
    }
}

impl Rule for SeparateAttributes {
    fn meta(&self) -> &'static RuleMeta {
        &SEPARATE_ATTRIBUTES
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::ATTRIBUTE_LIST]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if node
            .parent()
            .is_some_and(|p| matches!(p.kind(), kind::PARAMETER | kind::TYPE_PARAMETER))
        {
            return;
        }
        let attributes: Vec<SyntaxNode> = node
            .children()
            .filter(|c| c.kind() == kind::ATTRIBUTE)
            .collect();
        let Some(second) = attributes.get(1) else {
            return;
        };
        let grammar = ctx.file.grammar;
        let has_comment = node
            .descendants_with_tokens()
            .any(|e| grammar.has_tag(e.kind(), "comment"));
        let mut fix = None;
        if !has_comment {
            let target = node
                .children()
                .find(|c| c.kind() == kind::ATTRIBUTE_TARGET_SPECIFIER)
                .map_or(String::new(), |t| format!("{} ", text_of(&t, ctx)));
            let range = significant_range(node, grammar);
            let separator = line_separator(range, ctx);
            let lists: Vec<String> = attributes
                .iter()
                .map(|a| format!("[{target}{}]", text_of(a, ctx)))
                .collect();
            fix = Some(Fix::new(
                "Split into one list per attribute",
                FixKind::Safe,
                vec![Edit::replace(range, lists.join(&separator))],
            ));
        }
        let diagnostic = ctx.report(
            significant_range(second, grammar),
            "combined attributes: put each attribute in its own list",
        );
        if let Some(fix) = fix {
            diagnostic.fix(fix);
        }
    }
}

/// The range from the whitespace before `list` to its end, when only whitespace sits between the
/// parentheses of `list` (an empty list with no comment in it).
fn empty_parens_with_leading_space(list: &SyntaxNode, ctx: &RuleContext<'_>) -> Option<TextRange> {
    let range = significant_range(list, ctx.file.grammar);
    let text = ctx.file.text;
    let inner = text[usize::from(range.start())..usize::from(range.end())]
        .strip_prefix('(')?
        .strip_suffix(')')?;
    if !inner.trim().is_empty() {
        return None;
    }
    let before = &text[..usize::from(range.start())];
    let start = before.trim_end_matches([' ', '\t']).len();
    Some(TextRange::new((start as u32).into(), range.end()))
}

impl Rule for NoEmptyAttributeParens {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_ATTRIBUTE_PARENS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::ATTRIBUTE_ARGUMENT_LIST]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        if node
            .children()
            .any(|c| c.kind() == kind::ATTRIBUTE_ARGUMENT)
        {
            return;
        }
        let range = significant_range(node, ctx.file.grammar);
        let remove = empty_parens_with_leading_space(node, ctx);
        let diagnostic = ctx.report(
            TextRange::at(range.start(), 1.into()),
            "the attribute has empty parentheses",
        );
        if let Some(remove) = remove {
            diagnostic.fix(Fix::new(
                "Remove the parentheses",
                FixKind::Safe,
                vec![Edit::delete(remove)],
            ));
        }
    }
}

impl Rule for NoEmptyDelegateParens {
    fn meta(&self) -> &'static RuleMeta {
        &NO_EMPTY_DELEGATE_PARENS
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[kind::PARAMETER_LIST]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let anonymous = node
            .parent()
            .is_some_and(|p| p.kind() == kind::ANONYMOUS_METHOD_EXPRESSION);
        if !anonymous || node.children().any(|c| c.kind() == kind::PARAMETER) {
            return;
        }
        let range = significant_range(node, ctx.file.grammar);
        let remove = empty_parens_with_leading_space(node, ctx);
        let diagnostic = ctx.report(
            TextRange::at(range.start(), 1.into()),
            "the anonymous method has an empty parameter list",
        );
        if let Some(remove) = remove {
            diagnostic.fix(Fix::new(
                "Remove the parentheses",
                FixKind::Unsafe,
                vec![Edit::delete(remove)],
            ));
        }
    }
}

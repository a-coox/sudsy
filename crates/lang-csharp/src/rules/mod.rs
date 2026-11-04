//! C# rules: syntactic ones, and the semantic ones over the
//! binding, type, value and flow layers. `var-preference` and `prefer-collection-expression` are
//! not built (see docs/roadmap.md).

mod async_suffix;
mod binding;
mod declarations;
mod directives;
mod doc_comment_space;
mod empty_catch;
mod expressions;
mod file_layout;
mod flow;
mod member_order;
mod statements;
mod syntax_forms;
mod usings;

use crate::generated::kind;
use core_rules::Rule;
use core_syntax::{SyntaxKind, SyntaxNode};

pub static RULES: &[&dyn Rule] = &[
    &empty_catch::NoEmptyCatch,
    &async_suffix::AsyncSuffix,
    &usings::UsingOrder,
    &member_order::MemberOrder,
    &member_order::AccessorOrder,
    &flow::UnreachableCode,
    &flow::UnusedVariable,
    &flow::UnusedAssignment,
    &binding::RemoveUnusedUsings,
    &binding::PossibleNullDereference,
    &binding::UnusedPrivateMember,
    &doc_comment_space::DocCommentSpace,
    &directives::NoRegions,
    &directives::DirectiveKeywordSpace,
    &directives::NoRegionInBody,
    &expressions::NullableShorthand,
    &expressions::UseLambda,
    &expressions::ConstantOnRight,
    &expressions::UseLiteralSuffix,
    &declarations::SeparateFieldDeclarations,
    &declarations::SuppressionJustification,
    &file_layout::SingleClassPerFile,
    &file_layout::SingleNamespacePerFile,
    &file_layout::FileNameMatchesType,
    &statements::NoEmptyStatement,
    &statements::NoCommentBeforeBlock,
    &syntax_forms::UseStringEmpty,
    &syntax_forms::SeparateAttributes,
    &syntax_forms::NoEmptyAttributeParens,
    &syntax_forms::NoEmptyDelegateParens,
];

/// Methods, constructors, local functions, lambdas and anonymous methods: what body-scope rules
/// are called for.
pub static BODY_KINDS: &[SyntaxKind] = &[
    kind::METHOD_DECLARATION,
    kind::CONSTRUCTOR_DECLARATION,
    kind::LOCAL_FUNCTION_STATEMENT,
    kind::SIMPLE_LAMBDA_EXPRESSION,
    kind::PARENTHESIZED_LAMBDA_EXPRESSION,
    kind::ANONYMOUS_METHOD_EXPRESSION,
    kind::OPERATOR_DECLARATION,
    kind::CONVERSION_OPERATOR_DECLARATION,
    kind::DESTRUCTOR_DECLARATION,
    kind::GET_ACCESSOR_DECLARATION,
    kind::SET_ACCESSOR_DECLARATION,
    kind::INIT_ACCESSOR_DECLARATION,
    kind::ADD_ACCESSOR_DECLARATION,
    kind::REMOVE_ACCESSOR_DECLARATION,
];

/// A text range from a provider's byte offset and length.
fn range_of(start: u32, len: u32) -> core_syntax::TextRange {
    core_syntax::TextRange::at(start.into(), len.into())
}

/// Is there a comment inside `range` (a range within `node`)?
fn has_comment_in(
    node: &SyntaxNode,
    range: core_syntax::TextRange,
    ctx: &core_rules::RuleContext<'_>,
) -> bool {
    node.descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .any(|t| {
            range.contains_range(t.text_range()) && ctx.file.grammar.has_tag(t.kind(), "comment")
        })
}

/// What goes between declarations that replace the one at `range`: a new line with the same
/// indentation when it starts its line, else a space.
fn line_separator(range: core_syntax::TextRange, ctx: &core_rules::RuleContext<'_>) -> String {
    let text = ctx.file.text;
    let start = usize::from(range.start());
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let indent = &text[line_start..start];
    if indent.trim().is_empty() {
        format!("\n{indent}")
    } else {
        " ".to_string()
    }
}

/// The source text of a node without the comments and blank space around it.
fn text_of<'a>(node: &SyntaxNode, ctx: &core_rules::RuleContext<'a>) -> &'a str {
    let range = core_rules::tree::significant_range(node, ctx.file.grammar);
    &ctx.file.text[usize::from(range.start())..usize::from(range.end())]
}

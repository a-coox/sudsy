//! C# rules: syntactic ones, and the semantic ones over the
//! binding, type, value and flow layers. `var-preference` and `prefer-collection-expression` are
//! not built (see docs/roadmap.md).

mod async_suffix;
mod binding;
mod directives;
mod doc_comment_space;
mod empty_catch;
mod flow;
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
    &flow::UnreachableCode,
    &flow::UnusedVariable,
    &flow::UnusedAssignment,
    &binding::RemoveUnusedUsings,
    &binding::PossibleNullDereference,
    &binding::UnusedPrivateMember,
    &doc_comment_space::DocCommentSpace,
    &directives::NoRegions,
    &directives::DirectiveKeywordSpace,
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

/// The source text of a node without the comments and blank space around it.
fn text_of<'a>(node: &SyntaxNode, ctx: &core_rules::RuleContext<'a>) -> &'a str {
    let range = core_rules::tree::significant_range(node, ctx.file.grammar);
    &ctx.file.text[usize::from(range.start())..usize::from(range.end())]
}

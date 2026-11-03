//! C# syntactic rules (plan.md "Initial built-in rules"). The semantic ones (`remove-unused-usings`,
//! `var-preference`, `prefer-collection-expression`) need the binding and type layers of M5.

mod async_suffix;
mod empty_catch;
mod namespace;
mod null_coalescing;
mod usings;

use crate::generated::kind;
use core_rules::Rule;
use core_syntax::{SyntaxKind, SyntaxNode};

pub static RULES: &[&dyn Rule] = &[
    &empty_catch::NoEmptyCatch,
    &async_suffix::AsyncSuffix,
    &namespace::FileScopedNamespace,
    &usings::UsingOrder,
    &null_coalescing::PreferNullCoalescing,
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
];

/// The source text of a node without the comments and blank space around it.
fn text_of<'a>(node: &SyntaxNode, ctx: &core_rules::RuleContext<'a>) -> &'a str {
    let range = core_rules::tree::significant_range(node, ctx.file.grammar);
    &ctx.file.text[usize::from(range.start())..usize::from(range.end())]
}

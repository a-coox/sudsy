//! C# formatting (plan.md "Formatting engine", "Language providers > C#").
//!
//! Nodes are laid out by kind: statements, declarations and lists have their own code; everything
//! else is a sequence of its children with the spacing of [`spacing`]. The hooks of the plan
//! (member chains, lambda as last argument, LINQ queries, binary expression trees, attribute
//! lists) are in [`chains`], [`lists`] and [`query`].

mod braces;
mod chains;
mod lists;
mod options;
mod query;
mod seq;
mod spacing;

pub use options::{BRACE_NEWLINE, MEMBER_CHAIN_MIN_CALLS, TRAILING_COMMAS, option_schema};

use crate::generated::kind as k;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

pub struct CSharpFormatter;

impl NodeFormatter for CSharpFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        let kind = node.kind();
        let g = cx.grammar;
        let doc = match kind {
            k::COMPILATION_UNIT => braces::compilation_unit(cx, node)?,
            k::FILE_SCOPED_NAMESPACE_DECLARATION => braces::file_scoped_namespace(cx, node)?,
            k::INTERPOLATED_STRING_EXPRESSION => cx.verbatim_node(node),
            k::ACCESSOR_LIST => braces::accessor_list(cx, node)?,
            k::SWITCH_SECTION => braces::switch_section(cx, node)?,
            k::SWITCH_EXPRESSION => braces::switch_expression(cx, node)?,
            k::ENUM_DECLARATION => braces::enum_declaration(cx, node)?,
            k::ARGUMENT_LIST => lists::argument_list(cx, node)?,
            k::ATTRIBUTE_LIST => lists::plain_list(cx, node)?,
            k::ANONYMOUS_OBJECT_CREATION_EXPRESSION | k::LIST_PATTERN => {
                lists::list_with_surroundings(cx, node)?
            }
            k::ARRAY_RANK_SPECIFIER
            | k::IMPLICIT_ARRAY_CREATION_EXPRESSION
            | k::IMPLICIT_STACK_ALLOC_ARRAY_CREATION_EXPRESSION => seq::seq(cx, node)?,
            k::EQUALS_VALUE_CLAUSE => chains::equals_value_clause(cx, node)?,
            k::ARROW_EXPRESSION_CLAUSE => chains::arrow_clause(cx, node)?,
            k::CONDITIONAL_EXPRESSION => chains::conditional(cx, node)?,
            k::PARENTHESIZED_EXPRESSION => chains::parenthesized(cx, node)?,
            k::QUERY_EXPRESSION => query::query_expression(cx, node)?,
            k::QUERY_BODY => query::query_body(cx, node)?,
            k::QUERY_CONTINUATION => query::query_continuation(cx, node)?,
            _ => {
                if g.has_tag(kind, "binary-chain") {
                    chains::binary_chain(cx, node)?
                } else if g.has_tag(kind, "member-chain") {
                    chains::member_chain(cx, node)?
                } else if g.has_tag(kind, "assignment") {
                    chains::assignment(cx, node)?
                } else if g.has_tag(kind, "initializer-expression") {
                    lists::initializer(cx, node)?
                } else if kind == k::COLLECTION_EXPRESSION {
                    lists::collection_expression(cx, node)?
                } else if kind == k::BLOCK
                    || g.has_tag(kind, "namespace")
                    || g.has_tag(kind, "type-declaration")
                    || kind == k::SWITCH_STATEMENT
                {
                    braces::braced(cx, node)?
                } else if g.has_tag(kind, "delimited-list") {
                    lists::plain_list(cx, node)?
                } else if let Some(prefix) = seq::prefix_unary(cx, node)? {
                    prefix
                } else {
                    seq::seq(cx, node)?
                }
            }
        };
        Ok(Some(doc))
    }
}

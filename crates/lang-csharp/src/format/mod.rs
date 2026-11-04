//! C# formatting.
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
mod tags;

pub use options::{
    BRACE_NEWLINE, MEMBER_CHAIN_MIN_CALLS, PREPROCESSOR_SYMBOLS, TRAILING_COMMAS, option_schema,
};

use self::options::{
    attach_comments, constructor_initializer_on_new_line, hanging_base_list,
    inline_accessor_bodies, label_outdent, sort_usings, switch_arm_as_assignment,
    unbreakable_single_item_lists,
};
use self::tags::Tag;
use crate::generated::kind as k;
use core_config::ResolvedOptions;
use core_format::comments::CommentStyle;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

pub struct CSharpFormatter;

/// Does the list have exactly one child node, of one of these kinds?
fn single_child(list: &SyntaxNode, kinds: &[core_syntax::SyntaxKind]) -> bool {
    let mut children = list.children();
    matches!((children.next(), children.next()), (Some(c), None) if kinds.contains(&c.kind()))
}

/// `a[0]`, `a[i]`, `a["key"]`: an index that is one name or literal never breaks.
fn simple_index(list: &SyntaxNode) -> bool {
    let mut args = list.children();
    let (Some(arg), None) = (args.next(), args.next()) else {
        return false;
    };
    matches!(
        arg.children().next().map(|e| e.kind()),
        Some(
            k::IDENTIFIER_NAME
                | k::NUMERIC_LITERAL_EXPRESSION
                | k::STRING_LITERAL_EXPRESSION
                | k::CHARACTER_LITERAL_EXPRESSION
        )
    )
}

impl NodeFormatter for CSharpFormatter {
    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &core_syntax::Grammar,
        options: &ResolvedOptions,
    ) -> Option<String> {
        sort_usings(options)
            .then(|| crate::using_sort::sort_all(tree, text, grammar))
            .flatten()
    }

    fn comment_style(&self, options: &ResolvedOptions) -> CommentStyle {
        CommentStyle {
            attach_comments: attach_comments(options),
            ..CommentStyle::default()
        }
    }

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
            k::BASE_CONSTRUCTOR_INITIALIZER | k::THIS_CONSTRUCTOR_INITIALIZER
                if constructor_initializer_on_new_line(cx.options) =>
            {
                braces::constructor_initializer(cx, node)?
            }
            k::GET_ACCESSOR_DECLARATION
            | k::SET_ACCESSOR_DECLARATION
            | k::INIT_ACCESSOR_DECLARATION
            | k::ADD_ACCESSOR_DECLARATION
            | k::REMOVE_ACCESSOR_DECLARATION
            | k::UNKNOWN_ACCESSOR_DECLARATION
                if inline_accessor_bodies(cx.options) =>
            {
                match braces::accessor_declaration(cx, node) {
                    Some(doc) => doc?,
                    None => seq::seq(cx, node)?,
                }
            }
            k::LABELED_STATEMENT if label_outdent(cx.options) > 0 => {
                seq::labeled_statement(cx, node)?
            }
            k::SWITCH_SECTION => braces::switch_section(cx, node)?,
            k::SWITCH_EXPRESSION => braces::switch_expression(cx, node)?,
            k::ENUM_DECLARATION => braces::enum_declaration(cx, node)?,
            k::ARGUMENT_LIST => lists::argument_list(cx, node)?,
            k::ATTRIBUTE_LIST
                if unbreakable_single_item_lists(cx.options)
                    && single_child(node, &[k::ATTRIBUTE]) =>
            {
                lists::unbreakable_list(cx, node)?
            }
            k::TYPE_ARGUMENT_LIST
                if unbreakable_single_item_lists(cx.options)
                    && single_child(
                        node,
                        &[k::PREDEFINED_TYPE, k::IDENTIFIER_NAME, k::QUALIFIED_NAME],
                    ) =>
            {
                lists::unbreakable_list(cx, node)?
            }
            k::BRACKETED_ARGUMENT_LIST
                if unbreakable_single_item_lists(cx.options) && simple_index(node) =>
            {
                lists::unbreakable_list(cx, node)?
            }
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
            k::SWITCH_EXPRESSION_ARM if switch_arm_as_assignment(cx.options) => {
                chains::switch_arm(cx, node)?
            }
            k::BASE_LIST if hanging_base_list(cx.options) => braces::base_list(cx, node)?,
            k::PARENTHESIZED_EXPRESSION => chains::parenthesized(cx, node)?,
            k::QUERY_EXPRESSION => query::query_expression(cx, node)?,
            k::QUERY_BODY => query::query_body(cx, node)?,
            k::QUERY_CONTINUATION => query::query_continuation(cx, node)?,
            _ => {
                if tags::has(g, kind, Tag::BinaryChain) {
                    chains::binary_chain(cx, node)?
                } else if tags::has(g, kind, Tag::MemberChain) {
                    chains::member_chain(cx, node)?
                } else if tags::has(g, kind, Tag::Assignment) {
                    chains::assignment(cx, node)?
                } else if tags::has(g, kind, Tag::InitializerExpression) {
                    lists::initializer(cx, node)?
                } else if kind == k::COLLECTION_EXPRESSION {
                    lists::collection_expression(cx, node)?
                } else if kind == k::BLOCK
                    || tags::has(g, kind, Tag::Namespace)
                    || tags::has(g, kind, Tag::TypeDeclaration)
                    || kind == k::SWITCH_STATEMENT
                {
                    braces::braced(cx, node)?
                } else if tags::has(g, kind, Tag::DelimitedList) {
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

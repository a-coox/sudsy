//! JavaScript and TypeScript formatting, Prettier's layout. `format_node` asks each module in
//! [`MODULES`] for the node (a module matches the kinds it owns); a node no module owns is
//! printed as written, with its strings and numbers restyled (`literal::verbatim`). A later step
//! adds its own module and one line in `MODULES`.

mod arrow;
mod assign;
mod ast;
mod binary;
mod call;
mod chain;
mod class;
mod decl;
mod jsx;
mod literal;
mod object;
mod parens;
mod rewrite;
mod stmt;
mod template;
mod ternary;
mod ts;
mod unary;
mod util;

use core_config::shared;
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter};
use core_syntax::SyntaxNode;

pub(crate) use literal::same_value;

include!("schema.generated.rs");

pub struct JsFormatter;

type Module = fn(&SyntaxNode, &FormatCtx<'_>) -> Result<Option<Doc>, FormatError>;

const MODULES: &[Module] = &[
    stmt::format,
    decl::format,
    class::format,
    parens::format,
    binary::format,
    call::format,
    chain::format,
    assign::format,
    ternary::format,
    arrow::format,
    unary::format,
    jsx::format,
    template::format,
    ts::format,
    object::format,
    literal::format,
];

impl NodeFormatter for JsFormatter {
    fn format_node(
        &self,
        node: &SyntaxNode,
        cx: &FormatCtx<'_>,
    ) -> Result<Option<Doc>, FormatError> {
        for module in MODULES {
            if let Some(doc) = module(node, cx)? {
                return Ok(Some(parens::wrap(node, cx, doc)));
            }
        }
        Ok(Some(parens::wrap(node, cx, literal::verbatim(cx, node))))
    }

    fn rewrite_source(
        &self,
        tree: &SyntaxNode,
        text: &str,
        grammar: &core_syntax::Grammar,
        options: &core_config::ResolvedOptions,
    ) -> Option<String> {
        rewrite::rewrite(tree, text, grammar, options)
    }

    fn extra_check(&self, before: &SyntaxNode, after: &SyntaxNode) -> Result<(), String> {
        crate::safety::check(before, after)
    }
}

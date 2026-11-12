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
use core_format::{Doc, FormatCtx, FormatError, NodeFormatter, comments};
use core_syntax::SyntaxNode;

pub(crate) use literal::same_value;

include!("schema.generated.rs");

pub struct JsFormatter;

/// The last token of `node` that is not trivia (JS has no structured trivia, so this is
/// `FormatCtx::last_token` without its ancestor checks, which cost on every node).
fn last_significant(cx: &FormatCtx<'_>, node: &SyntaxNode) -> Option<core_syntax::SyntaxToken> {
    let start = node.text_range().start();
    let mut t = node.last_token();
    while let Some(x) = t {
        if !cx.grammar.is_trivia(x.kind()) {
            return Some(x);
        }
        t = x.prev_token().filter(|p| p.text_range().start() >= start);
    }
    None
}

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
        // A trailing comment belongs to the largest node that ends at its token (Prettier's
        // attachment), so it goes after that node's layout and its line break reaches only the
        // groups around the node, not those inside it.
        let hoisted = comments::comment_at_end(cx.grammar, node)
            .then(|| last_significant(cx, node))
            .flatten()
            .filter(|last| {
                comments::has_trailing_comment(cx.grammar, last)
                    && node
                        .parent()
                        .is_some_and(|p| cx.last_token(&p).is_some_and(|l| l != *last))
            });
        if let Some(last) = &hoisted {
            cx.hold_trailing(last);
        }
        let mut doc = None;
        for module in MODULES {
            if let Some(d) = module(node, cx)? {
                doc = Some(d);
                break;
            }
        }
        let doc = parens::wrap(node, cx, doc.unwrap_or_else(|| literal::verbatim(cx, node)));
        Ok(Some(match &hoisted {
            Some(last) => core_format::doc::concat(vec![doc, cx.held_trailing(last)]),
            None => doc,
        }))
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

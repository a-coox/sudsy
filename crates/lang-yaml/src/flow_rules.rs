//! Opt-in rules that forbid flow collections: `no-flow-mappings` and `no-flow-sequences`.

use crate::generated::kind;
use core_rules::rule;
use core_rules::{Check, Opt, OptionDecl, RuleContext};
use core_syntax::{SyntaxKind, SyntaxNode};

const ALLOW_EMPTY: OptionDecl = OptionDecl {
    name: "allow_empty",
    default: Opt::Bool(false),
    check: Check::Any,
    description: "Do not report empty collections (`{}`, `[]`).",
};

/// Reports the opening delimiter of a flow collection, unless it is empty and that is allowed.
fn forbid(node: &SyntaxNode, ctx: &mut RuleContext<'_>, open: SyntaxKind, message: &str) {
    if ctx.options.bool("allow_empty") && node.children().next().is_none() {
        return;
    }
    if let Some(token) = node
        .children_with_tokens()
        .filter_map(|c| c.into_token())
        .find(|t| t.kind() == open)
    {
        ctx.report(token.text_range(), message);
    }
}

rule! {
    pub NoFlowMappings {
        id: "no-flow-mappings",
        category: Style,
        default: Off,
        description: "A flow mapping (`{a: 1}`); write a block mapping instead.",
        example: "point: {x: 1}   # reported; write the entries on their own lines",
        options: [ALLOW_EMPTY],
        kinds: &[kind::FLOW_MAPPING],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        forbid(node, ctx, kind::L_BRACE, "flow mappings are not allowed");
    }
}

rule! {
    pub NoFlowSequences {
        id: "no-flow-sequences",
        category: Style,
        default: Off,
        description: "A flow sequence (`[a, b]`); write a block sequence instead.",
        example: "list: [1, 2]   # reported; write one `- item` per line",
        options: [ALLOW_EMPTY],
        kinds: &[kind::FLOW_SEQUENCE],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        forbid(node, ctx, kind::L_BRACKET, "flow sequences are not allowed");
    }
}

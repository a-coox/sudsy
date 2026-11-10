use crate::generated::kind;
use core_rules::{Check, Opt, OptionDecl, RuleContext, rule};
use core_syntax::SyntaxNode;

/// Levels of selection sets from `set` down: a set, and the sets of its fields and inline fragments.
fn depth(set: &SyntaxNode) -> i64 {
    1 + set
        .children()
        .filter_map(|selection| {
            selection
                .children()
                .find(|n| n.kind() == kind::SELECTION_SET)
        })
        .map(|inner| depth(&inner))
        .max()
        .unwrap_or(0)
}

rule! {
    pub SelectionSetDepth {
        id: "selection-set-depth",
        category: Suspicious,
        default: Off,
        description: "An operation or fragment nests selection sets deeper than allowed, which \
                  makes a query expensive to execute. Fragments that are spread are not followed.",
        example: "{ a { b { c { d { e { f { g { h } } } } } } } }   # reported at the default depth",
        options: [OptionDecl {
            name: "max_depth",
            default: Opt::Int(7),
            check: Check::Any,
            description: "The deepest allowed nesting; the operation's own selection set is level 1.",
        }],
        kinds: &[kind::OPERATION, kind::FRAGMENT],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(top) = node.children().find(|n| n.kind() == kind::SELECTION_SET) else {
            return;
        };
        let max = ctx.options.int("max_depth");
        let found = depth(&top);
        if found > max {
            ctx.report(
                top.text_range(),
                format!("selection sets nest {found} levels deep; at most {max} are allowed"),
            );
        }
    }
}

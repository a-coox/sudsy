//! Rules about how modules are imported that need to know what an imported name is used for.

use super::scope::run;
use super::{Check, code_text, span};
use crate::generated::kind as k;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_syntax::SyntaxNode;
use oxc_ast::AstKind;
use oxc_ast::ast::ImportDeclarationSpecifier;
use oxc_semantic::{NodeId, Reference};
use oxc_span::{GetSpan, Span};

/// `no-require-imports`: `require(...)` of the global `require`, and `import a = require(...)`.
pub(super) fn require_imports(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let range = span(ctx, n);
    if n.kind() == k::CALL_EXPRESSION {
        let Some(callee) = n.children().next() else {
            return;
        };
        if code_text(&callee, ctx.file.grammar) != "require" {
            return;
        }
        // A `require` the file declares itself is not the module loader.
        let start = u32::from(span(ctx, &callee).start());
        let declared = model(ctx.file).is_some_and(|m| {
            m.with(|cx| {
                let sc = cx.s.scoping();
                let global = sc
                    .root_unresolved_references()
                    .iter()
                    .find(|(name, _)| name.as_str() == "require");
                !global.is_some_and(|(_, ids)| {
                    ids.iter()
                        .any(|&id| cx.s.reference_span(sc.get_reference(id)).start == start)
                })
            })
        });
        if declared {
            return;
        }
    }
    c.hit(ctx, range, "");
}

/// Is the value reference `r` safe in a type-only import (`typeof a`, `a.b` in a type, a
/// computed key of a type literal)?
fn value_use_is_type(cx: &Cx<'_, '_>, id: NodeId) -> bool {
    let nodes = cx.s.nodes();
    let (mut child, mut parent) = (id, nodes.parent_id(id));
    loop {
        match cx.kind(parent) {
            AstKind::TSTypeQuery(_) => return true,
            AstKind::TSQualifiedName(_)
            | AstKind::StaticMemberExpression(_)
            | AstKind::ComputedMemberExpression(_) => {}
            AstKind::TSPropertySignature(p) => return p.key.span() == cx.span(child),
            _ => return false,
        }
        child = parent;
        parent = nodes.parent_id(parent);
    }
}

fn only_type_use(cx: &Cx<'_, '_>, r: &Reference) -> bool {
    if r.is_read() || r.is_write() {
        return value_use_is_type(cx, r.node_id());
    }
    r.is_type()
}

/// `1`, `1 and 2`, `1, 2 and 3` with each name in quotes.
fn word_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [one] => format!("\"{one}\""),
        [rest @ .., last] => {
            let head: Vec<String> = rest.iter().map(|n| format!("\"{n}\"")).collect();
            format!("{} and \"{last}\"", head.join(", "))
        }
    }
}

/// `consistent-type-imports`: an import only used as a type is written `import type`, and a type
/// is not imported with `import("a")`.
pub(super) fn type_imports(c: &Check, n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    if n.kind() == k::TS_IMPORT_TYPE {
        c.hit(ctx, span(ctx, n), "");
        return;
    }
    let message = c.message.clone();
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits: Vec<(Span, String)> = Vec::new();
        if !cx.ts {
            return hits;
        }
        for (_, node) in cx.s.nodes().iter_enumerated() {
            let AstKind::ImportDeclaration(d) = node.kind() else {
                continue;
            };
            let Some(specifiers) = &d.specifiers else {
                continue;
            };
            if d.import_kind.is_type() {
                continue;
            }
            let (mut types, mut others) = (Vec::new(), 0);
            for s in specifiers {
                let (local, is_type) = match s {
                    ImportDeclarationSpecifier::ImportSpecifier(s) => {
                        (&s.local, s.import_kind.is_type())
                    }
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => (&s.local, false),
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => (&s.local, false),
                };
                if is_type {
                    continue;
                }
                let refs: Vec<&Reference> = local
                    .symbol_id
                    .get()
                    .map(|sid| sc.get_resolved_references(sid).collect())
                    .unwrap_or_default();
                let pragma = cx.jsx_pragma(local.name.as_ref());
                if !pragma && !refs.is_empty() && refs.iter().all(|r| only_type_use(cx, r)) {
                    types.push(local.name.to_string());
                } else {
                    others += 1;
                }
            }
            if types.is_empty() {
                continue;
            }
            let text = if others == 0 && d.with_clause.is_none() {
                "All imports in the declaration are only used as types. Use `import type`."
                    .to_string()
            } else if others == 0 {
                continue;
            } else {
                message.replace("{name}", &word_list(&types))
            };
            hits.push((d.span, text));
        }
        hits
    });
}

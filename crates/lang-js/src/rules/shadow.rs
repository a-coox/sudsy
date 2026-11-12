//! `no-shadow`.

use super::Check;
use super::scope::run;
use crate::semantic::Cx;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;
use oxc_ast::AstKind;
use oxc_ast::ast::{ClassType, FunctionType};
use oxc_semantic::{NodeId, ScopeId, SymbolFlags, SymbolId};
use oxc_span::GetSpan;
use std::collections::HashMap;

fn line_of(text: &str, offset: u32) -> usize {
    text.as_bytes()[..offset as usize]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Does the symbol name a value (a variable, function, class, enum, value import)?
fn is_value(flags: SymbolFlags) -> bool {
    flags.intersects(SymbolFlags::Value | SymbolFlags::NamespaceModule)
        || (flags.contains(SymbolFlags::Import) && !flags.contains(SymbolFlags::TypeImport))
}

/// Is the parameter `id` one of a function type, a signature or a function without a body?
fn in_signature(cx: &Cx<'_, '_>, id: NodeId) -> bool {
    let nodes = cx.s.nodes();
    let Some(params) = nodes
        .ancestor_ids(id)
        .find(|a| matches!(cx.kind(*a), AstKind::FormalParameters(_)))
    else {
        return false;
    };
    match cx.parent(params) {
        AstKind::Function(f) => f.body.is_none(),
        AstKind::TSFunctionType(_)
        | AstKind::TSConstructorType(_)
        | AstKind::TSMethodSignature(_)
        | AstKind::TSCallSignatureDeclaration(_)
        | AstKind::TSConstructSignatureDeclaration(_) => true,
        _ => false,
    }
}

/// Is the type parameter declared by `node` one of a `static` method?
fn static_method_generic(cx: &Cx<'_, '_>, node: NodeId) -> bool {
    let mut up = cx.s.nodes().ancestor_ids(node);
    let (_, Some(function)) = (up.next(), up.next()) else {
        return false;
    };
    matches!(cx.kind(function), AstKind::Function(_))
        && matches!(cx.parent(function), AstKind::MethodDefinition(m) if m.r#static)
}

/// Is `inner` (a function or class expression) the initializer of the variable declared by
/// `outer`, directly or as an operand of `||`, `&&`, `??` or a branch of `?:`?
fn is_initializer(cx: &Cx<'_, '_>, outer: NodeId, inner: NodeId) -> bool {
    let AstKind::VariableDeclarator(d) = cx.kind(outer) else {
        return false;
    };
    let Some(init) = &d.init else { return false };
    let nodes = cx.s.nodes();
    let mut top = inner;
    loop {
        let parent = nodes.parent_id(top);
        let climb = match cx.kind(parent) {
            AstKind::LogicalExpression(_) => true,
            AstKind::ConditionalExpression(c) => c.test.span() != cx.span(top),
            _ => false,
        };
        if !climb {
            break;
        }
        top = parent;
    }
    init.span() == cx.span(top)
}

/// A variable that has the name of one in a scope around it (ESLint's defaults: functions are
/// hoisted, the others are not, and `var a = function a() {}` does not count).
pub(super) fn shadow(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let text = ctx.file.text;
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let scopes: Vec<_> = sc.scope_descendants_from_root().collect();
        // The name of a function expression is a variable of its own, between the scope of the
        // function and the one around it.
        let mut names: Vec<HashMap<&str, SymbolId>> = vec![HashMap::new(); scopes.len()];
        let mut expression_names: HashMap<ScopeId, SymbolId> = HashMap::new();
        for sid in sc.symbol_ids() {
            let scope = sc.symbol_scope_id(sid);
            if sc
                .symbol_flags(sid)
                .contains(SymbolFlags::FunctionExpression)
            {
                expression_names.insert(scope, sid);
            } else {
                names[scope.index()]
                    .entry(sc.symbol_name(sid))
                    .or_insert(sid);
            }
        }
        let find = |from: ScopeId, name: &str| {
            sc.scope_ancestors(from).find_map(|s| {
                let own = names[s.index()].get(name).copied();
                own.or_else(|| {
                    expression_names
                        .get(&s)
                        .copied()
                        .filter(|e| sc.symbol_name(*e) == name)
                })
            })
        };
        let mut hits = Vec::new();
        for sid in sc.symbol_ids() {
            let scope = sc.symbol_scope_id(sid);
            let name = sc.symbol_name(sid);
            // `declare global { ... }` augments the global scope: nothing in it shadows.
            let global = |s| matches!(cx.kind(sc.get_node_id(s)), AstKind::TSGlobalDeclaration(_));
            if sc.scope_ancestors(scope).any(global) {
                continue;
            }
            let (inner_flags, is_expression_name) = {
                let f = sc.symbol_flags(sid);
                (f, f.contains(SymbolFlags::FunctionExpression))
            };
            let own = (!is_expression_name)
                .then(|| {
                    expression_names
                        .get(&scope)
                        .copied()
                        .filter(|e| sc.symbol_name(*e) == name)
                })
                .flatten();
            let Some(outer) = own.or_else(|| sc.scope_parent_id(scope).and_then(|p| find(p, name)))
            else {
                continue;
            };
            let (inner, outer_decl) = (cx.decls(sid)[0], cx.decls(outer)[0]);
            let outer_flags = sc.symbol_flags(outer);
            let on_initializer = {
                let declaration = matches!(cx.kind(outer_decl.1), AstKind::VariableDeclarator(_))
                    .then(|| cx.span(cx.s.nodes().parent_id(outer_decl.1)));
                let expression = match cx.kind(inner.1) {
                    AstKind::Function(f) => f.r#type == FunctionType::FunctionExpression,
                    AstKind::Class(k) => k.r#type == ClassType::ClassExpression,
                    _ => false,
                };
                if cx.ts {
                    expression && is_initializer(cx, outer_decl.1, inner.1)
                } else {
                    expression
                        && declaration
                            .is_some_and(|d| d.start < inner.0.start && inner.0.end < d.end)
                        && sc.scope_parent_id(scope) == Some(sc.symbol_scope_id(outer))
                }
            };
            let hoisted = matches!(
                cx.kind(outer_decl.1),
                AstKind::Function(f) if f.r#type == FunctionType::FunctionDeclaration
            );
            let in_tdz = inner.0.end < outer_decl.0.start && !hoisted;
            let ts_skip = cx.ts && {
                let shadowed_value =
                    is_value(outer_flags) && !outer_flags.contains(SymbolFlags::TypeImport);
                let signature = matches!(
                    cx.kind(inner.1),
                    AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_)
                ) && cx
                    .binding(inner.0.start)
                    .is_some_and(|b| in_signature(cx, b));
                let generic = matches!(cx.kind(inner.1), AstKind::TSTypeParameter(_))
                    && static_method_generic(cx, inner.1);
                is_value(inner_flags) != shadowed_value || signature || generic
            };
            if on_initializer || in_tdz || ts_skip {
                continue;
            }
            let line = line_of(text, outer_decl.0.start).to_string();
            hits.push((
                inner.0,
                c.message.replace("{name}", name).replace("{line}", &line),
            ));
        }
        hits
    });
}

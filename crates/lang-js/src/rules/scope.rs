//! Rules that read what names bind to (docs/design/wave-3b-js.md step 15): the model of
//! `crate::semantic`, built on the first of them that runs on a file. Positions follow ESLint's
//! reports, so the parity runners compare them as they are.

use super::Check;
use crate::semantic::{Cx, model};
use core_rules::RuleContext;
use core_syntax::{SyntaxNode, TextRange};
use oxc_ast::AstKind;
use oxc_ast::ast::{ClassElement, Expression};
use oxc_semantic::{NodeId, ScopeFlags, ScopeId, SymbolFlags, SymbolId};
use oxc_span::{GetSpan, Span};

/// The ES2026 globals of ESLint's default configuration.
const BUILTINS: &[&str] = &[
    "AggregateError",
    "Array",
    "ArrayBuffer",
    "AsyncDisposableStack",
    "Atomics",
    "BigInt",
    "BigInt64Array",
    "BigUint64Array",
    "Boolean",
    "DataView",
    "Date",
    "DisposableStack",
    "Error",
    "EvalError",
    "FinalizationRegistry",
    "Float16Array",
    "Float32Array",
    "Float64Array",
    "Function",
    "Infinity",
    "Int16Array",
    "Int32Array",
    "Int8Array",
    "Intl",
    "Iterator",
    "JSON",
    "Map",
    "Math",
    "NaN",
    "Number",
    "Object",
    "Promise",
    "Proxy",
    "RangeError",
    "ReferenceError",
    "Reflect",
    "RegExp",
    "Set",
    "SharedArrayBuffer",
    "String",
    "SuppressedError",
    "Symbol",
    "SyntaxError",
    "TypeError",
    "URIError",
    "Uint16Array",
    "Uint32Array",
    "Uint8Array",
    "Uint8ClampedArray",
    "WeakMap",
    "WeakRef",
    "WeakSet",
    "constructor",
    "decodeURI",
    "decodeURIComponent",
    "encodeURI",
    "encodeURIComponent",
    "escape",
    "eval",
    "globalThis",
    "hasOwnProperty",
    "isFinite",
    "isNaN",
    "isPrototypeOf",
    "parseFloat",
    "parseInt",
    "propertyIsEnumerable",
    "toLocaleString",
    "toString",
    "undefined",
    "unescape",
    "valueOf",
];

/// What a `commonjs` file (`.cjs`) adds.
const COMMONJS: &[&str] = &["exports", "global", "module", "require"];

pub(super) fn range(sp: Span) -> TextRange {
    TextRange::new(sp.start.into(), sp.end.into())
}

/// Runs `f` on the file's model (nothing for a file that does not parse) and reports what it
/// returns, in source order.
pub(super) fn run(ctx: &mut RuleContext<'_>, f: impl FnOnce(&Cx<'_, '_>) -> Vec<(Span, String)>) {
    let Some(m) = model(ctx.file) else { return };
    let mut hits = m.with(f);
    hits.sort_by_key(|(sp, _)| sp.start);
    hits.dedup();
    for (sp, message) in hits {
        ctx.report(range(sp), message);
    }
}

pub(super) fn is_function(k: AstKind<'_>) -> bool {
    matches!(
        k,
        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
    )
}

pub(super) fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn is_jsx_ref(k: AstKind<'_>) -> bool {
    matches!(
        k,
        AstKind::JSXOpeningElement(_)
            | AstKind::JSXClosingElement(_)
            | AstKind::JSXMemberExpression(_)
    )
}

/// The nearest function scope (or the top one) that holds `scope`'s variables.
pub(super) fn var_scope(cx: &Cx<'_, '_>, mut scope: ScopeId) -> ScopeId {
    let sc = cx.s.scoping();
    while !sc.scope_flags(scope).intersects(ScopeFlags::Var) {
        match sc.scope_parent_id(scope) {
            Some(p) => scope = p,
            None => break,
        }
    }
    scope
}

/// The execution context a node runs in: its function, the program, or the initializer of an
/// instance field. Static initializers and static blocks run as part of the class's context.
fn exec_ctx(cx: &Cx<'_, '_>, from: NodeId, inclusive: bool, at: Span) -> NodeId {
    let nodes = cx.s.nodes();
    let own = inclusive.then_some(from);
    for id in own.into_iter().chain(nodes.ancestor_ids(from)) {
        match cx.kind(id) {
            k if is_function(k) => return id,
            AstKind::PropertyDefinition(p)
                if !p.r#static && p.value.as_ref().is_some_and(|v| contains(v.span(), at)) =>
            {
                return id;
            }
            AstKind::AccessorProperty(p)
                if !p.r#static && p.value.as_ref().is_some_and(|v| contains(v.span(), at)) =>
            {
                return id;
            }
            AstKind::Program(_) => return id,
            _ => {}
        }
    }
    from
}

/// Is `r` evaluated in an execution context other than the one that declares the symbol?
fn separate_context(cx: &Cx<'_, '_>, r: &oxc_semantic::Reference, sid: SymbolId) -> bool {
    let sc = cx.s.scoping();
    let at = cx.span(r.node_id());
    let declared = exec_ctx(cx, sc.get_node_id(sc.symbol_scope_id(sid)), true, at);
    exec_ctx(cx, r.node_id(), false, at) != declared
}

/// `no-undef`: a name no scope declares and the globals do not list.
pub(super) fn undef(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    let extra: Vec<String> = ctx
        .options
        .list("globals")
        .into_iter()
        .map(String::from)
        .collect();
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits = Vec::new();
        for (name, ids) in sc.root_unresolved_references() {
            let name = name.as_str();
            if BUILTINS.contains(&name)
                || (cx.commonjs && COMMONJS.contains(&name))
                || extra.iter().any(|g| g == name)
            {
                continue;
            }
            for &rid in ids {
                let r = sc.get_reference(rid);
                let parent = cx.parent(r.node_id());
                let flags = r.flags();
                let typeof_operand = matches!(
                    parent,
                    AstKind::UnaryExpression(u) if u.operator.is_typeof()
                );
                // `arguments` is bound by the nearest ordinary function.
                let arguments = name == "arguments"
                    && (cx.commonjs
                        || sc.scope_ancestors(r.scope_id()).any(|s| {
                            let f = sc.scope_flags(s);
                            f.is_function() && !f.is_arrow()
                        }));
                if flags.is_type() || is_jsx_ref(parent) || typeof_operand || arguments {
                    continue;
                }
                let span = cx.s.reference_span(r);
                hits.push((span, c.message.replace("{name}", name)));
            }
        }
        hits
    });
}

/// How a symbol was first declared.
fn first_decl_kind<'a>(cx: &Cx<'_, 'a>, sid: SymbolId) -> AstKind<'a> {
    cx.kind(cx.s.scoping().symbol_declaration(sid))
}

/// The write references of the symbols `want` selects, reported at the identifier.
fn writes(
    c: &Check,
    ctx: &mut RuleContext<'_>,
    want: impl Fn(&Cx<'_, '_>, SymbolId, SymbolFlags) -> bool,
) {
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits = Vec::new();
        for sid in sc.symbol_ids() {
            if !want(cx, sid, sc.symbol_flags(sid)) {
                continue;
            }
            for r in sc.get_resolved_references(sid).filter(|r| r.is_write()) {
                let name = sc.symbol_name(sid);
                hits.push((cx.s.reference_span(r), c.message.replace("{name}", name)));
            }
        }
        hits
    });
}

/// `no-const-assign`
pub(super) fn const_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |_, _, f| f.contains(SymbolFlags::ConstVariable));
}

/// `no-class-assign`
pub(super) fn class_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |cx, sid, _| {
        matches!(first_decl_kind(cx, sid), AstKind::Class(_))
    });
}

/// `no-func-assign`
pub(super) fn func_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    writes(c, ctx, |cx, sid, _| {
        matches!(first_decl_kind(cx, sid), AstKind::Function(_))
    });
}

/// The start of the assignment, update, `delete`, call or `for` loop an identifier is part of.
fn write_node_start(cx: &Cx<'_, '_>, id: NodeId, fallback: Span) -> u32 {
    (cx.s.nodes().ancestor_ids(id))
        .map(|a| cx.kind(a))
        .find(|k| {
            matches!(
                k,
                AstKind::AssignmentExpression(_)
                    | AstKind::UpdateExpression(_)
                    | AstKind::UnaryExpression(_)
                    | AstKind::CallExpression(_)
                    | AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
            )
        })
        .map_or(fallback.start, |k| k.span().start)
}

/// Is `id` (a read of a namespace import) the object of a member that is written to, or the first
/// argument of `Object.assign` and the like?
fn member_written(cx: &Cx<'_, '_>, id: NodeId, span: Span) -> bool {
    let parent_id = cx.s.nodes().parent_id(id);
    match cx.kind(parent_id) {
        AstKind::StaticMemberExpression(m) if m.object.span() == span => {}
        AstKind::ComputedMemberExpression(m) if m.object.span() == span => {}
        AstKind::CallExpression(call) => {
            let first = call.arguments.first().is_some_and(|a| a.span() == span);
            let Some(member) = call.callee.as_member_expression() else {
                return false;
            };
            let (Expression::Identifier(obj), Some(prop)) = (
                member.object().get_inner_expression(),
                member.static_property_name(),
            ) else {
                return false;
            };
            let methods: &[&str] = match obj.name.as_str() {
                "Object" => &[
                    "assign",
                    "defineProperty",
                    "defineProperties",
                    "freeze",
                    "setPrototypeOf",
                ],
                "Reflect" => &["defineProperty", "deleteProperty", "set", "setPrototypeOf"],
                _ => return false,
            };
            let prop = prop.as_str().unwrap_or_default();
            return first && methods.contains(&prop) && cx.s.is_reference_to_global_variable(obj);
        }
        _ => return false,
    }
    let member = cx.span(parent_id);
    let grand = cx.s.nodes().parent_id(parent_id);
    match cx.kind(grand) {
        AstKind::AssignmentExpression(a) => a.left.span() == member,
        AstKind::UpdateExpression(_) => true,
        AstKind::UnaryExpression(u) => u.operator.is_delete(),
        AstKind::ForInStatement(f) => f.left.span() == member,
        AstKind::ForOfStatement(f) => f.left.span() == member,
        AstKind::ArrayAssignmentTarget(_)
        | AstKind::ObjectAssignmentTarget(_)
        | AstKind::AssignmentTargetRest(_)
        | AstKind::AssignmentTargetWithDefault(_)
        | AstKind::AssignmentTargetPropertyProperty(_) => true,
        _ => false,
    }
}

/// `no-import-assign`
pub(super) fn import_assign(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits = Vec::new();
        for sid in sc.symbol_ids() {
            if !sc
                .symbol_flags(sid)
                .intersects(SymbolFlags::Import | SymbolFlags::TypeImport)
            {
                continue;
            }
            let name = sc.symbol_name(sid);
            let namespace = matches!(
                first_decl_kind(cx, sid),
                AstKind::ImportNamespaceSpecifier(_)
            );
            for r in sc.get_resolved_references(sid) {
                let span = cx.s.reference_span(r);
                let start = write_node_start(cx, r.node_id(), span);
                let at = Span::new(start, start + 1);
                if r.is_write() {
                    hits.push((at, c.message.replace("{name}", name)));
                } else if namespace && member_written(cx, r.node_id(), span) {
                    hits.push((at, format!("The members of '{name}' are read-only.")));
                }
            }
        }
        hits
    });
}

/// `no-redeclare`
pub(super) fn redeclare(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits = Vec::new();
        for sid in sc.symbol_ids() {
            let node = cx.kind(sc.get_node_id(sc.symbol_scope_id(sid)));
            let visited = matches!(
                node,
                AstKind::Program(_)
                    | AstKind::Function(_)
                    | AstKind::ArrowFunctionExpression(_)
                    | AstKind::BlockStatement(_)
                    | AstKind::ForStatement(_)
                    | AstKind::ForInStatement(_)
                    | AstKind::ForOfStatement(_)
                    | AstKind::SwitchStatement(_)
            ) || (!cx.ts && matches!(node, AstKind::StaticBlock(_)));
            if !visited {
                continue;
            }
            let mut decls = cx.decls(sid);
            // A `var` in a `catch` block is a variable of the function, not the parameter.
            decls.retain(|(_, n)| !matches!(cx.kind(*n), AstKind::CatchParameter(_)));
            if decls.len() < 2 {
                continue;
            }
            if cx.ts {
                // An overload signature is not a declaration of its own.
                decls.retain(
                    |(_, n)| !matches!(cx.kind(*n), AstKind::Function(f) if f.body.is_none()),
                );
                if decls.len() > 1 {
                    decls = merged(cx, decls);
                }
            }
            let name = sc.symbol_name(sid);
            for (sp, _) in decls.into_iter().skip(1) {
                hits.push((sp, c.message.replace("{name}", name)));
            }
        }
        hits
    });
}

/// The declarations TypeScript's declaration merging leaves reported. Interfaces merge with
/// each other, namespaces with each other, and a namespace or interface with a class, a function
/// or an enum; of those, one class, function or enum stands and several are redeclarations.
fn merged(cx: &Cx<'_, '_>, decls: Vec<(Span, NodeId)>) -> Vec<(Span, NodeId)> {
    let tags: Vec<char> = (decls.iter())
        .map(|(_, n)| match cx.kind(*n) {
            AstKind::TSInterfaceDeclaration(_) => 'i',
            AstKind::TSNamespaceDeclaration(_)
            | AstKind::TSExternalModuleDeclaration(_)
            | AstKind::TSGlobalDeclaration(_) => 'm',
            AstKind::Class(_) => 'c',
            AstKind::Function(_) => 'f',
            AstKind::TSEnumDeclaration(_) => 'e',
            _ => '-',
        })
        .collect();
    let all = |allowed: &str| tags.iter().all(|t| allowed.contains(*t));
    if tags.iter().all(|t| *t == 'i') || tags.iter().all(|t| *t == 'm') {
        return Vec::new();
    }
    for (allowed, main) in [("cim", 'c'), ("fm", 'f'), ("em", 'e')] {
        if all(allowed) {
            let mains: Vec<_> = (decls.iter().zip(&tags))
                .filter(|(_, t)| **t == main)
                .map(|(d, _)| *d)
                .collect();
            return if mains.len() == 1 { Vec::new() } else { mains };
        }
    }
    decls
}

fn in_static_initializer(body: &[ClassElement<'_>], at: u32) -> bool {
    body.iter().any(|m| match m {
        ClassElement::StaticBlock(b) => b.span.start <= at && at <= b.span.end,
        ClassElement::PropertyDefinition(p) => {
            p.r#static
                && p.value
                    .as_ref()
                    .is_some_and(|v| v.span().start <= at && at <= v.span().end)
        }
        _ => false,
    })
}

/// Is the reference evaluated while its variable is being initialized (`var a = a`)?
fn evaluated_during_init(
    cx: &Cx<'_, '_>,
    r: &oxc_semantic::Reference,
    sid: SymbolId,
    first: (Span, NodeId),
) -> bool {
    if separate_context(cx, r, sid) {
        return false;
    }
    let loc = cx.s.reference_span(r).end;
    let inside = |s: Span| s.start <= loc && loc <= s.end;
    if let AstKind::Class(class) = cx.kind(first.1) {
        return inside(class.span) && !in_static_initializer(&class.body.body, loc);
    }
    let Some(b) = cx.binding(first.0.start) else {
        return false;
    };
    let nodes = cx.s.nodes();
    for id in nodes.ancestor_ids(b) {
        match cx.kind(id) {
            AstKind::VariableDeclarator(d) => {
                if d.init.as_ref().is_some_and(|e| inside(e.span())) {
                    return true;
                }
                let stmt = nodes.parent_id(nodes.parent_id(id));
                return match cx.kind(stmt) {
                    AstKind::ForInStatement(f) => inside(f.right.span()),
                    AstKind::ForOfStatement(f) => inside(f.right.span()),
                    _ => false,
                };
            }
            AstKind::AssignmentPattern(p) if inside(p.right.span()) => return true,
            AstKind::FormalParameter(p)
                if p.initializer.as_ref().is_some_and(|e| inside(e.span())) =>
            {
                return true;
            }
            AstKind::Function(_)
            | AstKind::ArrowFunctionExpression(_)
            | AstKind::Class(_)
            | AstKind::CatchClause(_)
            | AstKind::ImportDeclaration(_)
            | AstKind::ExportDeclaration(_)
            | AstKind::ExportNamedDeclaration(_) => return false,
            _ => {}
        }
    }
    false
}

/// `no-use-before-define`
pub(super) fn use_before_define(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits = Vec::new();
        for sid in sc.symbol_ids() {
            let decls = cx.decls(sid);
            let first = decls[0];
            let class_decorators: Vec<Span> = match cx.kind(first.1) {
                AstKind::Class(cl) => cl.decorators.iter().map(|d| d.span).collect(),
                _ => Vec::new(),
            };
            for r in sc.get_resolved_references(sid) {
                let span = cx.s.reference_span(r);
                if r.is_type()
                    || (!cx.ts && is_jsx_ref(cx.parent(r.node_id())))
                    || class_decorators.iter().any(|d| contains(*d, span))
                {
                    continue;
                }
                if span.end < first.0.end || evaluated_during_init(cx, r, sid, first) {
                    hits.push((span, c.message.replace("{name}", sc.symbol_name(sid))));
                }
            }
        }
        hits
    });
}

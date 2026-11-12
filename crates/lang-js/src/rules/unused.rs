//! `no-unused-vars` with ESLint's defaults (`vars: all`, `args: after-used`, `caughtErrors:
//! all`) and, in TypeScript files, typescript-eslint's reading of types, `typeof` queries and
//! declaration merging. The checks follow the two rules' own helpers.

use super::Check;
use super::scope::{contains, is_function, run, var_scope};
use crate::semantic::Cx;
use core_rules::RuleContext;
use core_syntax::SyntaxNode;
use oxc_ast::AstKind;
use oxc_ast::ast::{
    ExportDefaultDeclarationKind, Expression, Statement, TSNamespaceDeclarationBody,
};
use oxc_semantic::{NodeId, Reference, SymbolFlags, SymbolId};
use oxc_span::{GetSpan, Span};

fn is_loop(k: AstKind<'_>) -> bool {
    matches!(
        k,
        AstKind::ForStatement(_)
            | AstKind::ForInStatement(_)
            | AstKind::ForOfStatement(_)
            | AstKind::WhileStatement(_)
            | AstKind::DoWhileStatement(_)
    )
}

fn is_export(k: AstKind<'_>) -> bool {
    matches!(
        k,
        AstKind::ExportDeclaration(_)
            | AstKind::ExportNamedDeclaration(_)
            | AstKind::ExportDefaultDeclaration(_)
            | AstKind::ExportAllDeclaration(_)
    )
}

/// Is the identifier at `id` inside `typeof x` in a type?
fn in_type_query(cx: &Cx<'_, '_>, id: NodeId) -> bool {
    let mut at = cx.s.nodes().parent_id(id);
    loop {
        match cx.kind(at) {
            AstKind::TSTypeQuery(_) => return true,
            AstKind::TSQualifiedName(_) => at = cx.s.nodes().parent_id(at),
            _ => return false,
        }
    }
}

/// A read, as typescript-eslint counts them: a type reference reads too.
fn is_read(r: &Reference) -> bool {
    r.is_read() || (r.is_type() && !r.is_write())
}

/// Does the expression statement (or the expression, in a sequence of them) have no use for its
/// value?
fn unused_expression(cx: &Cx<'_, '_>, id: NodeId) -> bool {
    let parent = cx.s.nodes().parent_id(id);
    match cx.kind(parent) {
        AstKind::ExpressionStatement(_) => true,
        AstKind::SequenceExpression(s) => {
            s.expressions.last().is_none_or(|l| l.span() != cx.span(id))
                || unused_expression(cx, parent)
        }
        _ => false,
    }
}

fn in_loop(cx: &Cx<'_, '_>, id: NodeId) -> bool {
    for a in cx.s.nodes().ancestor_ids(id) {
        let k = cx.kind(a);
        if is_function(k) {
            return false;
        }
        if is_loop(k) {
            return true;
        }
    }
    false
}

fn is_statement_or_declaration(k: AstKind<'_>) -> bool {
    let name = format!("{:?}", k.ty());
    name.ends_with("Statement") || name.ends_with("Declaration")
}

/// Can the function around the reference outlive the right-hand side it is in (it is stored or
/// returned, so a call to it later may still read the variable)?
fn storable_function(cx: &Cx<'_, '_>, id: NodeId, rhs: Span) -> bool {
    let Some(function) = (cx.s.nodes().ancestor_ids(id)).find(|a| is_function(cx.kind(*a))) else {
        return false;
    };
    if !contains(rhs, cx.span(function)) {
        return false;
    }
    let (mut node, mut parent) = (function, cx.s.nodes().parent_id(function));
    while contains(rhs, cx.span(parent)) {
        match cx.kind(parent) {
            AstKind::SequenceExpression(s) => {
                if s.expressions
                    .last()
                    .is_none_or(|l| l.span() != cx.span(node))
                {
                    return false;
                }
            }
            AstKind::CallExpression(c) => return c.callee.span() != cx.span(node),
            AstKind::NewExpression(n) => return n.callee.span() != cx.span(node),
            AstKind::AssignmentExpression(_)
            | AstKind::TaggedTemplateExpression(_)
            | AstKind::YieldExpression(_) => return true,
            k if is_statement_or_declaration(k) => return true,
            _ => {}
        }
        node = parent;
        parent = cx.s.nodes().parent_id(parent);
    }
    false
}

/// `a = a + 1` and `a++` as statements read `a` only to write it again.
fn read_for_itself(cx: &Cx<'_, '_>, r: &Reference, rhs: Option<Span>) -> bool {
    if !is_read(r) {
        return false;
    }
    let id = r.node_id();
    let span = cx.span(id);
    let own = match cx.parent(id) {
        AstKind::AssignmentExpression(a) => {
            a.left.span() == span
                && !a.operator.is_logical()
                && unused_expression(cx, cx.s.nodes().parent_id(id))
        }
        AstKind::UpdateExpression(_) => unused_expression(cx, cx.s.nodes().parent_id(id)),
        _ => false,
    };
    own || rhs.is_some_and(|rhs| contains(rhs, span) && !storable_function(cx, id, rhs))
}

/// The right-hand side of the statement `a = ...` the reference writes, while later reads of the
/// variable can only come from it.
fn rhs_node(cx: &Cx<'_, '_>, r: &Reference, sid: SymbolId, prev: Option<Span>) -> Option<Span> {
    let id = r.node_id();
    let span = cx.span(id);
    let sc = cx.s.scoping();
    if let Some(prev) = prev
        && contains(prev, span)
    {
        return Some(prev);
    }
    let later =
        var_scope(cx, r.scope_id()) != var_scope(cx, sc.symbol_scope_id(sid)) || in_loop(cx, id);
    match cx.parent(id) {
        AstKind::AssignmentExpression(a)
            if a.left.span() == span
                && unused_expression(cx, cx.s.nodes().parent_id(id))
                && !later =>
        {
            Some(a.right.span())
        }
        _ => None,
    }
}

/// The node of the parameter list a binding belongs to, when it is one of a function.
fn param_list(cx: &Cx<'_, '_>, binding: NodeId) -> Option<NodeId> {
    for a in cx.s.nodes().ancestor_ids(binding) {
        match cx.kind(a) {
            AstKind::FormalParameters(_) => return Some(a),
            k if is_function(k) => return None,
            _ => {}
        }
    }
    None
}

/// Does the binding take a default (`a = 1`, `{ a = 1 }`), which writes it?
fn has_default(cx: &Cx<'_, '_>, binding: NodeId) -> bool {
    for a in cx.s.nodes().ancestor_ids(binding) {
        match cx.kind(a) {
            AstKind::AssignmentPattern(_) => return true,
            AstKind::FormalParameter(p) => return p.initializer.is_some(),
            AstKind::FormalParameters(_) => return false,
            _ => {}
        }
    }
    false
}

/// Is the body of a `for ... in/of` a single `return` (the loop variable is then used as a
/// guard)?
fn only_returns(body: &oxc_ast::ast::Statement<'_>) -> bool {
    match body {
        oxc_ast::ast::Statement::ReturnStatement(_) => true,
        oxc_ast::ast::Statement::BlockStatement(b) => {
            matches!(
                b.body.as_slice(),
                [oxc_ast::ast::Statement::ReturnStatement(_)]
            )
        }
        _ => false,
    }
}

/// Does the declarator belong to a `for ... in/of` whose body only returns?
fn returns_in_loop(cx: &Cx<'_, '_>, decl: NodeId) -> bool {
    let nodes = cx.s.nodes();
    match cx.kind(nodes.parent_id(nodes.parent_id(decl))) {
        AstKind::ForInStatement(f) => only_returns(&f.body),
        AstKind::ForOfStatement(f) => only_returns(&f.body),
        _ => false,
    }
}

/// Does the list of statements export names by a statement of its own (`export { a }`,
/// `export * from`, `export =`, `export default a`)? Declarations it holds are then not ambient.
fn overriding(body: &[Statement<'_>]) -> bool {
    body.iter().any(|s| match s {
        Statement::ExportNamedDeclaration(_)
        | Statement::ExportFromDeclaration(_)
        | Statement::ExportAllDeclaration(_)
        | Statement::TSExportAssignment(_) => true,
        Statement::ExportDefaultDeclaration(d) => {
            matches!(d.declaration, ExportDefaultDeclarationKind::Identifier(_))
        }
        _ => false,
    })
}

/// Is the declaration in a `.d.ts` file or a `declare` module, where it may be used from outside?
fn ambient(cx: &Cx<'_, '_>, node: NodeId) -> bool {
    let nodes = cx.s.nodes();
    let statement = if matches!(cx.kind(node), AstKind::VariableDeclarator(_)) {
        nodes.parent_id(node)
    } else {
        node
    };
    let container = nodes.parent_id(statement);
    match cx.kind(container) {
        AstKind::Program(p) => cx.definition && !overriding(&p.body),
        AstKind::TSModuleBlock(b) => {
            let declared = cx.definition
                || nodes.ancestor_ids(container).any(|a| match cx.kind(a) {
                    AstKind::TSNamespaceDeclaration(n) => n.declare,
                    AstKind::TSExternalModuleDeclaration(m) => m.declare,
                    AstKind::TSGlobalDeclaration(g) => g.declare,
                    _ => false,
                });
            declared && !overriding(&b.body)
        }
        _ => false,
    }
}

/// Symbols the language makes use of by their declaration alone (typescript-eslint's
/// `eslintUsed`), and the names the rule does not check.
fn skipped(cx: &Cx<'_, '_>, sid: SymbolId, flags: SymbolFlags, node: NodeId, name: Span) -> bool {
    let sc = cx.s.scoping();
    if flags.contains(SymbolFlags::EnumMember) {
        return true;
    }
    let first = cx.kind(node);
    // The name of a function or class expression belongs to its own scope.
    let own_name = match first {
        AstKind::Function(f) => f.r#type == oxc_ast::ast::FunctionType::FunctionExpression,
        AstKind::Class(k) => k.r#type == oxc_ast::ast::ClassType::ClassExpression,
        _ => false,
    };
    if own_name {
        return true;
    }
    match first {
        AstKind::FormalParameter(p) => {
            if p.accessibility.is_some() || p.readonly || p.r#override {
                return true;
            }
        }
        // `[K in T]` declares `K` for the mapped type alone.
        AstKind::TSMappedType(_) => return true,
        // `namespace A.B {}` declares no names of its own.
        AstKind::TSNamespaceDeclaration(n)
            if matches!(
                n.body,
                TSNamespaceDeclarationBody::TSNamespaceDeclaration(_)
            ) || matches!(cx.parent(node), AstKind::TSNamespaceDeclaration(_)) =>
        {
            return true;
        }
        _ => {}
    }
    if cx.ts
        && (cx.decls(sid).iter().any(|d| ambient(cx, d.1))
            || (flags.contains(SymbolFlags::Import) && cx.jsx_pragma(sc.symbol_name(sid))))
    {
        return true;
    }
    if let Some(b) = cx.binding(name.start) {
        let nodes = cx.s.nodes();
        // The parameters of signatures and of setters are not checked.
        let in_params = matches!(
            first,
            AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_)
        );
        if in_params && let Some(list) = param_list(cx, b) {
            let owner = nodes.parent_id(list);
            let setter = matches!(cx.parent(owner), AstKind::MethodDefinition(m) if m.kind.is_set())
                || matches!(cx.parent(owner), AstKind::ObjectProperty(p) if p.kind == oxc_ast::ast::PropertyKind::Set);
            let signature = matches!(
                cx.kind(owner),
                AstKind::TSFunctionType(_)
                    | AstKind::TSConstructorType(_)
                    | AstKind::TSMethodSignature(_)
                    | AstKind::TSCallSignatureDeclaration(_)
                    | AstKind::TSConstructSignatureDeclaration(_)
            ) || matches!(cx.kind(owner), AstKind::Function(f) if f.body.is_none());
            if setter || signature {
                return true;
            }
        }
        // `for (const a of b) return;`
        if matches!(first, AstKind::VariableDeclarator(_)) && returns_in_loop(cx, node) {
            return true;
        }
    }
    let _ = sid;
    let _ = sc;
    false
}

/// Is the symbol exported (its declaration is, or it is a merged type that is)?
fn exported(cx: &Cx<'_, '_>, flags: SymbolFlags, decls: &[(Span, NodeId)]) -> bool {
    decls.iter().any(|(_, n)| {
        let k = cx.kind(*n);
        if matches!(
            k,
            AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_)
        ) {
            return false;
        }
        let at = if matches!(k, AstKind::VariableDeclarator(_)) {
            cx.s.nodes().parent_id(*n)
        } else {
            *n
        };
        let merged = matches!(
            k,
            AstKind::TSTypeAliasDeclaration(_) | AstKind::TSInterfaceDeclaration(_)
        ) && flags.intersects(SymbolFlags::Value);
        is_export(cx.parent(at)) && !merged
    })
}

/// The spans a reference from inside does not count as a use of the variable.
struct Own {
    functions: Vec<Span>,
    types: Vec<Span>,
    modules: Vec<Span>,
    enums: Vec<Span>,
}

fn own_spans(cx: &Cx<'_, '_>, decls: &[(Span, NodeId)]) -> Own {
    let mut own = Own {
        functions: Vec::new(),
        types: Vec::new(),
        modules: Vec::new(),
        enums: Vec::new(),
    };
    for (_, n) in decls {
        match cx.kind(*n) {
            AstKind::Function(f) => own.functions.push(f.span),
            AstKind::VariableDeclarator(d) => {
                if let Some(
                    init @ (Expression::FunctionExpression(_)
                    | Expression::ArrowFunctionExpression(_)),
                ) = &d.init
                {
                    own.functions.push(init.span());
                }
            }
            k @ (AstKind::TSInterfaceDeclaration(_) | AstKind::TSTypeAliasDeclaration(_)) => {
                own.types.push(k.span());
            }
            k @ (AstKind::TSNamespaceDeclaration(_)
            | AstKind::TSExternalModuleDeclaration(_)
            | AstKind::TSGlobalDeclaration(_)) => own.modules.push(k.span()),
            k @ AstKind::TSEnumDeclaration(_) => own.enums.push(k.span()),
            _ => {}
        }
    }
    own
}

/// A variable as ESLint's scope manager has it: a name with its declarations and the references
/// that resolve to it. oxc has one symbol where ESLint has two for a class declaration (inside
/// and outside its body) and for a `var` in a `catch` block named like the parameter.
struct Var<'s> {
    sid: SymbolId,
    flags: SymbolFlags,
    decls: Vec<(Span, NodeId)>,
    refs: Vec<&'s Reference>,
}

fn variables<'s>(cx: &Cx<'s, '_>) -> Vec<Var<'s>> {
    let sc = cx.s.scoping();
    let mut out = Vec::new();
    for sid in sc.symbol_ids() {
        let flags = sc.symbol_flags(sid);
        let decls = cx.decls(sid);
        let mut refs: Vec<&Reference> = sc.get_resolved_references(sid).collect();
        refs.sort_by_key(|r| cx.span(r.node_id()).start);
        if let AstKind::Class(k) = cx.kind(decls[0].1)
            && k.r#type == oxc_ast::ast::ClassType::ClassDeclaration
        {
            refs.retain(|r| !contains(k.span, cx.span(r.node_id())));
        }
        let is_catch = |d: &(Span, NodeId)| matches!(cx.kind(d.1), AstKind::CatchParameter(_));
        if flags.contains(SymbolFlags::CatchVariable)
            && decls.iter().any(is_catch)
            && !decls.iter().all(is_catch)
        {
            let (catch, other): (Vec<_>, Vec<_>) = decls.into_iter().partition(is_catch);
            let clause = cx.span(cx.s.nodes().parent_id(catch[0].1));
            let (inner, outer): (Vec<_>, Vec<_>) = refs
                .into_iter()
                .partition(|r| contains(clause, cx.span(r.node_id())));
            out.push(Var {
                sid,
                flags: SymbolFlags::CatchVariable,
                decls: catch,
                refs: inner,
            });
            out.push(Var {
                sid,
                flags: flags - SymbolFlags::CatchVariable,
                decls: other,
                refs: outer,
            });
        } else {
            out.push(Var {
                sid,
                flags,
                decls,
                refs,
            });
        }
    }
    out
}

/// Does the declaration write the variable (an initializer, a loop variable, a default)?
fn writes_declaration(cx: &Cx<'_, '_>, (name, node): (Span, NodeId)) -> bool {
    match cx.kind(node) {
        AstKind::VariableDeclarator(d) => {
            let nodes = cx.s.nodes();
            d.init.is_some()
                || matches!(
                    cx.kind(nodes.parent_id(nodes.parent_id(node))),
                    AstKind::ForInStatement(_) | AstKind::ForOfStatement(_)
                )
        }
        AstKind::FormalParameter(_) => cx.binding(name.start).is_some_and(|b| has_default(cx, b)),
        _ => false,
    }
}

/// Is the variable read, as ESLint's `isUsedVariable` decides (typescript-eslint's version in
/// TypeScript files; the two agree on JavaScript).
fn is_used(cx: &Cx<'_, '_>, v: &Var<'_>) -> bool {
    let own = own_spans(cx, &v.decls);
    let imported_as_type =
        v.flags.contains(SymbolFlags::TypeImport) && !v.flags.contains(SymbolFlags::Import);
    let mut rhs: Option<Span> = None;
    for &r in &v.refs {
        let id = r.node_id();
        let span = cx.span(id);
        let for_itself = read_for_itself(cx, r, rhs);
        rhs = rhs_node(cx, r, v.sid, rhs);
        let inside = |spans: &[Span]| spans.iter().any(|s| contains(*s, span));
        let type_only = cx.ts && !imported_as_type && in_type_query(cx, id);
        if is_read(r)
            && !for_itself
            && !type_only
            && !inside(&own.functions)
            && !inside(&own.types)
            && !inside(&own.modules)
            && !inside(&own.enums)
        {
            return true;
        }
        // `for (a of b) return;` with the loop variable an existing one.
        if let AstKind::ForInStatement(f) = cx.parent(id)
            && only_returns(&f.body)
        {
            return true;
        }
        if let AstKind::ForOfStatement(f) = cx.parent(id)
            && only_returns(&f.body)
        {
            return true;
        }
    }
    false
}

/// The parameters of the function a (simple) parameter belongs to that come after it.
fn after_last_used(cx: &Cx<'_, '_>, sid: SymbolId, start: u32, list: NodeId) -> bool {
    let sc = cx.s.scoping();
    for other in sc.iter_bindings_in(sc.symbol_scope_id(sid)) {
        let (name, node) = cx.decls(other)[0];
        if name.start <= start
            || !matches!(
                cx.kind(node),
                AstKind::FormalParameter(_) | AstKind::FormalParameterRest(_)
            )
        {
            continue;
        }
        let Some(b) = cx.binding(name.start) else {
            continue;
        };
        if param_list(cx, b) != Some(list) {
            continue;
        }
        let referenced = sc
            .get_resolved_references(other)
            .any(|r| !(cx.ts && in_type_query(cx, r.node_id())));
        let forced = skipped(cx, other, sc.symbol_flags(other), node, name);
        if referenced || forced || has_default(cx, b) {
            return false;
        }
    }
    true
}

/// `no-unused-vars`
pub(super) fn unused_vars(c: &Check, _n: &SyntaxNode, ctx: &mut RuleContext<'_>) {
    run(ctx, |cx| {
        let sc = cx.s.scoping();
        let mut hits = Vec::new();
        for v in variables(cx) {
            let (name, node) = v.decls[0];
            if skipped(cx, v.sid, v.flags, node, name) || exported(cx, v.flags, &v.decls) {
                continue;
            }
            let kind = cx.kind(node);
            // A plain parameter only counts when no later one is used.
            if let AstKind::FormalParameter(p) = kind
                && p.initializer.is_none()
                && let Some(b) = cx.binding(name.start)
                && matches!(cx.parent(b), AstKind::FormalParameter(_))
                && let Some(list) = param_list(cx, b)
                && !after_last_used(cx, v.sid, name.start, list)
            {
                continue;
            }
            if is_used(cx, &v) {
                continue;
            }
            // `import { T } from "t"; type U = typeof T;` and `T` as a type only: not unused.
            let queried = cx.ts && v.refs.iter().any(|r| in_type_query(cx, r.node_id()));
            if queried
                && v.flags
                    .intersects(SymbolFlags::Import | SymbolFlags::TypeImport)
            {
                continue;
            }
            let home = var_scope(cx, sc.symbol_scope_id(v.sid));
            let writes = (v.refs.iter())
                .filter(|r| r.is_write() && var_scope(cx, r.scope_id()) == home)
                .map(|r| cx.s.reference_span(r))
                .chain(
                    v.decls
                        .iter()
                        .filter(|d| writes_declaration(cx, **d))
                        .map(|d| d.0),
                );
            let write = writes.max_by_key(|s| s.start);
            let assigned = write.is_some() || v.refs.iter().any(|r| r.is_write());
            let action = if queried {
                "assigned a value but only used as a type"
            } else if assigned {
                "assigned a value but never used"
            } else {
                "defined but never used"
            };
            hits.push((
                write.unwrap_or(name),
                c.message
                    .replace("{name}", sc.symbol_name(v.sid))
                    .replace("{action}", action),
            ));
        }
        hits
    });
}

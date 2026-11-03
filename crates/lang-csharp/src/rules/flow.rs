//! Rules over the flow layer: unreachable code, unused variables, dead stores.

use super::{BODY_KINDS, kind, range_of};
use core_config::Level;
use core_flow::{LocalKind, OpFlags, OpKind, dead_stores};
use core_rules::{
    BodyContext, Category, Edit, Fix, FixKind, Layer, Opt, OptionDecl, Rule, RuleContext, RuleMeta,
};
use core_syntax::{SyntaxNode, TextRange};
use std::collections::BTreeSet;

pub struct UnreachableCode;

static UNREACHABLE: RuleMeta = RuleMeta {
    id: "unreachable-code",
    category: Category::Suspicious,
    description: "A statement that can never run. At `fast` depth this is the statement after a \
                  `return`, `throw`, `break`, `continue` or `goto`; with the flow layer it also \
                  covers constant conditions and calls that do not return (`does_not_return` in \
                  `semantics:`). Only the first statement of a run is reported. Removing it is \
                  offered as an editor quick fix only, because unreachable code usually signals \
                  a bug. Off by default: the compiler already reports unreachable code (CS0162) and the SDK analyzers add IDE0035.",
    example: "return x;\nLog(x);   // never runs",
    default_level: Level::Off,
    fix: Some(FixKind::Suggestion),
    requires: &[],
    uses: &[Layer::Flow],
    options: &[OptionDecl {
        name: "non_returning_calls",
        default: Opt::Bool(true),
        description: "Treat the code after a call that never returns (`[DoesNotReturn]`,                       `does_not_return` in `semantics:`) as unreachable. The compiler does not,                       so turn this off to report exactly what it reports (CS0162).",
    }],
};

impl Rule for UnreachableCode {
    fn meta(&self) -> &'static RuleMeta {
        &UNREACHABLE
    }

    fn check_body(&self, body: &BodyContext<'_>, ctx: &mut RuleContext<'_>) {
        let grammar = ctx.file.grammar;
        let is_statement = |n: &SyntaxNode| grammar.has_tag(n.kind(), "statement");
        let report = |ctx: &mut RuleContext<'_>, statement: &SyntaxNode| {
            let range = core_rules::tree::significant_range(statement, grammar);
            ctx.report(range, "unreachable code").fix(Fix::new(
                "Remove the unreachable statement",
                FixKind::Suggestion,
                vec![Edit::delete(range)],
            ));
        };
        let Some(flow) = &body.flow else {
            for statement in syntactic(body.node, is_statement) {
                report(ctx, &statement);
            }
            return;
        };
        let reach = if ctx.options.bool("non_returning_calls") {
            flow.reachability()
        } else {
            flow.compiler_reachability()
        };
        // A statement is dead when none of its operations can run: one that only partly runs
        // (`return c ? throw e : 0;`) is not.
        let mut dead: BTreeSet<(u32, u32)> = BTreeSet::new();
        let mut live: BTreeSet<(u32, u32)> = BTreeSet::new();
        for (b, block) in flow.graph.blocks.iter().enumerate() {
            for (i, op) in block.ops.iter().enumerate() {
                if op.stmt_len > 0 {
                    let set = if reach.op_reachable(b, i) {
                        &mut live
                    } else {
                        &mut dead
                    };
                    set.insert((op.stmt_start, op.stmt_len));
                }
            }
        }
        // Nor is a statement around code that runs (the condition of a `do` that always returns).
        dead.retain(|&(start, len)| {
            !live
                .iter()
                .any(|&(s, l)| s >= start && s + l <= start + len)
        });
        let own = core_rules::tree::significant_range(body.node, grammar);
        dead.remove(&(own.start().into(), own.len().into()));
        let is_dead = |n: &SyntaxNode| {
            let r = core_rules::tree::significant_range(n, grammar);
            dead.contains(&(r.start().into(), r.len().into()))
        };
        for statement in body
            .node
            .descendants()
            .filter(|n| is_statement(n) && is_dead(n))
        {
            let nested = statement
                .ancestors()
                .skip(1)
                .any(|a| is_statement(&a) && is_dead(&a));
            let follows = statement
                .prev_sibling()
                .is_some_and(|p| is_statement(&p) && is_dead(&p));
            // The compiler does not report an unreachable `throw`: it is a defensive guard.
            if !nested && !follows && statement.kind() != kind::THROW_STATEMENT {
                report(ctx, &statement);
            }
        }
    }
}

/// Statements right after one that always leaves the block, found without flow information.
fn syntactic(body: &SyntaxNode, is_statement: impl Fn(&SyntaxNode) -> bool) -> Vec<SyntaxNode> {
    let jumps = [
        kind::RETURN_STATEMENT,
        kind::THROW_STATEMENT,
        kind::BREAK_STATEMENT,
        kind::CONTINUE_STATEMENT,
        kind::GOTO_STATEMENT,
    ];
    let mut out = Vec::new();
    for list in body
        .descendants()
        .filter(|n| matches!(n.kind(), kind::BLOCK | kind::SWITCH_SECTION))
    {
        let own = list.ancestors().find(|a| BODY_KINDS.contains(&a.kind()));
        if own.as_ref() != Some(body) {
            continue;
        }
        let mut statements = list.children().filter(|n| is_statement(n));
        while let Some(s) = statements.next() {
            if jumps.contains(&s.kind())
                && let Some(next) = statements.next()
                && !matches!(
                    next.kind(),
                    kind::LABELED_STATEMENT
                        | kind::LOCAL_FUNCTION_STATEMENT
                        | kind::THROW_STATEMENT
                        | kind::EMPTY_STATEMENT
                )
            {
                out.push(next);
            }
        }
    }
    out
}

pub struct UnusedVariable;

static UNUSED_VARIABLE: RuleMeta = RuleMeta {
    id: "unused-variable",
    category: Category::Cleanup,
    description: "A local variable or catch variable that is never used (the compiler's CS0168), \
                  or is only assigned constants and never read (CS0219). Variables captured by \
                  lambdas or local functions count as used. Off by default: the compiler already warns (CS0168, CS0219).",
    example: "int total = 0;   // never read\ntry { Run(); } catch (Exception e) { }   // e is never used",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[Layer::Flow],
    uses: &[],
    options: &[],
};

impl Rule for UnusedVariable {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_VARIABLE
    }

    fn check_body(&self, body: &BodyContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(flow) = &body.flow else { return };
        let graph = flow.graph;
        for (index, local) in graph.locals.iter().enumerate() {
            if !matches!(local.kind(), LocalKind::Declaration | LocalKind::Catch)
                || !local.tracked()
            {
                continue;
            }
            let (mut reads, mut writes, mut non_constant) = (0, 0, 0);
            for op in graph.blocks.iter().flat_map(|b| &b.ops) {
                if op.local() != Some(index) {
                    continue;
                }
                match op.kind() {
                    OpKind::Read | OpKind::ReadWrite => reads += 1,
                    OpKind::Write => {
                        writes += 1;
                        non_constant += i32::from(!op.has(OpFlags::CONSTANT));
                    }
                    _ => {}
                }
            }
            let message = match (reads, writes, non_constant) {
                (0, 0, _) => format!("variable `{}` is declared but never used", local.name),
                (0, _, 0) => format!(
                    "variable `{}` is assigned but its value is never used",
                    local.name
                ),
                _ => continue,
            };
            let range = range_of(local.start, local.len);
            let diagnostic = ctx.report(range, message);
            if let Some(edit) = removal(body, local.kind(), range) {
                diagnostic.fix(Fix::new(
                    format!("Remove `{}`", local.name),
                    FixKind::Safe,
                    vec![edit],
                ));
            }
        }
    }
}

/// The edit that removes the variable: its whole declaration statement when it declares nothing
/// else, or a catch variable's name.
fn removal(body: &BodyContext<'_>, local: LocalKind, name: TextRange) -> Option<Edit> {
    let file = body.file;
    let at = body.node.covering_element(name);
    let node = at.as_node().cloned().or_else(|| at.as_token()?.parent())?;
    if local == LocalKind::Catch {
        let text = &file.text[..usize::from(name.start())];
        let start = text.trim_end().len();
        return Some(Edit::delete(range_of(
            start as u32,
            u32::from(name.end()) - start as u32,
        )));
    }
    let statement = node
        .ancestors()
        .find(|n| n.kind() == kind::LOCAL_DECLARATION_STATEMENT)?;
    let declarators = statement
        .descendants()
        .filter(|n| n.kind() == kind::VARIABLE_DECLARATOR)
        .count();
    (declarators == 1).then(|| {
        Edit::delete(core_rules::tree::significant_range(
            &statement,
            file.grammar,
        ))
    })
}

pub struct UnusedAssignment;

static UNUSED_ASSIGNMENT: RuleMeta = RuleMeta {
    id: "unused-assignment",
    category: Category::Cleanup,
    description: "A value is stored in a local variable and no path reads it before it is \
                  overwritten or the variable goes out of scope (a dead store). Variables that \
                  are captured or whose address is taken are not checked; a variable that is \
                  never read at all is reported by `unused-variable` instead. Overlaps the SDK analyzer IDE0059 (unnecessary assignment).",
    example: "int n = 0;\nn = Count();   // the 0 is never read",
    default_level: Level::Off,
    fix: None,
    requires: &[Layer::Flow],
    uses: &[],
    options: &[],
};

impl Rule for UnusedAssignment {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_ASSIGNMENT
    }

    fn check_body(&self, body: &BodyContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(flow) = &body.flow else { return };
        let graph = flow.graph;
        let reach = flow.reachability();
        for store in dead_stores(graph, &reach) {
            let op = &graph.blocks[store.block].ops[store.op];
            let Some(index) = op.local() else { continue };
            let local = &graph.locals[index];
            let ever_read = graph.blocks.iter().flat_map(|b| &b.ops).any(|o| {
                o.local() == Some(index) && matches!(o.kind(), OpKind::Read | OpKind::ReadWrite)
            });
            if ever_read {
                ctx.report(
                    range_of(op.start, op.len),
                    format!("the value assigned to `{}` is never read", local.name),
                );
            }
        }
    }
}

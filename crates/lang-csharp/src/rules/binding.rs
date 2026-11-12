//! Rules over the binding, type and value layers, and the project-scope rule.

use super::range_of;
use core_rules::rule;
use core_rules::{
    Check, Edit, FileContext, Opt, OptionDecl, ProjectContext, ProjectFacts, RuleContext,
};
use core_semantics::{Accessibility, DeclarationFlags, DeclarationKind};
use core_syntax::{TextRange, TextSize};
use std::collections::BTreeSet;

rule! {
    pub RemoveUnusedUsings {
        id: "remove-unused-usings",
        category: Cleanup,
        default: Off,
        fix: Safe,
        description: "A `using` directive that nothing in the file needs (the compiler's hidden \
                  CS8019, IDE0005 in the SDK analyzers). Not reported in a file that has compiler errors or `#if` regions, \
                  because unresolved or inactive code could be the one using it.",
        example: "using System.Text;   // nothing from System.Text is used",
        requires: [Binding],
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(view) = &file.semantics else { return };
        for span in &view.data.answers.unused_imports {
            let range = range_of(span.start, span.len);
            let edit = Edit::delete(whole_line(file.text, range));
            ctx.report_fix(
                range,
                "unnecessary using directive",
                ("Remove the unnecessary using", vec![edit]),
            );
        }
    }
}

/// The range plus the indentation before it and the line break after it, when the directive has
/// its line to itself.
fn whole_line(text: &str, range: TextRange) -> TextRange {
    let (start, end) = (usize::from(range.start()), usize::from(range.end()));
    let line_start = text[..start].rfind('\n').map_or(0, |i| i + 1);
    let rest = &text[end..];
    let line_end = rest.find('\n').map_or(rest.len(), |i| i + 1);
    let alone = text[line_start..start].trim().is_empty() && rest[..line_end].trim().is_empty();
    if alone {
        TextRange::new(
            TextSize::from(line_start as u32),
            TextSize::from((end + line_end) as u32),
        )
    } else {
        range
    }
}

rule! {
    pub PossibleNullDereference {
        id: "possible-null-dereference",
        category: Correctness,
        default: Off,
        description: "A member or element access on an expression the compiler's nullable analysis \
                  says may be null. Only code under `#nullable enable` is checked. Off by \
                  default: the compiler reports the same thing as CS8602.",
        example: "string? s = Find();\nint n = s.Length;   // s may be null",
        requires: [Values],
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(view) = &file.semantics else { return };
        for span in &view.data.answers.maybe_null {
            ctx.report(
                range_of(span.start, span.len),
                "this expression may be null here",
            );
        }
    }
}

const DECLARED: &str = "declared";
const USED: &str = "used";

rule! {
    pub UnusedPrivateMember {
        id: "unused-private-member",
        category: Cleanup,
        default: Warn,
        description: "A private field, method, property or event that no file of the project uses \
                  (`analysis: deep`). Members with attributes, overrides, interface \
                  implementations, `extern` and `partial` methods are left alone, and so are the private                   methods named in `reflection_methods` of a type derived from one of                   `reflection_base_types` (a framework calls them by reflection; the `csharp/unity`                   preset lists Unity's). Overlaps the SDK analyzers IDE0051 (unused private member) and IDE0052 (unread private member).",
        example: "class A { private int cache; }   // cache is never used",
        requires: [Types, Project],
        options: [
            OptionDecl {
                name: "reflection_base_types",
                default: Opt::List(&[]),
                check: Check::Any,
                description: "Names of base classes whose derived types are called by a framework                           through reflection (with `reflection_methods`). A base is matched by its                           simple name anywhere up the inheritance chain, also when the project                           does not load its assembly.",
            },
            OptionDecl {
                name: "reflection_methods",
                default: Opt::List(&[]),
                check: Check::Any,
                description: "Names of private methods that count as used in a type derived from                           one of `reflection_base_types`.",
            },
        ],
    }

    fn collect(&self, file: &FileContext<'_>, facts: &mut ProjectFacts) {
        let Some(view) = &file.semantics else { return };
        let skipped = DeclarationFlags::OVERRIDE
            | DeclarationFlags::IMPLEMENTS
            | DeclarationFlags::EXPLICIT
            | DeclarationFlags::HAS_ATTRIBUTES
            | DeclarationFlags::EXTERN_OR_PARTIAL;
        for d in &view.data.answers.declarations {
            if d.accessibility() == Accessibility::Private
                && d.kind() != DeclarationKind::Type
                && d.flags & skipped == 0
                && d.name != "Main"
            {
                facts.add(
                    range_of(d.start, d.len),
                    &d.symbol,
                    format!("{DECLARED}:{}:{}", d.name, d.bases),
                );
            }
        }
        for r in &view.data.answers.references {
            facts.add(range_of(r.start, r.len), &r.symbol, USED);
        }
    }

    fn finish(&self, facts: &ProjectFacts, ctx: &mut ProjectContext<'_>) {
        let used: BTreeSet<&str> = facts
            .of_rule("unused-private-member")
            .filter(|f| f.value == USED)
            .map(|f| f.key.as_str())
            .collect();
        let (types, methods) = (
            ctx.options.list("reflection_base_types"),
            ctx.options.list("reflection_methods"),
        );
        for f in facts.of_rule("unused-private-member") {
            if let Some(rest) = f.value.strip_prefix("declared:")
                && !used.contains(f.key.as_str())
                && let Some((name, bases)) = rest.split_once(':')
                && !(methods.contains(&name) && bases.split(',').any(|b| types.contains(&b)))
            {
                ctx.report(
                    &f.file,
                    f.range,
                    format!("private member `{name}` is never used"),
                );
            }
        }
    }
}

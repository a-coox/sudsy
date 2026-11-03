//! Rules over the binding, type and value layers, and the project-scope rule.

use super::range_of;
use core_config::Level;
use core_rules::{
    Category, Edit, FileContext, Fix, FixKind, Layer, ProjectContext, ProjectFacts, Rule,
    RuleContext, RuleMeta,
};
use core_semantics::{Accessibility, DeclarationFlags, DeclarationKind};
use core_syntax::{TextRange, TextSize};
use std::collections::BTreeSet;

pub struct RemoveUnusedUsings;

static UNUSED_USINGS: RuleMeta = RuleMeta {
    id: "remove-unused-usings",
    category: Category::Cleanup,
    description: "A `using` directive that nothing in the file needs (the compiler's hidden \
                  CS8019). Not reported in a file that has compiler errors or `#if` regions, \
                  because unresolved or inactive code could be the one using it.",
    example: "using System.Text;   // nothing from System.Text is used",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[Layer::Binding],
    uses: &[],
    options: &[],
};

impl Rule for RemoveUnusedUsings {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_USINGS
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(view) = &file.semantics else { return };
        for span in &view.data.answers.unused_imports {
            let range = range_of(span.start, span.len);
            ctx.report(range, "unnecessary using directive")
                .fix(Fix::new(
                    "Remove the unnecessary using",
                    FixKind::Safe,
                    vec![Edit::delete(whole_line(file.text, range))],
                ));
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

pub struct PossibleNullDereference;

static NULL_DEREF: RuleMeta = RuleMeta {
    id: "possible-null-dereference",
    category: Category::Correctness,
    description: "A member or element access on an expression the compiler's nullable analysis \
                  says may be null. Only code under `#nullable enable` is checked. Off by \
                  default: the compiler reports the same thing as CS8602.",
    example: "string? s = Find();\nint n = s.Length;   // s may be null",
    default_level: Level::Off,
    fix: None,
    requires: &[Layer::Values],
    uses: &[],
    options: &[],
};

impl Rule for PossibleNullDereference {
    fn meta(&self) -> &'static RuleMeta {
        &NULL_DEREF
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

pub struct UnusedPrivateMember;

static UNUSED_PRIVATE: RuleMeta = RuleMeta {
    id: "unused-private-member",
    category: Category::Cleanup,
    description: "A private field, method, property or event that no file of the project uses \
                  (`analysis: deep`). Members with attributes, overrides, interface \
                  implementations, `extern` and `partial` methods are left alone.",
    example: "class A { private int cache; }   // cache is never used",
    default_level: Level::Warn,
    fix: None,
    requires: &[Layer::Types, Layer::Project],
    uses: &[],
    options: &[],
};

const DECLARED: &str = "declared";
const USED: &str = "used";

impl Rule for UnusedPrivateMember {
    fn meta(&self) -> &'static RuleMeta {
        &UNUSED_PRIVATE
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
                    format!("{DECLARED}:{}", d.name),
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
        for f in facts.of_rule("unused-private-member") {
            if let Some(name) = f.value.strip_prefix("declared:")
                && !used.contains(f.key.as_str())
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

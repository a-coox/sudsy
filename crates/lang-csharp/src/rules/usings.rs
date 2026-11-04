use super::kind;
use crate::using_sort::{self, Compare, Order};
use core_config::Level;
use core_rules::tree::{code_start, entry_range, reorder_edits, significant_range};
use core_rules::{Category, Check, Fix, FixKind, Opt, OptionDecl, Rule, RuleContext, RuleMeta};
use core_syntax::{SyntaxKind, SyntaxNode, TextRange};

pub struct UsingOrder;

static META: RuleMeta = RuleMeta {
    id: "using-order",
    category: Category::Cleanup,
    description: "`using` directives of a file or namespace are out of order: System first \
                  (`system_first`), names in alphabetical order (`compare`), and with the options \
                  `static_position` and `alias_position` set to `after`, `using static` \
                  directives after the plain ones and aliases after those, each group sorted. \
                  `global using` and `using unsafe` stay where they are, and a list restarts at \
                  a directive that has a preprocessor directive in front of it (the fix leaves \
                  such lists alone). Comments on lines of their own stay where they are; a \
                  comment after a directive moves with it. Overlaps the SDK analyzers' \
                  `dotnet_sort_system_directives_first` .editorconfig key.",
    example: "using Zeta;\nusing Alpha;   // Alpha goes first",
    default_level: Level::Off,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[
        OptionDecl {
            name: "system_first",
            default: Opt::Bool(true),
            check: Check::Any,
            description: "Put `System` and `System.*` before the other namespaces.",
        },
        OptionDecl {
            name: "compare",
            default: Opt::Str("ordinal"),
            check: Check::OneOf(&["ordinal", "culture", "culture_ignore_case"]),
            description: "`ordinal` compares code points; `culture` ignores case (and puts                           punctuation before digits before letters, a lower-case name before                           an upper-case one that is otherwise equal); `culture_ignore_case`                           leaves names that differ only in case in their order.",
        },
        OptionDecl {
            name: "static_position",
            default: Opt::Str("keep"),
            check: Check::OneOf(&["keep", "after"]),
            description: "`after`: `using static` directives come after the plain usings, \
                          sorted (System ones first with `system_first`); `keep`: they stay \
                          where they are.",
        },
        OptionDecl {
            name: "alias_position",
            default: Opt::Str("keep"),
            check: Check::OneOf(&["keep", "after"]),
            description: "`after`: alias directives come after all other usings, sorted by \
                          alias name; `keep`: they stay where they are.",
        },
    ],
};

fn order_of(ctx: &RuleContext<'_>) -> Order {
    Order {
        system_first: ctx.options.bool("system_first"),
        compare: match ctx.options.string("compare") {
            "culture" => Compare::Culture,
            "culture_ignore_case" => Compare::CultureIgnoreCase,
            _ => Compare::Ordinal,
        },
        statics_after: ctx.options.string("static_position") == "after",
        aliases_after: ctx.options.string("alias_position") == "after",
    }
}

impl Rule for UsingOrder {
    fn meta(&self) -> &'static RuleMeta {
        &META
    }

    fn kinds(&self) -> &[SyntaxKind] {
        &[
            kind::COMPILATION_UNIT,
            kind::NAMESPACE_DECLARATION,
            kind::FILE_SCOPED_NAMESPACE_DECLARATION,
        ]
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let Some(usings) = using_sort::usings_of(node) else {
            return;
        };
        let (text, g) = (ctx.file.text, ctx.file.grammar);
        let entries = using_sort::entries(&usings, text, g);
        let order = order_of(ctx);
        let found = using_sort::misplaced_entries(&entries, &order);
        let Some((_, directives)) = using_sort::list_span(&entries, text, g) else {
            return;
        };
        let sorted = using_sort::sorted(&entries, &order);
        let ranges: Vec<_> = entries.iter().map(|e| entry_range(&e.node, g)).collect();
        let mut fix = (!directives && sorted.iter().enumerate().any(|(i, &s)| i != s)).then(|| {
            Fix::new(
                "Sort the using directives",
                FixKind::Safe,
                reorder_edits(text, &ranges, &sorted),
            )
        });
        for m in found {
            let node = &entries[m.at].node;
            let range = TextRange::new(code_start(node, g), significant_range(node, g).end());
            let diagnostic = ctx.report(range, &m.message);
            if let Some(fix) = fix.take() {
                diagnostic.fix(fix);
            }
        }
    }
}

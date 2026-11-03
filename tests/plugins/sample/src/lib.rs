//! A sample rule plugin, built to Wasm by the plugin host's tests. Three rules, one per kind of
//! thing a plugin can do: a node-scope rule with options and a fix, a body-scope rule on the
//! flow graph, and a project-scope rule.

use plugin_sdk::tree::significant_range;
use plugin_sdk::{
    BodyContext, Category, Edit, FileContext, Fix, FixKind, Layer, Level, Opt, OptionDecl,
    ProjectContext, ProjectFacts, Rule, RuleContext, RuleMeta, SyntaxKind, SyntaxNode, kind_of,
};
use std::collections::HashMap;
use std::sync::OnceLock;

/// `"debug": true` becomes `"debug": false`; the key is an option.
pub struct NoDebugFlag;

static NO_DEBUG_FLAG: RuleMeta = RuleMeta {
    id: "no-debug-flag",
    category: Category::Suspicious,
    description: "A boolean flag that must not be switched on.",
    example: "{ \"debug\": true }   // reported; fixed to false",
    default_level: Level::Warn,
    fix: Some(FixKind::Safe),
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "key",
        default: Opt::Str("debug"),
        description: "The member that must not be true.",
    }],
};

impl Rule for NoDebugFlag {
    fn meta(&self) -> &'static RuleMeta {
        &NO_DEBUG_FLAG
    }

    fn kinds(&self) -> &[SyntaxKind] {
        static KINDS: OnceLock<[SyntaxKind; 1]> = OnceLock::new();
        KINDS.get_or_init(|| [kind_of("json", "Member")])
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let key = format!("\"{}\"", ctx.options.string("key"));
        let mut parts = node
            .children_with_tokens()
            .filter(|e| !ctx.file.grammar.is_trivia(e.kind()));
        let (Some(name), _colon, Some(value)) = (parts.next(), parts.next(), parts.next()) else {
            return;
        };
        let value = value.into_node();
        let (file, grammar) = (ctx.file.text, ctx.file.grammar);
        let text_of = |n: &SyntaxNode| {
            let r = significant_range(n, grammar);
            &file[usize::from(r.start())..usize::from(r.end())]
        };
        if name.as_token().is_some_and(|t| t.text() == key)
            && let Some(value) = value
            && text_of(&value) == "true"
        {
            let range = significant_range(&value, ctx.file.grammar);
            ctx.report(
                significant_range(node, ctx.file.grammar),
                format!("{key} is on"),
            )
            .fix(Fix::new(
                "Switch it off",
                FixKind::Safe,
                vec![Edit::replace(range, "false")],
            ));
        }
    }
}

/// Methods with many branches in their flow graph.
pub struct BranchyBody;

static BRANCHY_BODY: RuleMeta = RuleMeta {
    id: "branchy-body",
    category: Category::Performance,
    description: "A body whose flow graph has more branching blocks than the limit.",
    example: "void F(int a) { if (a > 1) { } if (a > 2) { } }   // with max_branches: 1",
    default_level: Level::Warn,
    fix: None,
    requires: &[Layer::Flow],
    uses: &[],
    options: &[OptionDecl {
        name: "max_branches",
        default: Opt::Int(3),
        description: "The most blocks that may branch.",
    }],
};

impl Rule for BranchyBody {
    fn meta(&self) -> &'static RuleMeta {
        &BRANCHY_BODY
    }

    fn check_body(&self, body: &BodyContext<'_>, ctx: &mut RuleContext<'_>) {
        let Some(flow) = &body.flow else {
            return;
        };
        let branches = flow
            .graph
            .blocks
            .iter()
            .filter(|b| b.succ.len() > 1)
            .count();
        let max = usize::try_from(ctx.options.int("max_branches")).unwrap_or(0);
        if branches > max {
            let range = significant_range(body.node, body.file.grammar);
            ctx.report(
                range,
                format!("{branches} branching blocks; the limit is {max}"),
            );
        }
    }
}

/// The same `"name"` value in two JSON files of the project.
pub struct DuplicateName;

static DUPLICATE_NAME: RuleMeta = RuleMeta {
    id: "duplicate-name",
    category: Category::Correctness,
    description: "Two objects in the project have the same \"name\".",
    example: "a.json: { \"name\": \"x\" }   b.json: { \"name\": \"x\" }   // reported in b.json",
    default_level: Level::Warn,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for DuplicateName {
    fn meta(&self) -> &'static RuleMeta {
        &DUPLICATE_NAME
    }

    fn collect(&self, file: &FileContext<'_>, facts: &mut ProjectFacts) {
        let member = kind_of("json", "Member");
        for node in file.root.descendants().filter(|n| n.kind() == member) {
            let text = node.text().to_string();
            if let Some(value) = text
                .strip_prefix("\"name\"")
                .and_then(|r| r.trim_start().strip_prefix(':'))
            {
                facts.add(significant_range(&node, file.grammar), "name", value.trim());
            }
        }
    }

    fn finish(&self, facts: &ProjectFacts, ctx: &mut ProjectContext<'_>) {
        let mut seen: HashMap<&str, usize> = HashMap::new();
        for fact in facts.of_rule("duplicate-name") {
            let count = seen.entry(fact.value.as_str()).or_default();
            *count += 1;
            if *count > 1 {
                ctx.report(
                    &fact.file,
                    fact.range,
                    format!("the name {} is used twice", fact.value),
                );
            }
        }
    }
}

/// Every `TODO` in the text: a file-scope rule that is cheap to run, so that running it in a
/// plugin instead of built in measures the plugin boundary (the tree and text copied into the
/// plugin, the call) rather than the rule.
pub struct TodoMarker;

static TODO_MARKER: RuleMeta = RuleMeta {
    id: "todo-marker",
    category: Category::Style,
    description: "A TODO left in the file.",
    example: "// TODO: remove",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[],
};

impl Rule for TodoMarker {
    fn meta(&self) -> &'static RuleMeta {
        &TODO_MARKER
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        for (at, _) in file.text.match_indices("TODO") {
            let start = plugin_sdk::TextSize::from(at as u32);
            ctx.report(
                plugin_sdk::TextRange::at(start, 4.into()),
                "a TODO is left here",
            );
        }
    }
}

/// Misbehaves on purpose, for the host's tests of its limits. Off unless the config turns it on.
pub struct Misbehave;

static MISBEHAVE: RuleMeta = RuleMeta {
    id: "misbehave",
    category: Category::Correctness,
    description: "Test fixture: traps, spins or reports a span outside the file, as `mode` says.",
    example: "mode: trap | spin | span",
    default_level: Level::Off,
    fix: None,
    requires: &[],
    uses: &[],
    options: &[OptionDecl {
        name: "mode",
        default: Opt::Str("trap"),
        description: "trap | spin | span",
    }],
};

impl Rule for Misbehave {
    fn meta(&self) -> &'static RuleMeta {
        &MISBEHAVE
    }

    fn check_file(&self, file: &FileContext<'_>, ctx: &mut RuleContext<'_>) {
        match ctx.options.string("mode") {
            "trap" => panic!("misbehave: on purpose"),
            "spin" => {
                let mut n = 0u64;
                loop {
                    n = std::hint::black_box(n.wrapping_add(1));
                }
            }
            _ => {
                let end = file.text.len() as u32 + 10;
                ctx.report(
                    plugin_sdk::TextRange::new(0.into(), end.into()),
                    "past the end",
                );
            }
        }
    }
}

plugin_sdk::export_plugin!(
    name: "sample",
    rules: [
        ("json", &NoDebugFlag),
        ("csharp", &BranchyBody),
        ("json", &DuplicateName),
        ("json", &Misbehave),
        ("*", &TodoMarker)
    ]
);

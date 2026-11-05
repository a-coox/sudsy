//! `unknown-property` and `invalid-property-value`, both read from `data/properties.txt`.

use crate::generated::kind;
use core_config::suggest;
use core_rules::{Check, Opt, OptionDecl, RuleContext, rule};
use core_syntax::{SyntaxNode, SyntaxToken};
use std::collections::HashMap;
use std::sync::OnceLock;

const SEVERITIES: &[&str] = &[
    "none",
    "silent",
    "suggestion",
    "warning",
    "error",
    "refactoring",
];

/// What one line of the data file allows.
#[derive(Default)]
struct Spec {
    bool: bool,
    int: bool,
    any: bool,
    sev: bool,
    list: bool,
    words: Vec<String>,
}

struct Table {
    exact: HashMap<String, Spec>,
    /// `(prefix, suffix, spec)` for the names with a `*`.
    wild: Vec<(String, String, Spec)>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table = Table {
            exact: HashMap::new(),
            wild: Vec::new(),
        };
        let data = include_str!("../../data/properties.txt");
        for line in data
            .lines()
            .filter(|l| !l.starts_with('#') && !l.is_empty())
        {
            let (name, words) = line
                .split_once(':')
                .expect("data/properties.txt: `name: words`");
            let mut spec = Spec::default();
            for word in words.split_whitespace() {
                match word {
                    "bool" => spec.bool = true,
                    "int" => spec.int = true,
                    "..." => spec.any = true,
                    "sev" => spec.sev = true,
                    "list" => spec.list = true,
                    other => spec.words.push(other.to_ascii_lowercase()),
                }
            }
            match name.to_ascii_lowercase().split_once('*') {
                Some((p, s)) => table.wild.push((p.into(), s.into(), spec)),
                None => {
                    table.exact.insert(name.to_ascii_lowercase(), spec);
                }
            }
        }
        table
    })
}

fn lookup(name: &str) -> Option<&'static Spec> {
    let t = table();
    t.exact.get(name).or_else(|| {
        t.wild.iter().find_map(|(p, s, spec)| {
            (name.len() > p.len() + s.len()
                && name.starts_with(p.as_str())
                && name.ends_with(s.as_str()))
            .then_some(spec)
        })
    })
}

impl Spec {
    fn accepts(&self, value: &str) -> bool {
        let mut v = value.trim().to_ascii_lowercase();
        if v == "unset" || self.any {
            return true;
        }
        if let Some((rest, sev)) = v.rsplit_once(':').filter(|_| self.sev)
            && SEVERITIES.contains(&sev.trim())
        {
            v = rest.trim().to_string();
        }
        let one = |v: &str| {
            self.words.iter().any(|w| w == v)
                || (self.bool && (v == "true" || v == "false"))
                || (self.int && !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
        };
        if self.list {
            v.split(',').all(|e| one(e.trim()))
        } else {
            one(&v)
        }
    }

    fn candidates(&self) -> impl Iterator<Item = &str> {
        let bools = self.bool.then_some(["true", "false"]).into_iter().flatten();
        self.words.iter().map(String::as_str).chain(bools)
    }
}

/// The key and value tokens of a `Pair`.
fn pair(node: &SyntaxNode) -> (Option<SyntaxToken>, Option<SyntaxToken>) {
    let mut tokens = node.children_with_tokens().filter_map(|e| e.into_token());
    let key = tokens.clone().find(|t| t.kind() == kind::KEY);
    (key, tokens.find(|t| t.kind() == kind::VALUE))
}

/// The known name closest to `name` (the original-case key), with the `*` of a name like
/// `dotnet_diagnostic.*.severity` filled in from `name`.
fn closest(name: &str, original: &str) -> Option<String> {
    let t = table();
    let filled: Vec<String> = t
        .wild
        .iter()
        .filter(|(p, _, _)| name.starts_with(p.as_str()))
        .filter_map(|(p, s, _)| {
            Some(format!(
                "{p}{}{s}",
                original.get(p.len()..name.rfind('.')?)?
            ))
        })
        .collect();
    let all = t
        .exact
        .keys()
        .map(String::as_str)
        .chain(filled.iter().map(String::as_str));
    suggest(name, all).map(String::from)
}

rule! {
    pub UnknownProperty {
        id: "unknown-property",
        category: Correctness,
        default: Warn,
        description: "A property name that is not an EditorConfig or .NET (`dotnet_*`, `csharp_*`, \
                  `visual_basic_*`, `dotnet_diagnostic.<ID>.severity`, naming rule) property but is \
                  one typo away from one: the setting is silently ignored. Names of other tools \
                  (`resharper_*`, `ij_*`) are left alone. Known properties and values: \
                  `crates/lang-editorconfig/data/properties.txt`.",
        example: "indent_syle = space   # unknown property; did you mean `indent_style`?",
        options: [OptionDecl {
            name: "strict",
            default: Opt::Bool(false),
            check: Check::Any,
            description: "Also report `dotnet_*`, `csharp_*` and `visual_basic_*` names that are \
                      close to no known property (the list may lag behind new .NET options).",
        }],
        kinds: &[kind::PAIR],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (Some(key), _) = pair(node) else { return };
        let original = key.text().trim();
        let name = original.to_ascii_lowercase();
        if lookup(&name).is_some() {
            return;
        }
        let near = closest(&name, original);
        let net = ["dotnet_", "csharp_", "visual_basic_"].iter().any(|p| name.starts_with(p));
        if near.is_none() && !(net && ctx.options.bool("strict")) {
            return;
        }
        let hint = near.map(|n| format!("; did you mean `{n}`?")).unwrap_or_default();
        ctx.report(key.text_range(), format!("unknown property `{original}`{hint}"));
    }
}

rule! {
    pub InvalidPropertyValue {
        id: "invalid-property-value",
        category: Correctness,
        default: Warn,
        description: "A known EditorConfig or .NET property has a value it does not accept \
                  (`indent_style = tabs`, `dotnet_diagnostic.CA1000.severity = high`). Values are \
                  case-insensitive, `unset` is always valid, and .NET code-style options may end in \
                  `:silent`, `:suggestion`, `:warning` or `:error`. Unknown properties are not \
                  checked.",
        example: "indent_style = tabs   # `tabs` is not a value of `indent_style`; did you mean `tab`?",
        kinds: &[kind::PAIR],
    }

    fn check(&self, node: &SyntaxNode, ctx: &mut RuleContext<'_>) {
        let (Some(key), value) = pair(node) else { return };
        let name = key.text().trim().to_ascii_lowercase();
        let Some(spec) = lookup(&name) else { return };
        let text = value.as_ref().map_or("", |v| v.text().trim());
        if spec.accepts(text) {
            return;
        }
        let hint = match suggest(&text.to_ascii_lowercase(), spec.candidates()) {
            Some(s) => format!("; did you mean `{s}`?"),
            None if spec.words.len() + 2 * usize::from(spec.bool) <= 8 => {
                let mut all: Vec<&str> = spec.candidates().collect();
                if spec.int {
                    all.push("a number");
                }
                format!(" (expected: {})", all.join(", "))
            }
            None => String::new(),
        };
        let range = value.as_ref().map_or(key.text_range(), |v| v.text_range());
        ctx.report(range, format!("`{text}` is not a value of `{}`{hint}", key.text().trim()));
    }
}

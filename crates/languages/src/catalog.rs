//! The parity catalog (`docs/parity/*.yaml`), embedded: the source of the reasons printed for
//! settings that cannot be migrated, of the checks that each `init --from` mapping agrees with it,
//! and of the "Equivalent" ids shown for each rule.

use serde::Deserialize;
use std::sync::OnceLock;

pub struct Entry {
    pub id: String,
    /// `rule: <id>`, `option: <name>`, `formatter`, `sdk: <id>`, `config: <text>`, `planned`,
    /// `deferred` or `wontfix: <reason>`.
    pub sudsy: String,
    /// `covered`, `partial`, `missing`, `sdk`, `wontfix` or `deferred`.
    pub coverage: String,
    pub gap: String,
}

#[derive(Deserialize)]
pub struct Catalog {
    pub tool: String,
    pub languages: Vec<String>,
    /// Filled by [`parse`] from `listed` and `deferred`.
    #[serde(skip)]
    pub entries: Vec<Entry>,
    #[serde(rename = "entries")]
    listed: Pairs,
    #[serde(default)]
    deferred: Vec<Deferred>,
}

#[derive(Deserialize)]
struct Deferred {
    gap: String,
    ids: Vec<String>,
}

/// A YAML map as its pairs in file order.
#[derive(Default)]
struct Pairs(Vec<(String, String)>);

impl<'de> Deserialize<'de> for Pairs {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Pairs;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a map of id to status")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<Pairs, A::Error> {
                let mut out = Vec::new();
                while let Some(pair) = m.next_entry()? {
                    out.push(pair);
                }
                Ok(Pairs(out))
            }
        }
        d.deserialize_map(V)
    }
}

/// Reads a catalog file (format: docs/parity/README.md): each `id: <status> [| <gap>]` of
/// `entries`, then each id under `deferred` with the group's `gap`.
pub fn parse(text: &str) -> Result<Catalog, String> {
    let mut c: Catalog = serde_saphyr::from_str(text).map_err(|e| e.to_string())?;
    for (id, value) in std::mem::take(&mut c.listed).0 {
        let (status, gap) = value.split_once(" | ").unwrap_or((&value, ""));
        let (kind, arg) = status.split_once(' ').unwrap_or((status, ""));
        let coverage = match kind {
            "sdk" | "wontfix" | "deferred" => kind,
            "planned" => "missing",
            _ if gap.is_empty() => "covered",
            _ => "partial",
        };
        let sudsy = if arg.is_empty() {
            kind.into()
        } else {
            format!("{kind}: {arg}")
        };
        c.entries.push(Entry {
            id,
            sudsy,
            coverage: coverage.into(),
            gap: gap.into(),
        });
    }
    for d in std::mem::take(&mut c.deferred) {
        c.entries.extend(d.ids.into_iter().map(|id| Entry {
            id,
            sudsy: "deferred".into(),
            coverage: "deferred".into(),
            gap: d.gap.clone(),
        }));
    }
    Ok(c)
}

macro_rules! catalogs {
    ($($name:literal),*) => {{
        static CELL: OnceLock<Vec<Catalog>> = OnceLock::new();
        CELL.get_or_init(|| {
            vec![$(parse(include_str!(concat!(
                "../../../docs/parity/", $name, ".yaml"
            )))
            .expect("parity catalog parses")),*]
        })
    }};
}

/// Every catalog, one per tool, in file name order.
pub fn all() -> &'static [Catalog] {
    catalogs!(
        "biome",
        "csharpier",
        "dotnet-format-editorconfig",
        "eslint-plugin-jsonc",
        "prettier",
        "roslynator",
        "shfmt",
        "stylecop",
        "stylelint",
        "stylelint-less",
        "stylelint-scss",
        "taplo",
        "yamllint"
    )
}

/// The entries of the catalog named `tool` (the file name without `.yaml`).
pub fn entries(tool: &str) -> &'static [Entry] {
    all()
        .iter()
        .find(|c| c.tool == tool)
        .map_or(&[], |c| &c.entries)
}

/// The entry for an upstream setting. Some entries name several settings (`a / b`, `a, b`).
pub fn find(tool: &str, id: &str) -> Option<&'static Entry> {
    // A tool's plugin catalogs (`stylelint-scss` for `stylelint`) are searched with its own.
    all()
        .iter()
        .filter(|c| {
            c.tool == tool
                || c.tool
                    .strip_prefix(tool)
                    .is_some_and(|r| r.starts_with('-'))
        })
        .flat_map(|c| &c.entries)
        .find(|e| e.id == id || e.id.split([',', '/']).any(|part| part.trim() == id))
}

/// Why a setting cannot be migrated, in the catalog's words.
pub fn reason(tool: &str, id: &str) -> String {
    match find(tool, id) {
        Some(e) if !e.gap.is_empty() => e.gap.clone(),
        Some(e) if e.sudsy == "formatter" => "fixed formatter behavior, not configurable".into(),
        Some(e) if e.sudsy.starts_with("rule: ") || e.sudsy.starts_with("option: ") => {
            format!("this setting has no Sudsy option; {}", e.sudsy)
        }
        Some(e) => e.sudsy.clone(),
        None => "not in the parity catalog".into(),
    }
}

fn tool_name(tool: &str) -> &str {
    match tool {
        "stylecop" => "StyleCop",
        "csharpier" => "CSharpier",
        "roslynator" => "Roslynator",
        "prettier" => "Prettier",
        "biome" => "Biome",
        other => other,
    }
}

/// An upstream setting that maps to a rule.
pub struct Upstream {
    /// The tool's display name ("StyleCop").
    pub tool: &'static str,
    pub id: String,
    pub partial: bool,
}

impl std::fmt::Display for Upstream {
    /// "StyleCop SA1106", with " (partial)" for partial coverage.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let partial = if self.partial { " (partial)" } else { "" };
        write!(f, "{} {}{partial}", self.tool, self.id)
    }
}

/// The upstream settings that map to the rule `rule` in one of `languages` (`all`: every
/// language), in catalog order.
pub fn upstream(rule: &str, languages: &[&str]) -> Vec<Upstream> {
    let status = format!("rule: {rule}");
    let mut out = Vec::new();
    for c in all() {
        let applies = languages.contains(&"all")
            || c.languages.iter().any(|l| languages.contains(&l.as_str()));
        if !applies {
            continue;
        }
        for e in &c.entries {
            if e.sudsy == status && matches!(e.coverage.as_str(), "covered" | "partial") {
                out.push(Upstream {
                    tool: tool_name(&c.tool),
                    id: e.id.clone(),
                    partial: e.coverage == "partial",
                });
            }
        }
    }
    out
}

/// The rules that cover the upstream setting `id` (case-insensitive; the whole id or one of the
/// settings an entry names).
pub fn rules_for(id: &str) -> Vec<&'static str> {
    let mut out: Vec<&str> = all()
        .iter()
        .flat_map(|c| &c.entries)
        .filter(|e| matches!(e.coverage.as_str(), "covered" | "partial"))
        .filter(|e| {
            e.id.eq_ignore_ascii_case(id)
                || e.id
                    .split([',', '/'])
                    .any(|p| p.trim().eq_ignore_ascii_case(id))
        })
        .filter_map(|e| e.sudsy.strip_prefix("rule: "))
        .collect();
    out.sort();
    out.dedup();
    out
}

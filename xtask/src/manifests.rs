//! What a change to the root `Cargo.toml` or `Cargo.lock` reaches, so `affected` can select the
//! crates it touches instead of everything (adding a language edits both).

use std::collections::{BTreeMap, BTreeSet};

/// The workspace members (lock entries without `source`) whose dependency closure in the lock
/// contains a package entry that differs between `base` and `new` (added, removed or changed).
pub fn lock_changes(base: &str, new: &str) -> BTreeSet<String> {
    let (old, new) = (packages(base), packages(new));
    let mut reached: BTreeSet<&str> = (old.symmetric_difference(&new)).map(|p| p.name).collect();
    // Edges of both sides, by name: a version bump reaches every user of that name.
    let edges: Vec<(&str, &str)> = (old.iter().chain(&new))
        .flat_map(|p| p.deps.iter().map(move |d| (p.name, *d)))
        .collect();
    while let Some(&(user, _)) = edges
        .iter()
        .find(|(u, d)| reached.contains(d) && !reached.contains(u))
    {
        reached.insert(user);
    }
    (new.iter().chain(&old))
        .filter(|p| p.member && reached.contains(p.name))
        .map(|p| p.name.to_string())
        .collect()
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Package<'a> {
    name: &'a str,
    text: &'a str,
    member: bool,
    deps: Vec<&'a str>,
}

fn packages(lock: &str) -> BTreeSet<Package<'_>> {
    (lock.split("[[package]]").skip(1))
        .map(|block| {
            let field = |k: &str| {
                (block.lines())
                    .find_map(|l| l.strip_prefix(k)?.strip_prefix(" = \""))
                    .map(|v| v.trim_end_matches('"'))
            };
            let deps = (block.lines())
                .filter_map(|l| l.strip_prefix(" \"")?.split([' ', '"']).next())
                .collect();
            Package {
                name: field("name").unwrap_or(""),
                text: block.trim(),
                member: field("source").is_none(),
                deps,
            }
        })
        .collect()
}

/// The `[workspace.dependencies]` keys that differ between the two root manifests, or `None` when
/// anything else differs (members aside: a new or removed member is a changed folder of its own).
pub fn manifest_changes(base: &str, new: &str) -> Option<BTreeSet<String>> {
    let (old, new) = (entries(base), entries(new));
    let mut deps = BTreeSet::new();
    for key in old.keys().chain(new.keys()) {
        if old.get(key) == new.get(key) {
            continue;
        }
        match (key.0.as_str(), key.1.as_str()) {
            ("workspace", "members") => {}
            ("workspace.dependencies", dep) => {
                deps.insert(dep.to_string());
            }
            _ => return None,
        }
    }
    Some(deps)
}

/// (section, key) to value; a value with open brackets goes on over the next lines. Comments and
/// blank lines do not count.
fn entries(toml: &str) -> BTreeMap<(String, String), String> {
    let (mut out, mut section) = (BTreeMap::new(), String::new());
    let mut open: Option<(String, String)> = None;
    let depth = |v: &str| v.matches(['[', '{']).count() > v.matches([']', '}']).count();
    for line in toml.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let key = if let Some(key) = open.take() {
            let v: &mut String = out.get_mut(&key).expect("open entry");
            v.push_str(line);
            key
        } else if line.starts_with('[') {
            section = line.trim_matches(['[', ']']).to_string();
            continue;
        } else {
            let (k, v) = line.split_once('=').unwrap_or((line, ""));
            let key = (section.clone(), k.trim().to_string());
            out.insert(key.clone(), v.trim().to_string());
            key
        };
        if depth(&out[&key]) {
            open = Some(key);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"
[[package]]
name = "cli"
version = "0.1.0"
dependencies = [
 "languages",
]

[[package]]
name = "languages"
version = "0.1.0"
dependencies = [
 "lang-json",
]

[[package]]
name = "lang-json"
version = "0.1.0"
dependencies = [
 "tree-sitter-json",
]

[[package]]
name = "lang-toml"
version = "0.1.0"

[[package]]
name = "tree-sitter-json"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
"#;

    #[test]
    fn lock_reaches_the_users_of_a_changed_package() {
        assert!(lock_changes(LOCK, LOCK).is_empty());
        let bumped = LOCK.replace(
            "name = \"tree-sitter-json\"\nversion = \"0.1.0\"",
            "name = \"tree-sitter-json\"\nversion = \"0.2.0\"",
        );
        assert_eq!(
            lock_changes(LOCK, &bumped),
            ["cli", "lang-json", "languages"].map(String::from).into()
        );
        // A new member: itself and its users (languages lists it).
        let added = LOCK.replace(" \"lang-json\",\n]", " \"lang-json\",\n \"lang-new\",\n]")
            + "\n[[package]]\nname = \"lang-new\"\nversion = \"0.1.0\"\n";
        assert_eq!(
            lock_changes(LOCK, &added),
            ["cli", "lang-new", "languages"].map(String::from).into()
        );
    }

    const TOML: &str = r#"[workspace]
members = ["crates/*",
  "xtask"]

[workspace.dependencies]
anyhow = "1"
# a comment
lang-json = { path = "crates/lang-json" }

[profile.dev]
opt-level = 1
"#;

    #[test]
    fn manifest_names_changed_workspace_dependencies() {
        assert_eq!(manifest_changes(TOML, TOML), Some(BTreeSet::new()));
        let added = TOML.replace(
            "anyhow = \"1\"",
            "anyhow = \"1\"\ntree-sitter-new = \"0.1\"\n# b\n",
        );
        assert_eq!(
            manifest_changes(TOML, &added),
            Some(["tree-sitter-new".to_string()].into())
        );
        let members = TOML.replace("\"xtask\"]", "\"xtask\", \"tools/x\"]");
        assert_eq!(manifest_changes(TOML, &members), Some(BTreeSet::new()));
        assert_eq!(manifest_changes(TOML, &TOML.replace("= 1", "= 2")), None);
        assert_eq!(
            manifest_changes(
                TOML,
                &TOML.replace("[profile.dev]", "[profile.dev]\nlto = true")
            ),
            None
        );
    }
}

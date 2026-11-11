//! What a migration produces: the `sudsy.yaml` tree, the report lines, and the rendering of the
//! tree as YAML with everything a named preset already says removed.

use languages::catalog;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub struct Out {
    tool: &'static str,
    extends: Vec<String>,
    cfg: Map<String, Value>,
    pub not_migrated: Vec<String>,
    pub notes: Vec<String>,
    /// `.editorconfig` lines (C# only) that hand the checks Sudsy now does over from the SDK.
    pub editorconfig: Vec<String>,
}

impl Out {
    /// `tool` is the parity catalog the mappings are checked against.
    pub fn new(tool: &'static str) -> Self {
        Out {
            tool,
            extends: Vec::new(),
            cfg: Map::new(),
            not_migrated: Vec::new(),
            notes: Vec::new(),
            editorconfig: Vec::new(),
        }
    }

    pub fn extend(&mut self, preset: &str) {
        if !self.extends.iter().any(|p| p == preset) {
            self.extends.push(preset.to_string());
        }
    }

    /// Sets `<lang>.format.<name>` for each language; `id` is the catalog entry that maps it.
    pub fn opt(&mut self, id: &str, langs: &[&str], name: &str, value: impl Into<Value>) {
        self.check(id, |sudsy| sudsy.ends_with(&format!(".format.{name}")));
        self.opt_free(langs, name, value);
    }

    /// Like [`Out::opt`] for a setting derived from another one, with no catalog entry of its own.
    pub fn opt_free(&mut self, langs: &[&str], name: &str, value: impl Into<Value>) {
        let value = value.into();
        for lang in langs {
            self.put(&[lang, "format", name], value.clone());
        }
    }

    /// Sets the level of a rule; `id` is the catalog entry that maps it.
    pub fn rule(&mut self, id: &str, lang: &str, rule: &str, level: &str) {
        self.check(id, |sudsy| sudsy == format!("rule: {rule}"));
        self.rule_free(lang, rule, level);
    }

    pub fn rule_free(&mut self, lang: &str, rule: &str, level: &str) {
        self.put(&[lang, "lint", "rules", rule, "level"], level.into());
    }

    pub fn rule_opt(&mut self, id: &str, lang: &str, rule: &str, name: &str, value: Value) {
        self.check(id, |sudsy| sudsy == format!("rule: {rule}"));
        self.put(&[lang, "lint", "rules", rule, "options", name], value);
    }

    /// A rule option set from a setting with no catalog entry of its own.
    pub fn rule_opt_free(&mut self, lang: &str, rule: &str, name: &str, value: Value) {
        self.put(&[lang, "lint", "rules", rule, "options", name], value);
    }

    /// The catalog entry of an upstream setting; an unknown one is listed as not migrated.
    pub fn entry(&mut self, id: &str, unknown: &str) -> Option<&'static catalog::Entry> {
        let entry = catalog::find(self.tool, id);
        if entry.is_none() {
            self.skip_why(id, unknown);
        }
        entry
    }

    /// Adds a glob to the top-level `exclude`.
    pub fn exclude(&mut self, pattern: &str) {
        let list = self
            .cfg
            .entry("exclude")
            .or_insert_with(|| Value::Array(vec![]));
        list.as_array_mut().expect("array").push(pattern.into());
    }

    /// Lists a setting that cannot be migrated, with the catalog's reason.
    pub fn skip(&mut self, id: &str) {
        let why = catalog::reason(self.tool, id);
        self.skip_why(id, &why);
    }

    pub fn skip_why(&mut self, key: &str, why: &str) {
        self.not_migrated.push(format!("{key} ({why})"));
    }

    pub fn note(&mut self, text: impl Into<String>) {
        self.notes.push(text.into());
    }

    /// In tests every mapping must agree with the catalog entry it names.
    fn check(&self, id: &str, agrees: impl Fn(&str) -> bool) {
        if cfg!(debug_assertions) {
            let entry = catalog::find(self.tool, id)
                .unwrap_or_else(|| panic!("{}: no catalog entry `{id}`", self.tool));
            assert!(
                agrees(&entry.sudsy),
                "{}: `{id}` is `{}` in the catalog",
                self.tool,
                entry.sudsy
            );
        }
    }

    fn put(&mut self, path: &[&str], value: Value) {
        let (last, parents) = path.split_last().expect("non-empty path");
        let mut map = &mut self.cfg;
        for key in parents {
            let child = map
                .entry(key.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            map = child.as_object_mut().expect("object path");
        }
        map.insert(last.to_string(), value);
    }

    /// The `sudsy.yaml` text. `presets` are the (name, text) pairs of the known presets.
    pub fn render(mut self, tool_arg: &str, presets: &[(&str, &str)]) -> (String, Report) {
        let mut inherited = BTreeMap::new();
        for name in &self.extends {
            let text = presets
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, t)| *t)
                .unwrap_or_else(|| panic!("unknown preset {name}"));
            let tree: Value = serde_saphyr::from_str(text).expect("presets parse");
            flatten(&mut Vec::new(), &tree, &mut inherited);
        }
        prune(&mut Vec::new(), &mut self.cfg, &inherited);
        collapse_levels(&mut self.cfg);
        let mut text =
            format!("# Generated by `sudsy init --from {tool_arg}`; review before use.\n");
        if !self.extends.is_empty() {
            text.push_str("extends:\n");
            for name in &self.extends {
                text.push_str(&format!("  - preset:{name}\n"));
            }
        }
        emit(&self.cfg, 0, &mut text);
        let report = Report {
            extends: self.extends,
            not_migrated: self.not_migrated,
            notes: self.notes,
            editorconfig: self.editorconfig,
        };
        (text, report)
    }
}

pub struct Report {
    pub extends: Vec<String>,
    pub not_migrated: Vec<String>,
    pub notes: Vec<String>,
    pub editorconfig: Vec<String>,
}

/// Leaves of `tree` by path; a rule given as a bare level counts as `{ level }`.
fn flatten(path: &mut Vec<String>, tree: &Value, out: &mut BTreeMap<String, Value>) {
    match tree {
        Value::Object(map) => {
            for (key, value) in map {
                path.push(key.clone());
                flatten(path, value, out);
                path.pop();
            }
        }
        leaf => {
            let is_rule = path.len() >= 2 && path[path.len() - 2] == "rules";
            let mut key = path.join("\0");
            if is_rule {
                key.push_str("\0level");
            }
            out.insert(key, leaf.clone());
        }
    }
}

/// Removes the leaves of `cfg` that the presets already set to the same value.
fn prune(
    path: &mut Vec<String>,
    cfg: &mut Map<String, Value>,
    inherited: &BTreeMap<String, Value>,
) {
    for key in cfg.keys().cloned().collect::<Vec<_>>() {
        path.push(key.clone());
        let child = cfg.get_mut(&key).expect("key exists");
        let drop = match child {
            Value::Object(map) => {
                // A rule that still has options keeps its level: the loader requires one.
                let is_rule = path.len() >= 2 && path[path.len() - 2] == "rules";
                let level = map
                    .get("level")
                    .or_else(|| inherited.get(&format!("{}\0level", path.join("\0"))))
                    .cloned();
                prune(path, map, inherited);
                if is_rule && !map.is_empty() && !map.contains_key("level") {
                    map.extend(level.map(|l| ("level".to_string(), l)));
                }
                map.is_empty()
            }
            leaf => inherited.get(&path.join("\0")) == Some(&*leaf),
        };
        path.pop();
        if drop {
            cfg.remove(&key);
        }
    }
}

/// `rule: { level: warn }` becomes `rule: warn`.
fn collapse_levels(cfg: &mut Map<String, Value>) {
    for (key, value) in cfg.iter_mut() {
        let Value::Object(map) = value else { continue };
        if key == "rules" {
            for rule in map.values_mut() {
                if let Value::Object(r) = rule
                    && r.len() == 1
                    && let Some(level) = r.get("level")
                {
                    *rule = level.clone();
                }
            }
        } else {
            collapse_levels(map);
        }
    }
}

fn emit(map: &Map<String, Value>, indent: usize, out: &mut String) {
    for (key, value) in map {
        let pad = " ".repeat(indent);
        match value {
            Value::Object(child) => {
                out.push_str(&format!("{pad}{key}:\n"));
                emit(child, indent + 2, out);
            }
            _ => out.push_str(&format!("{pad}{key}: {}\n", scalar(value))),
        }
    }
}

fn scalar(value: &Value) -> String {
    match value {
        Value::String(s) => {
            let plain = s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_-./".contains(c))
                && !matches!(
                    s.to_ascii_lowercase().as_str(),
                    "true" | "false" | "null" | "yes" | "no" | "on" | "off" | "y" | "n"
                );
            if plain {
                s.clone()
            } else {
                Value::String(s.clone()).to_string()
            }
        }
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(scalar).collect();
            format!("[{}]", items.join(", "))
        }
        other => other.to_string(),
    }
}

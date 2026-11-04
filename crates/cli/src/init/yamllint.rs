//! yamllint (`.yamllint`, `.yamllint.yaml`, `.yamllint.yml`) -> `yaml:` section.

use super::out::Out;
use anyhow::{Result, bail};
use serde_json::{Map, Value, json};

pub const FILES: [&str; 3] = [".yamllint", ".yamllint.yaml", ".yamllint.yml"];
const L: &[&str] = &["yaml"];
/// Rules yamllint's default config sets to `warning`; the others it enables are errors.
const DEFAULT_WARN: [&str; 4] = [
    "comments",
    "comments-indentation",
    "document-start",
    "truthy",
];
/// Rules that are only a matter of the formatter's output.
const FORMATTER_ONLY: [&str; 5] = [
    "colons",
    "commas",
    "hyphens",
    "trailing-spaces",
    "comments-indentation",
];

pub fn migrate(config: &Value, out: &mut Out) -> Result<()> {
    let Some(root) = config.as_object() else {
        bail!("the config is not a mapping");
    };
    let mut default_base = false;
    match root.get("extends").and_then(Value::as_str) {
        Some("default") => default_base = true,
        Some("relaxed") => {
            default_base = true;
            out.skip_why(
                "extends: relaxed",
                "started from the default preset; relaxed's looser rule levels are not applied",
            );
        }
        Some(other) => out.skip_why(
            &format!("extends: {other}"),
            "only the built-in configs can be followed",
        ),
        None => {}
    }
    if default_base {
        out.extend("yaml/yamllint");
    }
    for key in ["yaml-files", "ignore", "ignore-from-file", "locale"] {
        if root.contains_key(key) {
            out.skip_why(key, "use `include`/`exclude` globs in sudsy.yaml");
        }
    }
    if let Some(Value::Object(rules)) = root.get("rules") {
        for (name, conf) in rules {
            rule(name, conf, default_base, out);
        }
    }
    Ok(())
}

fn rule(name: &str, conf: &Value, default_base: bool, out: &mut Out) {
    let (enabled, mut opts) = match conf {
        Value::String(s) if s == "disable" => (false, Map::new()),
        Value::Bool(false) => (false, Map::new()),
        Value::Object(map) => (true, map.clone()),
        _ => (true, Map::new()),
    };
    let level = match opts.remove("level").as_ref().and_then(Value::as_str) {
        Some("warning") => "warn",
        Some("error") => "error",
        _ if default_base && DEFAULT_WARN.contains(&name) => "warn",
        _ => "error",
    };
    let level = if enabled { level } else { "off" };
    let mut take = |key: &str| opts.remove(key);
    let lint = |out: &mut Out, id: &str, sudsy: &str| out.rule(id, "yaml", sudsy, level);
    match name {
        "anchors" => {
            let undeclared = take("forbid-undeclared-aliases") != Some(json!(false));
            let l = if undeclared { level } else { "off" };
            out.rule(
                "anchors.forbid-undeclared-aliases",
                "yaml",
                "no-undefined-alias",
                l,
            );
            if take("forbid-duplicated-anchors") == Some(json!(true)) {
                lint(
                    out,
                    "anchors.forbid-duplicated-anchors",
                    "no-duplicate-anchors",
                );
            }
            if take("forbid-unused-anchors") == Some(json!(true)) {
                lint(out, "anchors.forbid-unused-anchors", "no-unused-anchors");
            }
        }
        "comments" => {
            let required = take("require-starting-space") != Some(json!(false));
            out.rule(
                "comments.require-starting-space",
                "yaml",
                "comment-space",
                if required { level } else { "off" },
            );
            if let Some(n) = take("min-spaces-from-content") {
                out.opt("comments.min-spaces-from-content", L, "comment_spacing", n);
            }
            if take("ignore-shebangs") == Some(json!(false)) {
                out.skip("comments.ignore-shebangs");
            }
        }
        "document-start" => {
            lint(out, "document-start", "document-start");
            match take("present") {
                Some(Value::Bool(true)) => {
                    out.opt("document-start.present", L, "document_start", "always")
                }
                Some(Value::Bool(false)) => {
                    out.opt("document-start.present", L, "document_start", "never")
                }
                _ => {}
            }
        }
        "empty-values" => lint(out, "empty-values", "no-empty-values"),
        "float-values" => lint(out, "float-values", "float-style"),
        "octal-values" => lint(out, "octal-values", "no-octal-values"),
        "quoted-strings" => lint(out, "quoted-strings", "quoted-strings"),
        "key-duplicates" => {
            lint(out, "key-duplicates", "no-duplicate-keys");
            if take("forbid-duplicated-merge-keys") == Some(json!(true)) {
                out.rule_opt(
                    "key-duplicates.forbid-duplicated-merge-keys",
                    "yaml",
                    "no-duplicate-keys",
                    "merge_keys",
                    "forbid".into(),
                );
            }
        }
        "key-ordering" => {
            lint(out, "key-ordering", "sorted-keys");
            if let Some(keys) = take("ignored-keys") {
                out.rule_opt(
                    "key-ordering.ignored-keys",
                    "yaml",
                    "sorted-keys",
                    "ignored_keys",
                    keys,
                );
            }
        }
        "line-length" => {
            lint(out, "line-length", "max-line-length");
            if let Some(max) = take("max") {
                out.rule_opt(
                    "line-length.max",
                    "yaml",
                    "max-line-length",
                    "max",
                    max.clone(),
                );
                out.opt_free(L, "line_width", max);
            }
            if let Some(v) = take("allow-non-breakable-words") {
                out.rule_opt(
                    "line-length.allow-non-breakable-words",
                    "yaml",
                    "max-line-length",
                    "allow_non_breakable_words",
                    v,
                );
            }
        }
        "truthy" => {
            lint(out, "truthy", "ambiguous-booleans");
            if let Some(v) = take("allowed-values") {
                out.rule_opt(
                    "truthy.allowed-values",
                    "yaml",
                    "ambiguous-booleans",
                    "allowed",
                    v,
                );
            }
            if let Some(v) = take("check-keys") {
                out.rule_opt(
                    "truthy.check-keys",
                    "yaml",
                    "ambiguous-booleans",
                    "check_keys",
                    v,
                );
            }
        }
        "braces" | "brackets" if enabled => {
            let option = if name == "braces" {
                "bracket_spacing"
            } else {
                "array_bracket_spacing"
            };
            if let Some(n) = take("min-spaces-inside").and_then(|n| n.as_i64()) {
                out.opt(
                    &format!("{name}.min-spaces-inside / max-spaces-inside"),
                    L,
                    option,
                    n > 0,
                );
            }
            take("max-spaces-inside");
        }
        "empty-lines" if enabled => {
            if let Some(n) = take("max") {
                out.opt("empty-lines.max", L, "max_blank_lines", n);
            }
        }
        "indentation" if enabled => {
            match take("spaces") {
                Some(n) if n.is_u64() => out.opt("indentation.spaces", L, "indent_size", n),
                Some(_) => out.skip_why(
                    "indentation.spaces",
                    "only a number of spaces can be migrated",
                ),
                None => {}
            }
            match take("indent-sequences") {
                Some(Value::Bool(b)) => out.opt(
                    "indentation.indent-sequences",
                    L,
                    "sequence_indent",
                    if b { "indented" } else { "flush" },
                ),
                Some(_) => out.skip_why(
                    "indentation.indent-sequences",
                    "only true and false can be migrated",
                ),
                None => {}
            }
        }
        "new-line-at-end-of-file" => {
            out.opt("new-line-at-end-of-file", L, "final_newline", enabled);
        }
        "new-lines" if enabled => match take("type").as_ref().and_then(Value::as_str) {
            Some("unix") => out.opt("new-lines.type", L, "end_of_line", "lf"),
            Some("dos") => out.opt("new-lines.type", L, "end_of_line", "crlf"),
            Some(_) => out.skip_why(
                "new-lines.type",
                "`platform` depends on the machine; use lf, crlf or auto",
            ),
            None => {}
        },
        n if FORMATTER_ONLY.contains(&n)
            || [
                "braces",
                "brackets",
                "empty-lines",
                "indentation",
                "new-lines",
            ]
            .contains(&n) => {}
        other => {
            out.skip_why(&format!("rules.{other}"), "not a yamllint rule Sudsy knows");
            return;
        }
    }
    for key in opts.keys() {
        let id = format!("{name}.{key}");
        let known = super::catalog::find("yamllint", &id).is_some();
        out.skip_why(
            &id,
            &super::catalog::reason("yamllint", if known { &id } else { name }),
        );
    }
}

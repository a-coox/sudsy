//! Pattern rules on JSON: matching, constraints, fixes and the annotation fixtures.

use core_rules::Extensions;
use lang_json::JsonProvider;
use provider_api::testing::{expect, fix_with, lint_with, pattern_error, pattern_rules};

const DEBUG_RULE: &str = r#"
id: no-debug-flag
language: json
message: "`debug` must not be enabled"
severity: error
rule:
  pattern: '"debug": true'
fix: '"debug": false'
fix-kind: safe
"#;

fn rules(source: &str) -> Extensions {
    pattern_rules(&JsonProvider::new(), source)
}

#[test]
fn a_json_member_pattern_reports_and_fixes() {
    let ext = rules(DEBUG_RULE);
    let text = expect(
        &JsonProvider::new(),
        &ext,
        "a.json",
        r#"{
  "debug": true,
//^^^^^^^^^^^^^ no-debug-flag
  "nested": { "debug": true },
//            ^^^^^^^^^^^^^ no-debug-flag
  "other": false
}
"#,
        "",
    );
    let fixed = fix_with(&JsonProvider::new(), &ext, "a.json", &text, "", false);
    assert_eq!(
        fixed,
        "{\n  \"debug\": false,\n  \"nested\": { \"debug\": false },\n  \"other\": false\n}\n"
    );
}

#[test]
fn metavariables_capture_and_repeat() {
    let ext = rules(
        r#"
id: duplicate-pair
language: json
message: "$A repeats $A"
rule:
  pattern: '[$A, $A]'
"#,
    );
    let found = lint_with(
        &JsonProvider::new(),
        &ext,
        "a.json",
        "[[1, 1], [1, 2], [\"x\", \"x\"]]",
        "",
    );
    let messages: Vec<_> = found.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, ["1 repeats 1", "\"x\" repeats \"x\""]);
}

#[test]
fn multiple_metavariables_take_any_run() {
    let ext = rules(
        r#"
id: first-is-zero
language: json
message: "starts with zero, then $$$REST"
rule:
  pattern: '[0, $$$REST]'
fix: '[$$$REST]'
"#,
    );
    let text = "[[0], [0, 1, 2], [1, 0], [0, 7]]";
    let found = lint_with(&JsonProvider::new(), &ext, "a.json", text, "");
    let messages: Vec<_> = found.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(
        messages,
        ["starts with zero, then 1, 2", "starts with zero, then 7"]
    );
    let fixed = fix_with(&JsonProvider::new(), &ext, "a.json", text, "", true);
    assert_eq!(fixed, "[[0], [1, 2], [1, 0], [7]]");
}

#[test]
fn relations_constraints_and_combinators() {
    let ext = rules(
        r#"
id: port-outside-test
language: json
message: "port $P"
rule:
  pattern: '"port": $P'
  not-inside:
    pattern: '"test": $T'
constraints:
  P: { regex: '^\d+$' }
"#,
    );
    let text = r#"{ "a": { "port": 80 }, "test": { "port": 81 }, "b": { "port": "x" } }"#;
    let found = lint_with(&JsonProvider::new(), &ext, "a.json", text, "");
    let messages: Vec<_> = found.iter().map(|d| d.message.as_str()).collect();
    assert_eq!(messages, ["port 80"]);
}

#[test]
fn the_level_comes_from_the_rule_unless_the_config_says_otherwise() {
    let ext = rules(DEBUG_RULE);
    let p = JsonProvider::new();
    let on = |rules: &str| lint_with(&p, &ext, "a.json", "{\"debug\": true}", rules);
    assert_eq!(on("")[0].severity, core_rules::Severity::Error);
    assert_eq!(
        on("no-debug-flag: warn")[0].severity,
        core_rules::Severity::Warning
    );
    assert!(on("no-debug-flag: \"off\"").is_empty());
}

#[test]
fn invalid_files_are_refused_with_a_reason() {
    let load = |source: &str| pattern_error(&JsonProvider::new(), source);
    let base = "id: r\nlanguage: json\nmessage: m\n";
    assert!(
        load(&format!("{base}rule: {{ kind: Nope }}"))
            .unwrap()
            .contains("not a node kind")
    );
    assert!(
        load(&format!("{base}rule: {{ pattern: '[1, 2' }}"))
            .unwrap()
            .contains("not a single syntax node")
    );
    assert!(
        load(&format!("{base}rule: {{ pattern: '[$A]' }}\nfix: '$B'"))
            .unwrap()
            .contains("`$B`")
    );
    assert!(
        load(&format!("{base}rule: {{}}"))
            .unwrap()
            .contains("at least one")
    );
    assert!(load(&format!("{base}rule: {{ kind: Array }}\nbogus: 1")).is_some());
}

//! The sample plugin (`tests/plugins/sample`, built here to Wasm) loaded by the host and run
//! through the same harness as the built-in rules: levels, options, suppressions, fixes, the
//! flow layer, the project phase and the limits.

use core_rules::engine::{ActiveRule, run_file, run_project};
use core_rules::{
    ExtensionRule, Extensions, FileContext, LineIndex, ProjectFacts, RuleOptions, Severity,
    activate,
};
use lang_json::JsonProvider;
use provider_api::LanguageProvider;
use provider_api::testing::{expect, fix_with, lint_with};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Builds the C# worker once per test process.
fn worker_dll() -> PathBuf {
    static DLL: OnceLock<PathBuf> = OnceLock::new();
    DLL.get_or_init(|| {
        let project = repo_root().join("workers/csharp/Sudsy.Worker");
        let status = Command::new("dotnet")
            .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
            .arg(&project)
            .status()
            .expect("dotnet is installed");
        assert!(status.success(), "dotnet build failed");
        project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
    })
    .clone()
}

fn extensions() -> Extensions {
    let path = &provider_api::testing::sample_plugin();
    let rules = core_plugins::load(path, &std::fs::read(path).unwrap()).expect("the plugin loads");
    Extensions {
        rules: rules
            .into_iter()
            .map(|(language, rule)| ExtensionRule { language, rule })
            .collect(),
        files: vec![path.clone()],
    }
}

#[test]
fn rules_carry_the_plugin_name_and_declare_themselves() {
    let ext = extensions();
    let ids: Vec<_> = ext.ids().collect();
    assert_eq!(
        ids,
        [
            "sample/no-debug-flag",
            "sample/branchy-body",
            "sample/duplicate-name",
            "sample/misbehave",
            "sample/todo-marker",
            "sample/break-arrays"
        ]
    );
    let flag = ext.rules[0].rule.meta();
    assert_eq!(flag.fix, Some(core_rules::FixKind::Safe));
    assert_eq!(flag.options[0].name, "key");
    assert_eq!(ext.rules[1].rule.meta().requires, [core_rules::Layer::Flow]);
}

#[test]
fn a_plugin_rule_reports_fixes_and_obeys_levels_options_and_suppressions() {
    let ext = extensions();
    let p = JsonProvider::new();
    let text = expect(
        &p,
        &ext,
        "a.json",
        "{\n  \"debug\": true,\n//^^^^^^^^^^^^^ sample/no-debug-flag\n  \"other\": true\n}\n",
        "",
    );
    assert_eq!(
        fix_with(&p, &ext, "a.json", &text, "", false),
        "{\n  \"debug\": false,\n  \"other\": true\n}\n"
    );
    let on = |rules: &str| lint_with(&p, &ext, "a.json", &text, rules);
    assert_eq!(on("sample/no-debug-flag: \"off\"").len(), 0);
    assert_eq!(
        on("sample/no-debug-flag: error")[0].severity,
        Severity::Error
    );
    let other = on("sample/no-debug-flag:\n  level: warn\n  options: { key: other }");
    assert_eq!(other.len(), 1);
    assert!(other[0].message.contains("other"));
    let wrong_type = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        on("sample/no-debug-flag:\n  level: warn\n  options: { key: 3 }")
    }));
    assert!(
        wrong_type.is_err(),
        "an option of the wrong type is a config error"
    );

    let suppressed = "{\n  // sudsy-disable-next-line sample/no-debug-flag\n  \"debug\": true\n}\n";
    assert!(lint_with(&p, &ext, "a.jsonc", suppressed, "").is_empty());
}

#[test]
fn a_body_rule_reads_the_flow_graph_through_the_plugin_boundary() {
    let ext = extensions();
    let csharp = lang_csharp::provider(&worker_dll());
    let text = "class A {\n void Few(int a) { if (a > 1) { } }\n void Many(int a) { if (a > 1) { } if (a > 2) { } if (a > 3) { } }\n}\n";
    let found = lint_with(
        &csharp,
        &ext,
        "a.cs",
        text,
        "sample/branchy-body:\n  level: warn\n  options: { max_branches: 2 }",
    );
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(
        found[0].message.starts_with("3 branching blocks"),
        "{}",
        found[0].message
    );
    // Without the flow layer a rule that requires it does not run; it does not fail.
    assert!(
        lint_with(
            &JsonProvider::new(),
            &ext,
            "a.json",
            "{}",
            "sample/branchy-body: warn"
        )
        .is_empty()
    );
}

#[test]
fn the_project_phase_runs_in_the_plugin() {
    let ext = extensions();
    let provider = JsonProvider::new();
    let configured = std::collections::BTreeMap::new();
    let rules = ext.for_language("json");
    let active = activate(rules, &configured, |_| true).expect("rules activate");
    let active: Vec<ActiveRule<'_>> = active
        .into_iter()
        .filter(|a| a.rule.meta().id == "sample/duplicate-name")
        .collect();
    assert_eq!(active.len(), 1);

    let mut facts = ProjectFacts::default();
    for (path, text) in [
        ("a.json", "{ \"name\": \"x\" }"),
        (
            "b.json",
            "{ \"name\": \"y\", \"inner\": { \"name\": \"x\" } }",
        ),
    ] {
        let parsed = provider
            .parse(&provider_api::SourceFile::new(path, 1, text))
            .unwrap();
        let lines = LineIndex::new(text);
        let file = FileContext {
            path: Path::new(path),
            text,
            root: &parsed.tree,
            grammar: provider.grammar(),
            lines: &lines,
            body_kinds: &[],
            semantics: None,
        };
        facts.extend(&run_file(&file, &active, true).facts);
    }
    let found = run_project(&active, &facts);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].0, Path::new("b.json"));
    assert!(
        found[0].1.message.contains("\"x\" is used twice"),
        "{}",
        found[0].1.message
    );
    let _ = RuleOptions::default();
}

fn disabled_by(mode: &str, expected: &str) {
    let ext = extensions();
    let p = JsonProvider::new();
    let rules = format!("sample/misbehave:\n  level: warn\n  options: {{ mode: {mode} }}");
    let first = lint_with(&p, &ext, "a.json", "{}", &rules);
    assert_eq!(first.len(), 1, "{first:?}");
    assert!(
        first[0].message.contains("is disabled for this run"),
        "{}",
        first[0].message
    );
    assert!(first[0].message.contains(expected), "{}", first[0].message);
    // Disabled for the rest of the run: the next file is silent, and so are the plugin's other rules.
    assert!(lint_with(&p, &ext, "b.json", "{}", &rules).is_empty());
    assert!(lint_with(&p, &ext, "c.json", "{\"debug\": true}", "").is_empty());
}

#[test]
fn a_trapping_plugin_is_disabled_and_reported() {
    disabled_by("trap", "trapped");
}

#[test]
fn a_spinning_plugin_runs_out_of_fuel_or_time() {
    // Whichever limit comes first; the message says which.
    let ext = extensions();
    let p = JsonProvider::new();
    let rules = "sample/misbehave:\n  level: warn\n  options: { mode: spin }";
    let found = lint_with(&p, &ext, "a.json", "{}", rules);
    assert_eq!(found.len(), 1, "{found:?}");
    let m = &found[0].message;
    assert!(
        m.contains("used up its fuel") || m.contains("time limit"),
        "{m}"
    );
}

#[test]
fn a_sleeping_plugin_is_stopped_not_waited_for() {
    let start = std::time::Instant::now();
    disabled_by("sleep", "blocking");
    assert!(start.elapsed().as_secs() < 30, "{:?}", start.elapsed());
}

#[test]
fn a_span_outside_the_file_disables_the_plugin() {
    disabled_by("span", "not inside the file");
}

#[test]
fn files_that_are_not_components_and_foreign_versions_are_refused() {
    let err = core_plugins::load(Path::new("x.wasm"), b"not wasm")
        .err()
        .expect("refused");
    assert!(err.0.contains("x.wasm"), "{}", err.0);
}

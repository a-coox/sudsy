//! Layout directives from pattern rules (`layout:`), through the CLI: `format`, `check` and
//! `fix` honor them, config levels and suppressions apply, conflicts are errors.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sudsy"));
    command.args(args).current_dir(dir);
    // Only C# needs the worker, and building it is slow.
    if args.iter().any(|a| a.ends_with(".cs")) {
        command.env("SUDSY_CSHARP_WORKER", csharp_worker());
    }
    command.output().unwrap()
}

/// The C# worker, built once per test process (tests run in parallel).
fn csharp_worker() -> PathBuf {
    static DLL: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DLL.get_or_init(build_worker).clone()
}

fn build_worker() -> PathBuf {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
    let status = Command::new("dotnet")
        .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
        .arg(&project)
        .status()
        .expect("dotnet is installed");
    assert!(status.success(), "dotnet build failed");
    project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).replace("\r\n", "\n")
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n")
}

/// A project with `rules` (a file each) and `files`; `config` is the rest of `sudsy.yaml`.
fn project(config: &str, rules: &[&str], files: &[(&str, &str)]) -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    let write = |rel: &str, content: &str| fs::write(t.path().join(rel), content).unwrap();
    fs::create_dir(t.path().join("rules")).unwrap();
    write(
        "sudsy.yaml",
        &format!("plugins:\n  patterns: [\"rules/*.yaml\"]\n{config}"),
    );
    for (i, rule) in rules.iter().enumerate() {
        write(&format!("rules/r{i}.yaml"), rule);
    }
    for (name, text) in files {
        write(name, text);
    }
    t
}

fn read(t: &tempfile::TempDir, name: &str) -> String {
    fs::read_to_string(t.path().join(name))
        .unwrap()
        .replace("\r\n", "\n")
}

/// Formats `name`, then checks that a second run has nothing to do (idempotency); returns the text.
fn format_twice(t: &tempfile::TempDir, name: &str) -> String {
    let first = sudsy(t.path(), &["format", name]);
    assert!(first.status.success(), "{}{}", out(&first), err(&first));
    let again = sudsy(t.path(), &["format", "--check", name]);
    assert!(
        again.status.success(),
        "not idempotent: {}{}",
        out(&again),
        err(&again)
    );
    read(t, name)
}

fn json_rule(id: &str, rule: &str, layout: &str) -> String {
    format!("id: {id}\nlanguage: json\nlayout: {layout}\nrule: {rule}\n")
}

const ARRAYS: &str = "{ kind: Array }";

#[test]
fn break_all_puts_every_item_on_its_own_line() {
    let rule = json_rule("arrays-one-per-line", ARRAYS, "[break-all]");
    let t = project("", &[&rule], &[("a.json", "{\"a\": [1, 2], \"b\": [3]}\n")]);
    assert_eq!(
        format_twice(&t, "a.json"),
        "{\n  \"a\": [\n    1,\n    2\n  ],\n  \"b\": [\n    3\n  ]\n}\n"
    );
}

#[test]
fn keep_together_joins_what_the_source_broke() {
    let rule = json_rule(
        "keep-objects",
        "{ kind: Object, regex: '\"x\"' }",
        "[keep-together]",
    );
    let source = "{\n  \"o\": {\n    \"x\": 1,\n    \"y\": 2\n  }\n}\n";
    let plain = project("", &[], &[("a.json", source)]);
    assert_eq!(format_twice(&plain, "a.json"), source);
    let t = project("", &[&rule], &[("a.json", source)]);
    assert_eq!(
        format_twice(&t, "a.json"),
        "{ \"o\": { \"x\": 1, \"y\": 2 } }\n"
    );
}

#[test]
fn break_before_break_after_and_blank_line_before_shape_the_gaps() {
    let before = json_rule(
        "b-before",
        "{ kind: Member, regex: '^\"b\"' }",
        "[break-before]",
    );
    let after = json_rule(
        "a-after",
        "{ kind: Member, regex: '^\"a\"' }",
        "[break-after]",
    );
    let source = "{\"a\": 1, \"b\": 2}\n";
    let plain = project("", &[], &[("a.json", source)]);
    assert_eq!(format_twice(&plain, "a.json"), "{ \"a\": 1, \"b\": 2 }\n");
    for rule in [&before, &after] {
        let t = project("", &[rule], &[("a.json", source)]);
        assert_eq!(
            format_twice(&t, "a.json"),
            "{\n  \"a\": 1,\n  \"b\": 2\n}\n"
        );
    }

    let blank = json_rule(
        "blank-b",
        "{ kind: Member, regex: '^\"b\"' }",
        "['blank-line-before 2']",
    );
    let t = project("", &[&blank], &[("a.json", "{\"a\": 1, \"b\": 2}\n")]);
    assert_eq!(
        format_twice(&t, "a.json"),
        "{\n  \"a\": 1,\n\n\n  \"b\": 2\n}\n"
    );
}

#[test]
fn align_pads_the_keys_of_a_broken_object() {
    let rule = json_rule("aligned", "{ kind: Object }", "[align, break-all]");
    let t = project("", &[&rule], &[("a.json", "{\"a\": 1, \"bbb\": 2}\n")]);
    let text = format_twice(&t, "a.json");
    assert_eq!(text, "{\n  \"a\"  : 1,\n  \"bbb\": 2\n}\n");
}

#[test]
fn preserve_leaves_a_node_as_written() {
    let rule = json_rule("keep-as-is", "{ kind: Array, regex: '1' }", "[preserve]");
    let t = project(
        "",
        &[&rule],
        &[("a.json", "{\"a\":   [1,   2],\"b\":   [3,  4]}\n")],
    );
    assert_eq!(
        format_twice(&t, "a.json"),
        "{ \"a\": [1,   2], \"b\": [3, 4] }\n"
    );
}

#[test]
fn style_is_accepted_and_a_metavariable_picks_the_target() {
    let style = json_rule("styled", ARRAYS, "['style compact']");
    let t = project("", &[&style], &[("a.json", "[1, 2]\n")]);
    assert_eq!(format_twice(&t, "a.json"), "[1, 2]\n");

    let target = "id: items-one-per-line\nlanguage: json\nlayout:\n  - break-all on $V\nrule:\n  pattern: '\"items\": $V'\n";
    let t = project(
        "",
        &[target],
        &[("a.json", "{\"items\": [1, 2], \"other\": [3, 4]}\n")],
    );
    assert_eq!(
        format_twice(&t, "a.json"),
        "{\n  \"items\": [\n    1,\n    2\n  ],\n  \"other\": [3, 4]\n}\n"
    );
}

#[test]
fn yaml_flow_sequences_take_directives_too() {
    let rule =
        "id: seq-one-per-line\nlanguage: yaml\nlayout: [break-all]\nrule: { kind: FlowSequence }\n";
    let t = project("", &[rule], &[("a.yaml", "a: [1, 2]\n")]);
    assert_eq!(format_twice(&t, "a.yaml"), "a:\n  [\n    1,\n    2\n  ]\n");
}

#[test]
fn check_and_fix_honor_directives() {
    let rule = json_rule("arrays-one-per-line", ARRAYS, "[break-all]");
    let t = project("", &[&rule], &[("a.json", "{\"a\": [1, 2]}\n")]);
    let check = sudsy(t.path(), &["check", "a.json"]);
    assert!(!check.status.success(), "{}", out(&check));
    assert!(out(&check).contains("not formatted"), "{}", out(&check));
    let fix = sudsy(t.path(), &["fix", "a.json"]);
    assert!(fix.status.success(), "{}", err(&fix));
    assert_eq!(read(&t, "a.json"), "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n");
    assert!(sudsy(t.path(), &["check", "a.json"]).status.success());
}

#[test]
fn a_layout_only_rule_reports_nothing_when_linting() {
    let rule = json_rule("arrays-one-per-line", ARRAYS, "[break-all]");
    let formatted = "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n";
    let t = project("", &[&rule], &[("a.json", formatted)]);
    let lint = sudsy(t.path(), &["lint", "a.json"]);
    assert!(lint.status.success(), "{}{}", out(&lint), err(&lint));
    assert!(err(&lint).contains("0 warning"), "{}", err(&lint));
}

#[test]
fn a_rule_that_is_off_or_suppressed_attaches_nothing() {
    let rule = json_rule("arrays-one-per-line", ARRAYS, "[break-all]");
    let source = "{\"a\": [1, 2]}\n";
    let off = project(
        "lint:\n  rules:\n    arrays-one-per-line: \"off\"\n",
        &[&rule],
        &[("a.json", source)],
    );
    assert_eq!(format_twice(&off, "a.json"), "{ \"a\": [1, 2] }\n");

    let suppressed =
        "{\n  // sudsy-disable-next-line arrays-one-per-line\n  \"a\": [1, 2],\n  \"b\": [3]\n}\n";
    let t = project("", &[&rule], &[("a.jsonc", suppressed)]);
    assert_eq!(
        format_twice(&t, "a.jsonc"),
        "{\n  // sudsy-disable-next-line arrays-one-per-line\n  \"a\": [1, 2],\n  \"b\": [\n    3\n  ]\n}\n"
    );
}

#[test]
fn conflicting_rules_are_an_error_naming_both() {
    let a = json_rule("spread", ARRAYS, "[break-all]");
    let b = json_rule("compact", ARRAYS, "[keep-together]");
    let t = project("", &[&a, &b], &[("a.json", "[1, 2]\n")]);
    let format = sudsy(t.path(), &["format", "a.json"]);
    assert!(!format.status.success());
    let message = err(&format);
    assert!(
        message.contains("`spread`")
            && message.contains("`compact`")
            && message.contains("conflicting"),
        "{message}"
    );
    assert_eq!(read(&t, "a.json"), "[1, 2]\n");
}

#[test]
fn a_bad_layout_entry_is_a_config_error_naming_the_file() {
    let rule = json_rule("bad", ARRAYS, "[wiggle]");
    let t = project("", &[&rule], &[("a.json", "[1]\n")]);
    let format = sudsy(t.path(), &["format", "a.json"]);
    assert!(!format.status.success());
    assert!(
        err(&format).contains("r0.yaml") && err(&format).contains("wiggle"),
        "{}",
        err(&format)
    );
    let target =
        "id: t\nlanguage: json\nlayout: ['break-all on $NOPE']\nrule: { pattern: '[$$$A]' }\n";
    let t = project("", &[target], &[("a.json", "[1]\n")]);
    let format = sudsy(t.path(), &["format", "a.json"]);
    assert!(
        err(&format).contains("$NOPE") || err(&format).contains("NOPE"),
        "{}",
        err(&format)
    );
}

#[test]
fn service_builder_chains_break_one_call_per_line() {
    let rule = "id: services-chain\nlanguage: csharp\nlayout: [break-all]\nrule:\n  kind: InvocationExpression\n  regex: '^services\\s*\\.'\n";
    let source = "class C { void M() { services.AddA().AddB(); other.AddA().AddB(); } }\n";
    let t = project("", &[rule], &[("C.cs", source)]);
    let text = format_twice(&t, "C.cs");
    assert!(
        text.contains("services\n") && text.contains(".AddA()\n") && text.contains(".AddB();"),
        "{text}"
    );
    assert!(text.contains("other.AddA().AddB();"), "{text}");
}

/// A project with the sample plugin (its `break-arrays` rule is layout-only) and `rules`.
fn plugin_project(config: &str, rules: &[&str], source: &str) -> tempfile::TempDir {
    let t = project(
        &format!("  wasm: [\"sample.wasm\"]\n{config}"),
        rules,
        &[("a.json", source)],
    );
    fs::copy(
        provider_api::testing::sample_plugin(),
        t.path().join("sample.wasm"),
    )
    .unwrap();
    t
}

#[test]
fn a_wasm_plugin_attaches_directives_under_levels_options_and_suppressions() {
    let source = "{\"a\": [1, 2, 3], \"b\": [4, 5]}\n";
    let t = plugin_project("", &[], source);
    assert_eq!(
        format_twice(&t, "a.json"),
        "{\n  \"a\": [\n    1,\n    2,\n    3\n  ],\n  \"b\": [4, 5]\n}\n"
    );
    // Layout-only: nothing to report when linting.
    let lint = sudsy(t.path(), &["lint", "a.json"]);
    assert!(err(&lint).contains("0 warning"), "{}", err(&lint));

    let option = "lint:\n  rules:\n    sample/break-arrays:\n      level: warn\n      options: { min_items: 1 }\n";
    let t = plugin_project(option, &[], source);
    assert_eq!(
        format_twice(&t, "a.json"),
        "{\n  \"a\": [\n    1,\n    2,\n    3\n  ],\n  \"b\": [\n    4,\n    5\n  ]\n}\n"
    );

    let off = plugin_project(
        "lint:\n  rules:\n    sample/break-arrays: \"off\"\n",
        &[],
        source,
    );
    assert_eq!(
        format_twice(&off, "a.json"),
        "{ \"a\": [1, 2, 3], \"b\": [4, 5] }\n"
    );

    let t = plugin_project(
        "",
        &[],
        "{\n  // sudsy-disable-next-line sample/break-arrays\n  \"a\": [1, 2, 3]\n}\n",
    );
    fs::rename(t.path().join("a.json"), t.path().join("a.jsonc")).unwrap();
    assert_eq!(
        format_twice(&t, "a.jsonc"),
        "{\n  // sudsy-disable-next-line sample/break-arrays\n  \"a\": [1, 2, 3]\n}\n"
    );
}

#[test]
fn a_plugin_and_a_pattern_rule_can_conflict() {
    let keep = json_rule("compact", ARRAYS, "[keep-together]");
    let t = plugin_project("", &[&keep], "[1, 2, 3]\n");
    let format = sudsy(t.path(), &["format", "a.json"]);
    assert!(!format.status.success());
    let message = err(&format);
    assert!(
        message.contains("`compact`") && message.contains("`sample/break-arrays`"),
        "{message}"
    );
}

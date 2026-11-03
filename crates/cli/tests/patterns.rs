//! Pattern rules from `plugins.patterns` in a config, through the CLI: the same lint, fix and
//! `rules` paths as built-in rules, with config levels and suppressions.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn write(dir: &Path, rel: &str, content: &str) {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, content).unwrap();
}

fn out(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).replace("\r\n", "\n")
}

fn err(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).replace("\r\n", "\n")
}

const RULE: &str = "id: no-http
language: yaml
message: \"use https: $U\"
severity: warn
rule:
  pattern: \"url: $U\"
  regex: \"http:\"
fix: \"url: https://example.org\"
fix-kind: safe
";

fn project(config: &str) -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "sudsy.yaml", config);
    write(t.path(), "rules/no-http.yaml", RULE);
    write(
        t.path(),
        "app.yaml",
        "a:\n  url: http://example.org\nb:\n  url: https://example.org\n",
    );
    t
}

const CONFIG: &str = "plugins:\n  patterns: [\"rules/*.yaml\"]\n";

#[test]
fn a_pattern_rule_lints_fixes_and_is_listed() {
    let t = project(CONFIG);
    let lint = sudsy(t.path(), &["lint", "app.yaml"]);
    assert!(
        out(&lint).contains("warning[no-http]"),
        "{}{}",
        out(&lint),
        err(&lint)
    );
    assert!(out(&lint).contains("use https: http://example.org"));

    let rules = sudsy(t.path(), &["rules"]);
    assert!(out(&rules).contains("no-http"), "{}", out(&rules));
    let explain = sudsy(t.path(), &["rules", "no-http"]);
    assert!(
        out(&explain).contains("fix:           safe"),
        "{}",
        out(&explain)
    );

    let fix = sudsy(t.path(), &["fix", "app.yaml"]);
    assert!(fix.status.success(), "{}", err(&fix));
    let fixed = fs::read_to_string(t.path().join("app.yaml")).unwrap();
    assert!(fixed.contains("url: https://example.org"));
    assert!(!fixed.contains("http:"));
}

#[test]
fn the_config_sets_the_level_and_a_comment_suppresses() {
    let t = project(&format!(
        "{CONFIG}yaml:\n  lint:\n    rules:\n      no-http: error\n"
    ));
    let lint = sudsy(t.path(), &["lint", "app.yaml"]);
    assert_eq!(lint.status.code(), Some(1));
    assert!(out(&lint).contains("error[no-http]"));

    write(
        t.path(),
        "app.yaml",
        "a:\n  # sudsy-disable-next-line no-http\n  url: http://example.org\n",
    );
    assert_eq!(
        sudsy(t.path(), &["lint", "app.yaml"]).status.code(),
        Some(0)
    );
}

#[test]
fn unknown_ids_and_broken_rule_files_are_config_errors() {
    let t = project(&format!("{CONFIG}lint:\n  rules:\n    no-htp: warn\n"));
    let lint = sudsy(t.path(), &["lint", "app.yaml"]);
    assert_eq!(lint.status.code(), Some(2));
    assert!(
        err(&lint).contains("no-htp") && err(&lint).contains("no-http"),
        "{}",
        err(&lint)
    );

    let t = project(CONFIG);
    write(
        t.path(),
        "rules/bad.yaml",
        "id: bad\nlanguage: yaml\nmessage: m\nrule: { kind: Nope }\n",
    );
    let lint = sudsy(t.path(), &["lint", "app.yaml"]);
    assert_eq!(lint.status.code(), Some(2));
    assert!(
        err(&lint).contains("bad.yaml") && err(&lint).contains("Nope"),
        "{}",
        err(&lint)
    );

    let t = project(CONFIG);
    write(
        t.path(),
        "rules/dup.yaml",
        &RULE.replace("no-http", "document-start"),
    );
    let lint = sudsy(t.path(), &["lint", "app.yaml"]);
    assert!(err(&lint).contains("built-in"), "{}", err(&lint));

    let t = project("plugins:\n  patterns: [\"rules/missing.yaml\"]\n");
    let lint = sudsy(t.path(), &["lint", "app.yaml"]);
    assert_eq!(lint.status.code(), Some(2));
    assert!(err(&lint).contains("missing.yaml"), "{}", err(&lint));
}

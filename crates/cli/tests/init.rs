//! `sudsy init --from <tool>`: each directory under `tests/fixtures/init/` holds one tool's config
//! files (the directory name is the `--from` value). The generated `sudsy.yaml` and the report
//! are snapshots, and the config must load. Debug builds also check every mapping against the
//! parity catalog while migrating.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/init")
}

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap()
}

fn copy_fixture(tool: &str) -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    for file in fs::read_dir(root().join(tool)).unwrap() {
        let path = file.unwrap().path();
        fs::copy(&path, t.path().join(path.file_name().unwrap())).unwrap();
    }
    t
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).replace("\r\n", "\n")
}

#[test]
fn every_tool_fixture_migrates_to_a_loadable_config() {
    let mut tools = 0;
    for dir in fs::read_dir(root()).unwrap() {
        let tool = dir.unwrap().file_name().to_string_lossy().into_owned();
        let t = copy_fixture(&tool);
        let out = sudsy(t.path(), &["init", "--from", &tool]);
        assert_eq!(out.status.code(), Some(0), "{tool}: {}", text(&out.stderr));
        let yaml = fs::read_to_string(t.path().join("sudsy.yaml")).unwrap();
        insta::assert_snapshot!(tool.clone(), format!("{yaml}---\n{}", text(&out.stdout)));
        let printed = sudsy(t.path(), &["config", "print"]);
        assert_eq!(
            printed.status.code(),
            Some(0),
            "{tool}: {}",
            text(&printed.stderr)
        );
        tools += 1;
    }
    assert_eq!(tools, 7);
}

#[test]
fn an_existing_config_is_kept_without_force() {
    let t = copy_fixture("csharpier");
    fs::write(t.path().join("sudsy.yaml"), "analysis: fast\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "csharpier"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        fs::read_to_string(t.path().join("sudsy.yaml")).unwrap(),
        "analysis: fast\n"
    );
    let out = sudsy(t.path(), &["init", "--from", "csharpier", "--force"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(
        fs::read_to_string(t.path().join("sudsy.yaml"))
            .unwrap()
            .contains("csharpier-compat")
    );
}

#[test]
fn a_javascript_eslint_config_is_refused_with_an_explanation() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("eslint.config.js"), "export default [];\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "eslint-jsonc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("eslint.config.js is JavaScript"),
        "{}",
        text(&out.stderr)
    );
    assert!(!t.path().join("sudsy.yaml").exists());
}

#[test]
fn a_javascript_stylelint_config_is_refused_with_an_explanation() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("stylelint.config.js"), "export default {};\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "stylelint"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains("stylelint.config.js is JavaScript"),
        "{}",
        text(&out.stderr)
    );
}

#[test]
fn a_missing_oversized_or_broken_config_is_an_error() {
    let t = tempfile::tempdir().unwrap();
    assert_eq!(
        sudsy(t.path(), &["init", "--from", "yamllint"])
            .status
            .code(),
        Some(2)
    );
    fs::write(t.path().join(".yamllint"), "rules: [unterminated\n").unwrap();
    let out = sudsy(t.path(), &["init", "--from", "yamllint"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        text(&out.stderr).contains(".yamllint"),
        "{}",
        text(&out.stderr)
    );
    fs::write(t.path().join(".yamllint"), "#".repeat(2 << 20)).unwrap();
    let out = sudsy(t.path(), &["init", "--from", "yamllint"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("limit"), "{}", text(&out.stderr));
}

#[test]
fn write_editorconfig_appends_once() {
    let t = copy_fixture("csharpier");
    let out = sudsy(
        t.path(),
        &["init", "--from", "csharpier", "--write-editorconfig"],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let first = fs::read_to_string(t.path().join(".editorconfig")).unwrap();
    assert!(first.contains("dotnet_diagnostic.IDE0055.severity = none"));
    let out = sudsy(
        t.path(),
        &[
            "init",
            "--from",
            "csharpier",
            "--force",
            "--write-editorconfig",
        ],
    );
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        fs::read_to_string(t.path().join(".editorconfig")).unwrap(),
        first
    );
    let out = sudsy(
        t.path(),
        &["init", "--from", "yamllint", "--write-editorconfig"],
    );
    assert_eq!(out.status.code(), Some(2));
}

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
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

fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

const MESSY: &str = "{\"a\":   1,\n \"b\":[1,2]}";
const CLEAN: &str = "{ \"a\": 1, \"b\": [1, 2] }\n";

#[test]
fn formats_files_in_place() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", MESSY);
    write(t.path(), "sub/b.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(t.path(), "a.json"), CLEAN);
    assert_eq!(read(t.path(), "sub/b.json"), CLEAN);
    let again = sudsy(t.path(), &["--check"]);
    assert_eq!(again.status.code(), Some(0));
}

#[test]
fn check_reports_and_leaves_files_alone() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", MESSY);
    write(t.path(), "ok.json", CLEAN);
    let out = sudsy(t.path(), &["--check"]);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("would reformat a.json"), "{stdout}");
    assert!(!stdout.contains("ok.json"));
    assert_eq!(read(t.path(), "a.json"), MESSY);
}

#[test]
fn respects_gitignore_skips_other_languages_and_takes_explicit_paths() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), ".gitignore", "ignored/\n");
    write(t.path(), "ignored/a.json", MESSY);
    write(t.path(), "Program.cs", "class  A{}");
    write(t.path(), "notes.txt", "x");
    write(t.path(), "keep.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(read(t.path(), "ignored/a.json"), MESSY);
    assert_eq!(read(t.path(), "Program.cs"), "class  A{}");
    assert_eq!(read(t.path(), "keep.json"), CLEAN);
    let explicit = sudsy(t.path(), &["ignored/a.json"]);
    assert_eq!(explicit.status.code(), Some(0));
    assert_eq!(read(t.path(), "ignored/a.json"), CLEAN);
}

#[test]
fn syntax_errors_fail_without_touching_the_file() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "bad.json", "{\"a\": }");
    write(t.path(), "good.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("bad.json") && stderr.contains("1:7"),
        "{stderr}"
    );
    assert_eq!(read(t.path(), "bad.json"), "{\"a\": }");
    assert_eq!(read(t.path(), "good.json"), CLEAN);
}

#[test]
fn config_and_dialects_apply() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "format:\n  indent_size: 3\njson:\n  format:\n    trailing_commas: multiline\n    collapse: never\n",
    );
    write(t.path(), "a.json", "{\"a\":[1]}");
    write(t.path(), "tsconfig.json", "{\"a\":[1]}");
    write(t.path(), ".vscode/settings.json", "// c\n{\"a\":1,}");
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        read(t.path(), "a.json"),
        "{\n   \"a\": [\n      1\n   ]\n}\n"
    );
    assert_eq!(
        read(t.path(), "tsconfig.json"),
        "{\n   \"a\": [\n      1,\n   ],\n}\n"
    );
    assert_eq!(
        read(t.path(), ".vscode/settings.json"),
        "// c\n{\n   \"a\": 1,\n}\n"
    );
}

#[test]
fn dialect_option_overrides_the_file_name() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    dialect: jsonc\n",
    );
    write(t.path(), "a.json", "[1, // one\n 2]");
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(t.path(), "a.json"), "[\n  1, // one\n  2\n]\n");
}

#[test]
fn invalid_option_values_are_reported() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    collapse: sometimes\n",
    );
    write(t.path(), "a.json", "{}");
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("collapse"));
}

#[test]
fn keeps_crlf_and_byte_order_mark() {
    let t = tempfile::tempdir().unwrap();
    fs::write(t.path().join("a.json"), "\u{feff}{\"a\":\r\n1}").unwrap();
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(0));
    let bytes = fs::read(t.path().join("a.json")).unwrap();
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "\u{feff}{ \"a\": 1 }\r\n"
    );
}

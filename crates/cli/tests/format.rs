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
const CLEAN: &str = "{\n  \"a\": 1,\n  \"b\": [1, 2]\n}\n";

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
    write(t.path(), "notes.txt", "x  ");
    write(t.path(), "keep.json", MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(read(t.path(), "ignored/a.json"), MESSY);
    assert_eq!(read(t.path(), "notes.txt"), "x  ");
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
        "\u{feff}{\r\n  \"a\": 1\r\n}\r\n"
    );
}

const YAML_MESSY: &str = "a:   1
list:
    - x
    -   y
m: {b: 2}
";
const YAML_CLEAN: &str = "a: 1
list:
  - x
  - y
m: { b: 2 }
";

#[test]
fn formats_yaml_files() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.yaml", YAML_MESSY);
    write(t.path(), "sub/b.yml", YAML_MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(read(t.path(), "a.yaml"), YAML_CLEAN);
    assert_eq!(read(t.path(), "sub/b.yml"), YAML_CLEAN);
    assert_eq!(sudsy(t.path(), &["--check"]).status.code(), Some(0));
}

#[test]
fn the_config_file_is_formatted_like_any_other_yaml_file() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "yaml:
    format:
        indent_size: 4
    # keep
format:   {line_width: 100}
",
    );
    write(
        t.path(),
        "a.yaml",
        "k:
  - x
",
    );
    let out = sudsy(t.path(), &[]);
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    // The file's own `yaml:` section applies to it and to the other YAML files.
    assert_eq!(
        read(t.path(), "sudsy.yaml"),
        "yaml:
    format:
        indent_size: 4
    # keep
format: { line_width: 100 }
"
    );
    assert_eq!(
        read(t.path(), "a.yaml"),
        "k:
    - x
"
    );
}

#[test]
fn yaml_syntax_errors_fail_without_touching_the_file() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "bad.yaml",
        "a: b: c
",
    );
    write(t.path(), "ok.yaml", YAML_MESSY);
    let out = sudsy(t.path(), &[]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        read(t.path(), "bad.yaml"),
        "a: b: c
"
    );
    assert_eq!(read(t.path(), "ok.yaml"), YAML_CLEAN);
}

/// The C# worker, built once for the test process.
fn csharp_worker() -> std::path::PathBuf {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
    let status = Command::new("dotnet")
        .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
        .arg(&project)
        .status()
        .expect("dotnet is installed");
    assert!(status.success(), "dotnet build failed");
    project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
}

#[test]
fn formats_csharp_files_with_the_worker() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "Program.cs", "class  A{void M(){x();}}");
    write(t.path(), "bad.cs", "class A {");
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", csharp_worker())
        .current_dir(t.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        read(t.path(), "Program.cs"),
        "class A
{
    void M()
    {
        x();
    }
}
"
    );
    assert_eq!(read(t.path(), "bad.cs"), "class A {");
    assert!(String::from_utf8_lossy(&out.stderr).contains("bad.cs"));
}

#[test]
fn csharp_without_a_worker_fails_the_file_and_formats_the_rest() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "Program.cs", "class A{}");
    write(t.path(), "a.json", MESSY);
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", t.path().join("missing.dll"))
        .current_dir(t.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(read(t.path(), "Program.cs"), "class A{}");
    assert_eq!(read(t.path(), "a.json"), CLEAN);
}

#[test]
fn missing_dotnet_is_one_clear_error_and_other_languages_continue() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", MESSY);
    write(t.path(), "a.cs", "class A{}");
    write(t.path(), "worker.dll", "");
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", t.path().join("worker.dll"))
        .env("PATH", t.path().join("empty"))
        .current_dir(t.path())
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("Sudsy needs the .NET 10 runtime to format and lint C#. JSON and YAML keep working. Download it from https://dotnet.microsoft.com/download/dotnet/10.0"),
        "{stderr}"
    );
    assert_eq!(read(t.path(), "a.json"), CLEAN);
}

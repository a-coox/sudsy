//! Configuration through the CLI: overrides, `.editorconfig`, `include` / `exclude`, error
//! messages, `config print`, `init` and the C# preprocessor symbols.

use std::fs;
use std::path::{Path, PathBuf};
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

fn read(dir: &Path, rel: &str) -> String {
    fs::read_to_string(dir.join(rel)).unwrap()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

const JSON: &str = "{\"a\":{\"b\":[1,2]}}";

#[test]
fn overrides_apply_by_glob_and_beat_the_language_section() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    collapse: never\n    indent_size: 4\noverrides:\n  - files: [\"wide/**\"]\n    json:\n      format:\n        collapse: always\n        indent_size: 2\n",
    );
    write(t.path(), "a.json", JSON);
    write(t.path(), "wide/b.json", JSON);
    let out = sudsy(t.path(), &["format"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        read(t.path(), "a.json"),
        "{\n    \"a\": {\n        \"b\": [\n            1,\n            2\n        ]\n    }\n}\n"
    );
    assert_eq!(
        read(t.path(), "wide/b.json"),
        "{ \"a\": { \"b\": [1, 2] } }\n"
    );
}

#[test]
fn editorconfig_is_the_fallback_below_the_config() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        ".editorconfig",
        "root = true\n[*.json]\nindent_size = 3\n[sub/*.json]\nindent_size = 5\n",
    );
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    collapse: never\n",
    );
    write(t.path(), "a.json", "{\"a\":1}");
    write(t.path(), "sub/b.json", "{\"a\":1}");
    let out = sudsy(t.path(), &["format"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(read(t.path(), "a.json"), "{\n   \"a\": 1\n}\n");
    assert_eq!(read(t.path(), "sub/b.json"), "{\n     \"a\": 1\n}\n");
    // The config file beats .editorconfig.
    write(
        t.path(),
        "sudsy.yaml",
        "format:\n  indent_size: 2\njson:\n  format:\n    collapse: never\n",
    );
    write(t.path(), "a.json", "{\"a\":1}");
    assert_eq!(sudsy(t.path(), &["format"]).status.code(), Some(0));
    assert_eq!(read(t.path(), "a.json"), "{\n  \"a\": 1\n}\n");
}

#[test]
fn include_and_exclude_filter_what_is_found_but_not_named_files() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "include: [\"src/**\"]\nexclude: [\"src/gen\", \"**/*.skip.json\"]\n",
    );
    for f in [
        "src/a.json",
        "src/gen/b.json",
        "src/c.skip.json",
        "other/d.json",
    ] {
        write(t.path(), f, JSON);
    }
    let out = sudsy(t.path(), &["format", "--check"]);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(stdout(&out), "would reformat src/a.json\n");
    // A file named on the command line is always formatted.
    let out = sudsy(t.path(), &["format", "--check", "other/d.json"]);
    assert_eq!(stdout(&out), "would reformat other/d.json\n");
}

#[test]
fn paths_print_relative_to_the_working_directory_with_slashes() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "deep/er/a.json", JSON);
    write(t.path(), "deep/er/bad.json", "{");
    let out = sudsy(t.path(), &["format", "--check"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(
        stdout(&out).contains("would reformat deep/er/a.json\n"),
        "{}",
        stdout(&out)
    );
    assert!(
        stderr(&out).contains("error: deep/er/bad.json:"),
        "{}",
        stderr(&out)
    );
    // From a subdirectory, with a path above it.
    let sub = t.path().join("deep");
    let out = sudsy(&sub, &["format", "--check", "er/a.json"]);
    assert!(
        stdout(&out).contains("would reformat er/a.json\n"),
        "{}",
        stdout(&out)
    );
    let up = sudsy(&sub.join("er"), &["format", "--check", "../er/a.json"]);
    assert!(
        stdout(&up).contains("would reformat a.json\n"),
        "{}",
        stdout(&up)
    );
}

#[test]
fn config_errors_name_the_file_and_position_and_suggest() {
    let t = tempfile::tempdir().unwrap();
    write(t.path(), "a.json", JSON);
    write(
        t.path(),
        "sudsy.yaml",
        "json:\n  format:\n    colapse: never\n",
    );
    let out = sudsy(t.path(), &["format"]);
    assert_eq!(out.status.code(), Some(2));
    let message = stderr(&out);
    assert!(message.contains("sudsy.yaml:3:"), "{message}");
    assert!(message.contains("did you mean `collapse`"), "{message}");
    assert_eq!(read(t.path(), "a.json"), JSON);

    write(t.path(), "sudsy.yaml", "formt:\n  line_width: 100\n");
    let message = stderr(&sudsy(t.path(), &["format"]));
    assert!(message.contains("sudsy.yaml:1:1"), "{message}");
    assert!(message.contains("did you mean `format`"), "{message}");

    write(t.path(), "sudsy.yaml", "extends: [preset:json/nope]\n");
    let message = stderr(&sudsy(t.path(), &["format"]));
    assert!(message.contains("json/prettier-compat"), "{message}");

    write(t.path(), "sudsy.yaml", "extends: [base/missing.yaml]\n");
    let message = stderr(&sudsy(t.path(), &["format"]));
    assert!(message.contains("sudsy.yaml:1:"), "{message}");
    assert!(message.contains("not found"), "{message}");

    // An error in an extended file names that file.
    write(
        t.path(),
        "base/shared.yaml",
        "json:\n  format:\n    indent_size: 99\n",
    );
    write(t.path(), "sudsy.yaml", "extends: [base/shared.yaml]\n");
    let message = stderr(&sudsy(t.path(), &["format"]));
    assert!(message.contains("base/shared.yaml:3:"), "{message}");
    assert!(message.contains("between 1 and 16"), "{message}");
}

#[test]
fn extends_a_file_in_another_directory() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "team/base.yaml",
        "extends: [preset:json/default]\njson:\n  format:\n    collapse: never\n    indent_size: 8\n",
    );
    write(
        t.path(),
        "sudsy.yaml",
        "extends: [team/base.yaml]\njson:\n  format:\n    indent_size: 1\n",
    );
    write(t.path(), "a.json", "{\"a\":1}");
    assert_eq!(sudsy(t.path(), &["format"]).status.code(), Some(0));
    assert_eq!(read(t.path(), "a.json"), "{\n \"a\": 1\n}\n");
}

#[test]
fn config_print_shows_the_value_and_where_it_came_from() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "extends: [preset:yaml/prettier-compat]\nformat:\n  line_width: 90\nlint:\n  rules:\n    max-line-length: warn\nyaml:\n  format:\n    indent_size: 4\n  lint:\n    rules:\n      ambiguous-booleans: error\noverrides:\n  - files: [\"ci/**\"]\n    yaml:\n      format:\n        indent_size: 3\n",
    );
    write(
        t.path(),
        ".editorconfig",
        "[*.yaml]\nfinal_newline = x\ninsert_final_newline = false\n",
    );
    write(t.path(), "ci/a.yaml", "a: 1\n");
    write(t.path(), "b.yaml", "a: 1\n");
    let out = sudsy(t.path(), &["config", "print", "ci/a.yaml"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains("config: sudsy.yaml"), "{text}");
    assert!(text.contains("file: ci/a.yaml"), "{text}");
    assert!(text.contains("indent_size") && text.contains("3"), "{text}");
    assert!(
        text.contains("(sudsy.yaml:17:22 (overrides[0].yaml.format))"),
        "{text}"
    );
    assert!(text.contains("(sudsy.yaml:3:15 (format))"), "{text}");
    assert!(text.contains("(preset yaml/prettier-compat)"), "{text}");
    assert!(
        text.contains("(.editorconfig:3 (insert_final_newline))"),
        "{text}"
    );
    assert!(text.contains("ambiguous-booleans"), "{text}");
    assert!(text.contains("max-line-length"), "{text}");
    assert!(
        !text.contains("layout"),
        "internal values stay hidden: {text}"
    );
    // Outside the override the language section wins.
    let other = stdout(&sudsy(t.path(), &["config", "print", "b.yaml"]));
    assert!(other.contains("(sudsy.yaml:9:18 (yaml.format))"), "{other}");
    // Without a file: every language.
    let all = stdout(&sudsy(t.path(), &["config", "print"]));
    for language in ["csharp", "json", "yaml"] {
        assert!(
            all.contains(&format!("{language} format options:")),
            "{all}"
        );
    }
    let bad = sudsy(t.path(), &["config", "print", "notes.txt"]);
    assert_eq!(bad.status.code(), Some(2));
}

#[test]
fn init_writes_a_starter_config_and_will_not_overwrite() {
    let t = tempfile::tempdir().unwrap();
    let out = sudsy(t.path(), &["init"]);
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let text = read(t.path(), "sudsy.yaml");
    assert!(text.contains("preset:csharp/microsoft"), "{text}");
    assert!(text.contains("preset:yaml/default"), "{text}");
    // The starter loads.
    write(t.path(), "a.json", JSON);
    assert_eq!(sudsy(t.path(), &["format"]).status.code(), Some(0));
    assert_eq!(sudsy(t.path(), &["init"]).status.code(), Some(2));

    let t = tempfile::tempdir().unwrap();
    let out = sudsy(
        t.path(),
        &[
            "init",
            "--preset",
            "csharp/google",
            "--preset",
            "json/prettier-compat",
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        read(t.path(), "sudsy.yaml"),
        "# https://github.com/a-coox/sudsy\nextends:\n  - preset:csharp/google\n  - preset:json/prettier-compat\n"
    );
    let t = tempfile::tempdir().unwrap();
    let out = sudsy(t.path(), &["init", "--preset", "csharp/nope"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("csharp/google"), "{}", stderr(&out));
    assert!(!t.path().join("sudsy.yaml").exists());
}

/// Builds the worker once per test process (tests run in parallel threads).
fn csharp_worker() -> PathBuf {
    static DLL: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    DLL.get_or_init(|| {
        let project =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
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

#[test]
fn unmapped_editorconfig_formatting_keys_warn_once() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        ".editorconfig",
        "root = true\n[*.cs]\ncsharp_space_after_cast = true\ncsharp_space_after_comma = true\ndotnet_style_qualification_for_field = true\n",
    );
    write(t.path(), "A.cs", "class A { }\n");
    write(t.path(), "B.cs", "class B { }\n");
    let out = Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .arg("format")
        .env("SUDSY_CSHARP_WORKER", csharp_worker())
        .current_dir(t.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let warnings: Vec<_> = stderr(&out)
        .lines()
        .filter(|l| l.starts_with("warning:"))
        .map(str::to_string)
        .collect();
    assert_eq!(
        warnings,
        [
            "warning: .editorconfig:3: `csharp_space_after_cast = true` is not supported by Sudsy and is ignored"
        ]
    );
}

#[test]
fn preprocessor_symbols_from_the_config_select_the_branch_that_parses() {
    let t = tempfile::tempdir().unwrap();
    // The shape of the four CoreLib files: the `#else` branch is an `#error`.
    let source = "#if SUPPORTED\nclass  A{}\n#else\n#error unsupported\n#endif\n";
    write(t.path(), "A.cs", source);
    let run = |dir: &Path| {
        Command::new(env!("CARGO_BIN_EXE_sudsy"))
            .arg("format")
            .env("SUDSY_CSHARP_WORKER", csharp_worker())
            .current_dir(dir)
            .output()
            .unwrap()
    };
    let out = run(t.path());
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("A.cs"), "{}", stderr(&out));
    assert_eq!(read(t.path(), "A.cs"), source);

    write(
        t.path(),
        "sudsy.yaml",
        "csharp:\n  format:\n    preprocessor_symbols: [SUPPORTED]\n",
    );
    let out = run(t.path());
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert!(read(t.path(), "A.cs").starts_with("#if SUPPORTED\nclass A { }\n#else\n"));

    // Per-directory symbols through an override.
    write(
        t.path(),
        "sudsy.yaml",
        "overrides:\n  - files: [\"net8/**\"]\n    csharp:\n      format:\n        preprocessor_symbols: [SUPPORTED]\n",
    );
    write(t.path(), "net8/A.cs", source);
    write(t.path(), "A.cs", source);
    let out = run(t.path());
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(read(t.path(), "A.cs"), source);
    assert!(read(t.path(), "net8/A.cs").contains("class A { }"));

    write(
        t.path(),
        "sudsy.yaml",
        "csharp:\n  format:\n    preprocessor_symbols: [\"not a symbol\"]\n",
    );
    let message = stderr(&run(t.path()));
    assert!(message.contains("sudsy.yaml:3:"), "{message}");
}

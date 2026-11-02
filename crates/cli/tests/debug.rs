use std::process::Command;

fn sudsy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
}

fn worker_dll() -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let project = root.join("workers/csharp/Sudsy.Worker");
    let status = Command::new("dotnet")
        .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
        .arg(&project)
        .status()
        .unwrap();
    assert!(status.success());
    project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
}

#[test]
fn debug_parse_prints_the_tree_and_checks_the_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("A.cs");
    std::fs::write(&file, "class A { } // end\n").unwrap();
    let out = sudsy()
        .args(["debug", "parse"])
        .arg(&file)
        .env("SUDSY_CSHARP_WORKER", worker_dll())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8(out.stdout).unwrap();
    assert!(stdout.starts_with("CompilationUnit 0..19\n"), "{stdout}");
    assert!(stdout.contains("ClassDeclaration"), "{stdout}");
    assert!(stdout.contains("SingleLineCommentTrivia"), "{stdout}");
    assert!(String::from_utf8_lossy(&out.stderr).contains("round-trip ok"));
}

#[test]
fn debug_parse_rejects_other_languages() {
    let out = sudsy().args(["debug", "parse", "a.json"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("only .cs"));
}

#[test]
fn debug_config_prints_and_reports_errors() {
    let dir = tempfile::tempdir().unwrap();
    let good = dir.path().join("sudsy.yaml");
    std::fs::write(&good, "format:\n  line_width: 90\n").unwrap();
    let out = sudsy()
        .args(["debug", "config"])
        .arg(&good)
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("line_width"));

    std::fs::write(&good, "nope: 1\n").unwrap();
    let out = sudsy()
        .args(["debug", "config"])
        .arg(&good)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("sudsy.yaml:1:"));
}

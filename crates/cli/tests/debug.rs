use std::process::Command;

fn sudsy() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
}

#[test]
fn debug_parse_prints_the_tree_and_checks_the_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("A.cs");
    std::fs::write(&file, "class A { } // end\n").unwrap();
    let out = sudsy()
        .args(["debug", "parse"])
        .arg(&file)
        .env("SUDSY_CSHARP_WORKER", lang_csharp::testing::built_worker())
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
    assert!(String::from_utf8_lossy(&out.stderr).contains("takes a .cs file"));
}

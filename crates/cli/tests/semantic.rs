//! Semantic rules through the CLI: the analysis depths, the shared workspace, project scope and
//! the `semantics:` config.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn csharp_worker() -> PathBuf {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
    let status = Command::new("dotnet")
        .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
        .arg(&project)
        .status()
        .expect("dotnet is installed");
    assert!(status.success(), "dotnet build failed");
    project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
}

fn write(dir: &Path, rel: &str, content: &str) {
    fs::write(dir.join(rel), content).unwrap();
}

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .env("SUDSY_CSHARP_WORKER", csharp_worker())
        .current_dir(dir)
        .output()
        .unwrap()
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).replace('\r', "")
}

fn project() -> tempfile::TempDir {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    remove-unused-usings: warn\n",
    );
    write(
        t.path(),
        "Lib.cs",
        "namespace Lib { public class Widget { } }\nnamespace Other { public class Gadget { } }\n",
    );
    write(
        t.path(),
        "App.cs",
        "using Lib;\nusing Other;\nclass App {\n  Widget w;\n  int F() {\n    int unused;\n    return 1;\n    unused = 2;\n  }\n}\n",
    );
    t
}

#[test]
fn standard_depth_binds_across_files_and_reports_flow_findings() {
    let t = project();
    let out = sudsy(t.path(), &["lint", "App.cs", "Lib.cs"]);
    let text = stdout(&out);
    // `Lib` is used by a type declared in another file; `Other` is not used at all.
    assert!(text.contains("unnecessary using directive"), "{text}");
    assert_eq!(
        text.matches("unnecessary using directive").count(),
        1,
        "{text}"
    );
    assert!(text.contains("App.cs:2:1"), "{text}");
    assert!(text.contains("variable `unused`"), "{text}");
    assert!(text.contains("unreachable code"), "{text}");
}

#[test]
fn fast_depth_is_syntactic_only() {
    let t = project();
    let out = sudsy(
        t.path(),
        &["lint", "--analysis", "fast", "App.cs", "Lib.cs"],
    );
    let text = stdout(&out);
    assert!(text.contains("unreachable code"), "{text}");
    assert!(!text.contains("unnecessary using"), "{text}");
    assert!(!text.contains("variable `unused`"), "{text}");
}

#[test]
fn deep_depth_adds_project_scope_rules_across_partial_files() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "A.cs",
        "partial class P { private int used; private int lonely; private void Helper() { } }\n",
    );
    write(
        t.path(),
        "B.cs",
        "partial class P { int Get() { Helper(); return used; } }\n",
    );
    let standard = stdout(&sudsy(t.path(), &["lint"]));
    assert!(!standard.contains("never used"), "{standard}");
    let deep = stdout(&sudsy(t.path(), &["lint", "--analysis", "deep"]));
    assert!(
        deep.contains("private member `lonely` is never used"),
        "{deep}"
    );
    assert!(
        !deep.contains("`used`") && !deep.contains("`Helper`"),
        "{deep}"
    );
}

#[test]
fn user_summaries_make_calls_that_do_not_return_end_the_flow() {
    let t = tempfile::tempdir().unwrap();
    let code = "class Log { public static void Fatal(string m) { throw null; } }\nclass A { void F() { Log.Fatal(\"x\"); F(); } }\n";
    write(t.path(), "A.cs", code);
    assert!(!stdout(&sudsy(t.path(), &["lint"])).contains("unreachable"));
    write(
        t.path(),
        "sudsy.yaml",
        "csharp:\n  semantics:\n    does_not_return: [\"Log.Fatal\"]\n",
    );
    assert!(stdout(&sudsy(t.path(), &["lint"])).contains("unreachable code"));
}

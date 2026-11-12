//! Semantic rules through the CLI: the analysis depths, the shared workspace, project scope and
//! the `semantics:` config.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn write(dir: &Path, rel: &str, content: &str) {
    fs::write(dir.join(rel), content).unwrap();
}

fn sudsy(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_sudsy"))
        .args(args)
        .env("SUDSY_CSHARP_WORKER", lang_csharp::testing::built_worker())
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
        "lint:\n  rules:\n    remove-unused-usings: warn\n    unreachable-code: warn\n    unused-variable: warn\n",
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
fn excluded_files_are_not_part_of_the_semantic_workspace() {
    let t = tempfile::tempdir().unwrap();
    fs::create_dir(t.path().join("gen")).unwrap();
    write(
        t.path(),
        "gen/Lib.cs",
        "public class Widget { }
",
    );
    write(
        t.path(),
        "App.cs",
        "using System.Text;
class App { Widget w; }
",
    );
    let config = |exclude: &str| {
        write(
            t.path(),
            "sudsy.yaml",
            &format!(
                "{exclude}lint:
  rules:
    remove-unused-usings: warn
"
            ),
        );
        stdout(&sudsy(t.path(), &["lint"]))
    };
    assert!(config("").contains("unnecessary using"));
    // `Widget` does not resolve without gen/Lib.cs: the file has errors, so the rule is silent.
    assert!(
        !config(
            "exclude: [\"gen/**\"]
"
        )
        .contains("unnecessary using")
    );
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
fn debug_message_rules_degrade_to_the_written_name_at_fast_depth() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    debug-assert-message: warn\n    debug-fail-message: warn\n    use-tuple-syntax: warn\n",
    );
    // `Debug` is the file's own class here: only the name says otherwise.
    write(
        t.path(),
        "D.cs",
        "class Debug { public static void Assert(bool b) { } }\nclass A {\n  System.ValueTuple<int, int> p;\n  void F(bool b) {\n  Debug.Assert(b);\n  Debug.Fail(\"\");\n} }\n",
    );
    let fast = stdout(&sudsy(t.path(), &["lint", "--analysis", "fast", "D.cs"]));
    assert!(fast.contains("Debug.Assert should provide"), "{fast}");
    assert!(fast.contains("Debug.Fail should provide"), "{fast}");
    assert!(!fast.contains("tuple syntax"), "{fast}");
    let standard = stdout(&sudsy(t.path(), &["lint", "D.cs"]));
    assert!(
        !standard.contains("Debug.Assert should provide"),
        "{standard}"
    );
    assert!(standard.contains("tuple syntax"), "{standard}");
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
    let rules = "lint:\n  rules:\n    unreachable-code: warn\n";
    write(t.path(), "sudsy.yaml", rules);
    write(t.path(), "A.cs", code);
    assert!(!stdout(&sudsy(t.path(), &["lint"])).contains("unreachable"));
    write(
        t.path(),
        "sudsy.yaml",
        &format!("{rules}csharp:\n  semantics:\n    does_not_return: [\"Log.Fatal\"]\n"),
    );
    assert!(stdout(&sudsy(t.path(), &["lint"])).contains("unreachable code"));
}

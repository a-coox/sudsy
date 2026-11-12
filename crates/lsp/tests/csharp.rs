//! C# through the server: one long-lived worker serves a whole editing session.

use crate::common::{Client, apply};
use lsp_types::{Position, Range};
use provider_api::LanguageProvider;
use std::path::Path;

fn providers(worker: &Path) -> Vec<Box<dyn LanguageProvider>> {
    vec![
        Box::new(lang_json::JsonProvider::new()),
        Box::new(lang_csharp::provider(worker)),
    ]
}

#[test]
fn edits_diagnostics_and_formatting_share_one_worker() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("sudsy.yaml"),
        "lint:\n  rules:\n    no-empty-catch: error\n",
    )
    .unwrap();
    let mut client = Client::start(t.path(), providers(&lang_csharp::testing::built_worker()));
    let text = "class A{void M(){try{}catch{}}}";
    let uri = client.open("A.cs", "csharp", text);

    let found = client.diagnostics(&uri);
    assert_eq!(found.len(), 1, "{found:?}");
    let formatted = apply(text, &client.format(&uri));
    assert!(formatted.contains("class A\n{"), "{formatted}");

    // Fill the catch block with an incremental edit; the worker parses the new version.
    let at = text.find("catch{}").unwrap() as u32 + 6;
    client.change(
        &uri,
        2,
        Some(Range::new(Position::new(0, at), Position::new(0, at))),
        " throw; ",
    );
    assert!(client.diagnostics(&uri).is_empty());

    // Syntax errors are diagnostics, and formatting leaves the file alone.
    client.change(&uri, 3, None, "class A {");
    assert!(!client.diagnostics(&uri).is_empty());
    assert!(client.format(&uri).is_empty());
    client.shutdown();
}

#[test]
fn a_missing_worker_is_reported_once_and_other_languages_keep_working() {
    let t = tempfile::tempdir().unwrap();
    let missing = t.path().join("missing.dll");
    // `dotnet` starts but the assembly does not exist, so the worker exits before its handshake.
    let mut client = Client::start(t.path(), providers(&missing));
    let uri = client.open("A.cs", "csharp", "class A { }");
    assert!(client.diagnostics(&uri).is_empty());
    let message = client.wait_for("window/showMessage");
    assert!(
        message["message"].as_str().unwrap().starts_with("Sudsy: "),
        "{message}"
    );
    assert!(client.diagnostics(&uri).is_empty());
    assert!(client.notes.iter().all(|(m, _)| m != "window/showMessage"));

    let json = client.open("a.json", "json", "{\"a\": 1, \"a\": 2}");
    assert_eq!(client.diagnostics(&json).len(), 1);
    client.shutdown();
}

#[test]
fn semantic_rules_run_in_the_editor_and_follow_edits() {
    let t = tempfile::tempdir().unwrap();
    // Off by default: the compiler reports the same.
    std::fs::write(
        t.path().join("sudsy.yaml"),
        "lint:\n  rules:\n    unused-variable: warn\n",
    )
    .unwrap();
    let mut client = Client::start(t.path(), providers(&lang_csharp::testing::built_worker()));
    let text = "class A { int F() { int k = 5; return 1; } }";
    let uri = client.open("A.cs", "csharp", text);
    let found = client.diagnostics(&uri);
    assert!(
        found.iter().any(|d| d.message.contains("variable `k`")),
        "{found:?}"
    );
    // Using the variable lifts the diagnostic: the worker's workspace saw the edit.
    let at = text.find("return 1").unwrap() as u32;
    client.change(
        &uri,
        2,
        Some(Range::new(Position::new(0, at), Position::new(0, at + 8))),
        "return k",
    );
    assert!(client.diagnostics(&uri).is_empty());
}

#[test]
fn deep_diagnostics_use_every_file_of_the_project_not_only_the_open_ones() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(t.path().join("sudsy.yaml"), "analysis: deep\n").unwrap();
    std::fs::write(
        t.path().join("A.cs"),
        "partial class P { private int used; private int lonely; }\n",
    )
    .unwrap();
    std::fs::write(
        t.path().join("B.cs"),
        "partial class P { int Get() { return used; } }\n",
    )
    .unwrap();
    let mut client = Client::start(t.path(), providers(&lang_csharp::testing::built_worker()));
    let uri = client.open(
        "A.cs",
        "csharp",
        "partial class P { private int used; private int lonely; }\n",
    );
    let found = client.diagnostics(&uri);
    let messages: Vec<_> = found.iter().map(|d| d.message.as_str()).collect();
    assert!(
        messages
            .iter()
            .any(|m| m.contains("`lonely` is never used")),
        "{messages:?}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("`used`")),
        "{messages:?}"
    );
}

#[test]
fn the_unity_preset_counts_message_methods_of_derived_types_as_used() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("sudsy.yaml"),
        "analysis: deep
extends: [preset:csharp/unity]
",
    )
    .unwrap();
    let text = "class Base : MonoBehaviour { }
class P : Base { void Update() { } void Lonely() { } }
class Q { void Update() { } }
";
    std::fs::write(t.path().join("A.cs"), text).unwrap();
    let mut client = Client::start(t.path(), providers(&lang_csharp::testing::built_worker()));
    let uri = client.open("A.cs", "csharp", text);
    let found = client.diagnostics(&uri);
    let mut messages: Vec<_> = found.iter().map(|d| d.message.as_str()).collect();
    messages.retain(|m| m.contains("never used"));
    assert_eq!(
        messages,
        [
            "private member `Lonely` is never used",
            "private member `Update` is never used"
        ],
        "{found:?}"
    );
}

#[test]
fn a_document_the_config_excludes_gets_no_diagnostics() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("sudsy.yaml"),
        "exclude: [\"Library/**\"]
format:
  indent_size: 2
",
    )
    .unwrap();
    std::fs::create_dir(t.path().join("Library")).unwrap();
    let mut client = Client::start(t.path(), providers(&lang_csharp::testing::built_worker()));
    let text = "class A { void M() { int a; } }
";
    let kept = client.open("Library/A.cs", "csharp", text);
    assert!(client.diagnostics(&kept).is_empty());
    assert!(client.format(&kept).is_empty());
}

#[test]
fn a_missing_runtime_is_prompted_once_and_the_action_opens_the_download() {
    let t = tempfile::tempdir().unwrap();
    let missing = lang_csharp::provider_with_command(vec!["definitely-not-a-real-program".into()]);
    let mut client = Client::start(t.path(), vec![Box::new(missing)]);
    let first = client.open("A.cs", "csharp", "class A { }");
    let second = client.open("B.cs", "csharp", "class B { }");
    assert!(client.diagnostics(&first).is_empty());
    assert!(client.diagnostics(&second).is_empty());
    // The harness chooses the first action of a prompt; the server then opens the page.
    // Any later round trip collects the server's follow-up request.
    assert!(client.format(&second).is_empty());

    let params = |method: &str| -> Vec<serde_json::Value> {
        client
            .server_request_params
            .iter()
            .filter(|(m, _)| m == method)
            .map(|(_, p)| p.clone())
            .collect()
    };
    let prompts = params("window/showMessageRequest");
    assert_eq!(prompts.len(), 1, "{prompts:?}");
    assert_eq!(prompts[0]["type"], 2);
    assert_eq!(
        prompts[0]["message"],
        "Sudsy needs the .NET 10 runtime to format and lint C#. JSON and YAML keep working."
    );
    assert_eq!(prompts[0]["actions"][0]["title"], "Download .NET");
    let documents = params("window/showDocument");
    assert_eq!(documents.len(), 1, "{documents:?}");
    assert_eq!(
        documents[0]["uri"],
        "https://dotnet.microsoft.com/download/dotnet/10.0"
    );
    assert_eq!(documents[0]["external"], true);
    assert!(client.notes.iter().all(|(m, _)| m != "window/showMessage"));
}

#[test]
fn an_edit_of_another_file_refreshes_the_semantic_findings_of_an_open_one() {
    let t = tempfile::tempdir().unwrap();
    std::fs::write(
        t.path().join("sudsy.yaml"),
        "lint:\n  rules:\n    remove-unused-usings: warn\n",
    )
    .unwrap();
    let mut client = Client::start(t.path(), providers(&lang_csharp::testing::built_worker()));
    let a = client.open(
        "A.cs",
        "csharp",
        "using N;\nusing M;\nclass A { T f; U g; }\n",
    );
    let src = "namespace N { class T {} } namespace M { class U {} }\n";
    let b = client.open("B.cs", "csharp", src);
    assert!(client.diagnostics(&a).is_empty());
    // `T` moves to `M`: the import of `N` is unused now, with no compile error in A.
    let moved = "namespace N { class X {} } namespace M { class T {} class U {} }\n";
    client.change(&b, 2, None, moved);
    // The client pulls the edited file, then the server asks it to pull again.
    client.diagnostics(&b);
    client.wait_for_request("workspace/diagnostic/refresh");
    let found = client.diagnostics(&a);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].range.start.line, 0);
    client.shutdown();
}

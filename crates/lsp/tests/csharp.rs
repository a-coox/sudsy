//! C# through the server: one long-lived worker serves a whole editing session.

mod common;

use common::{Client, apply};
use lsp_types::{Position, Range};
use provider_api::LanguageProvider;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The C# worker, built once for the test process.
fn worker_dll() -> PathBuf {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/csharp/Sudsy.Worker");
    let status = Command::new("dotnet")
        .args(["build", "-c", "Debug", "--nologo", "-v", "q"])
        .arg(&project)
        .status()
        .expect("dotnet is installed");
    assert!(status.success(), "dotnet build failed");
    project.join("bin/Debug/net10.0/Sudsy.Worker.dll")
}

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
    let mut client = Client::start(t.path(), providers(&worker_dll()));
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

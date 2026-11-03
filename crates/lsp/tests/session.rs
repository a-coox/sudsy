//! The server driven by an in-process client: JSON and YAML (no worker involved).

mod common;

use common::{Client, apply, apply_action};
use lsp_types::{CodeActionKind, DiagnosticSeverity, NumberOrString, Position, Range};
use provider_api::LanguageProvider;

fn providers() -> Vec<Box<dyn LanguageProvider>> {
    vec![
        Box::new(lang_json::JsonProvider::new()),
        Box::new(lang_yaml::YamlProvider::new()),
    ]
}

fn write(dir: &std::path::Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

fn code(d: &lsp_types::Diagnostic) -> &str {
    match d.code.as_ref().unwrap() {
        NumberOrString::String(s) => s,
        NumberOrString::Number(_) => panic!("rule ids are strings"),
    }
}

fn whole(line: u32) -> Range {
    Range::new(Position::new(line, 0), Position::new(line, 0))
}

#[test]
fn diagnostics_follow_the_text_of_the_document() {
    let t = tempfile::tempdir().unwrap();
    let mut client = Client::start(t.path(), providers());
    let uri = client.open("a.json", "json", "{\"a\": 1, \"a\": 2}\n");
    let found = client.diagnostics(&uri);
    assert_eq!(found.len(), 1);
    assert_eq!(code(&found[0]), "no-duplicate-keys");
    assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
    assert_eq!(found[0].source.as_deref(), Some("sudsy"));
    assert_eq!(found[0].related_information.as_ref().unwrap().len(), 1);

    // Rename the second key with an incremental edit; the problem goes away.
    client.change(
        &uri,
        2,
        Some(Range::new(Position::new(0, 10), Position::new(0, 11))),
        "b",
    );
    assert!(client.diagnostics(&uri).is_empty());

    // A syntax error replaces everything else.
    client.change(&uri, 3, None, "{\"a\": ");
    let found = client.diagnostics(&uri);
    assert!(!found.is_empty() && found.iter().all(|d| code(d) == "syntax-error"));
    client.close(&uri);
    assert!(client.diagnostics(&uri).is_empty());
    client.shutdown();
}

#[test]
fn ranges_count_utf16_units() {
    let t = tempfile::tempdir().unwrap();
    let mut client = Client::start(t.path(), providers());
    // The emoji is two UTF-16 units, so the second `"a"` starts at column 18.
    let uri = client.open("a.json", "json", "{\"😀\": 1, \"a\": 1, \"a\": 2}\n");
    let found = client.diagnostics(&uri);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].range.start, Position::new(0, 18));
    client.shutdown();
}

#[test]
fn quick_fixes_come_with_the_diagnostic_and_apply() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    ambiguous-booleans: warn\n    document-start: warn\n",
    );
    let mut client = Client::start(t.path(), providers());
    let text = "a: yes\nb: no\n";
    let uri = client.open("c.yaml", "yaml", text);
    let line = Range::new(Position::new(0, 0), Position::new(0, 6));
    let actions = client.code_actions(&uri, line, Some(vec![CodeActionKind::QUICKFIX]));
    let titles: Vec<&str> = actions.iter().map(|a| a.title.as_str()).collect();
    // The `---` marker at the top, the quoting of the first line's `yes`, and the rule's
    // fix-all (both booleans). The second line is outside the range.
    assert!(
        titles
            .iter()
            .any(|t| t.contains("---") || t.to_lowercase().contains("document")),
        "{titles:?}"
    );
    let fix_all = actions
        .iter()
        .find(|a| a.title == "Fix all `ambiguous-booleans` problems in this file")
        .unwrap_or_else(|| panic!("{titles:?}"));
    let fixed = apply_action(text, &uri, fix_all.edit.as_ref().unwrap());
    assert_eq!(fixed, "a: \"yes\"\nb: \"no\"\n");
    // An unsafe fix is labelled and never preferred.
    let single = actions
        .iter()
        .find(|a| a.title.ends_with("(unsafe)"))
        .unwrap_or_else(|| panic!("{titles:?}"));
    assert_eq!(single.is_preferred, Some(false));
    assert_eq!(single.diagnostics.as_ref().unwrap().len(), 1);
    client.shutdown();
}

#[test]
fn fix_all_applies_only_safe_fixes_and_respects_the_requested_kind() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    ambiguous-booleans: warn\n    document-start: warn\n",
    );
    let mut client = Client::start(t.path(), providers());
    let text = "a: yes\n";
    let uri = client.open("c.yaml", "yaml", text);
    let only = Some(vec![CodeActionKind::new("source.fixAll")]);
    let actions = client.code_actions(&uri, whole(0), only);
    assert_eq!(actions.len(), 1, "{actions:?}");
    assert_eq!(
        actions[0].kind,
        Some(CodeActionKind::new(lsp::FIX_ALL_KIND))
    );
    // The safe `---` only; quoting `yes` is unsafe.
    assert_eq!(
        apply_action(text, &uri, actions[0].edit.as_ref().unwrap()),
        "---\na: yes\n"
    );
    // Asking for something else gets none of it.
    let other = Some(vec![CodeActionKind::new("refactor")]);
    assert!(client.code_actions(&uri, whole(0), other).is_empty());
    client.shutdown();
}

#[test]
fn formatting_returns_a_minimal_edit_and_nothing_when_formatted() {
    let t = tempfile::tempdir().unwrap();
    let mut client = Client::start(t.path(), providers());
    let text = "{\"a\":1,\n\"b\":[1,2]}\n";
    let uri = client.open("a.json", "json", text);
    let edits = client.format(&uri);
    let formatted = apply(text, &edits);
    assert_ne!(formatted, text);
    assert!(formatted.contains("\"a\": 1"), "{formatted}");
    client.change(&uri, 2, None, &formatted);
    assert!(client.format(&uri).is_empty());

    // A file that does not parse is left alone.
    client.change(&uri, 3, None, "{\"a\": ");
    assert!(client.format(&uri).is_empty());
    client.shutdown();
}

#[test]
fn a_config_change_reloads_rules_and_refreshes_the_client() {
    let t = tempfile::tempdir().unwrap();
    let mut client = Client::start(t.path(), providers());
    let uri = client.open("c.yaml", "yaml", "a: yes\n");
    assert!(client.diagnostics(&uri).is_empty());

    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    ambiguous-booleans: error\n",
    );
    client.file_changed("sudsy.yaml");
    let found = client.diagnostics(&uri);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].severity, Some(DiagnosticSeverity::ERROR));
    assert!(
        client
            .server_requests
            .contains(&"client/registerCapability".to_string())
    );
    assert!(
        client
            .server_requests
            .contains(&"workspace/diagnostic/refresh".to_string())
    );

    // A file that is not a config is ignored.
    write(t.path(), "other.yaml", "x: 1\n");
    client.file_changed("other.yaml");
    client.shutdown();
}

#[test]
fn an_invalid_config_is_reported_once_and_lifts_when_fixed() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    no-such-rule: warn\n",
    );
    let mut client = Client::start(t.path(), providers());
    let message = client.wait_for("window/showMessage");
    assert!(
        message["message"]
            .as_str()
            .unwrap()
            .contains("no-such-rule"),
        "{message}"
    );

    let uri = client.open("a.json", "json", "{\"a\": 1, \"a\": 2}\n");
    // No diagnostics while the config is broken, and the reason is not repeated.
    assert!(client.diagnostics(&uri).is_empty());
    assert!(client.diagnostics(&uri).is_empty());
    assert!(
        client.notes.iter().all(|(m, _)| m != "window/showMessage"),
        "{:?}",
        client.notes
    );

    write(
        t.path(),
        "sudsy.yaml",
        "lint:\n  rules:\n    sorted-keys: warn\n",
    );
    client.file_changed("sudsy.yaml");
    assert_eq!(client.diagnostics(&uri).len(), 1);
    client.shutdown();
}

#[test]
fn editorconfig_changes_reach_open_documents() {
    let t = tempfile::tempdir().unwrap();
    write(
        t.path(),
        ".editorconfig",
        "root = true\n[*.json]\nindent_size = 2\n",
    );
    let mut client = Client::start(t.path(), providers());
    let text = "{\"a\":{\"b\":[1,\n2]}}\n";
    let uri = client.open("a.json", "json", text);
    let two = apply(text, &client.format(&uri));
    assert!(two.contains("\n  \"a\""), "{two}");

    write(
        t.path(),
        ".editorconfig",
        "root = true\n[*.json]\nindent_size = 4\n",
    );
    client.file_changed(".editorconfig");
    let four = apply(text, &client.format(&uri));
    assert!(four.contains("\n    \"a\""), "{four}");
    client.shutdown();
}

fn line(n: u32) -> Range {
    Range::new(Position::new(n, 0), Position::new(n, 99))
}

#[test]
fn suppress_quick_fix_inserts_a_comment_and_silences_the_diagnostic() {
    let t = tempfile::tempdir().unwrap();
    let mut client = Client::start(t.path(), providers());
    let title = "Suppress `no-duplicate-keys` on this line";
    let suppress = |client: &mut Client, uri, text: &str| {
        let actions = client.code_actions(uri, line(1), Some(vec![CodeActionKind::QUICKFIX]));
        let found = actions.iter().find(|a| a.title == title);
        found.map(|a| apply_action(text, uri, a.edit.as_ref().unwrap()))
    };

    // JSONC and YAML, each in its own comment syntax and at the line's indentation.
    let text = "{\n  \"a\": 1, \"a\": 2\n}\n";
    let uri = client.open("a.jsonc", "jsonc", text);
    let fixed = suppress(&mut client, &uri, text).expect("jsonc has comments");
    assert_eq!(
        fixed,
        "{\n  // sudsy-disable-next-line no-duplicate-keys\n  \"a\": 1, \"a\": 2\n}\n"
    );
    client.change(&uri, 2, None, &fixed);
    assert!(client.diagnostics(&uri).is_empty());

    let text = "a:\n  b: 1\n  b: 2\n";
    let uri = client.open("d.yaml", "yaml", text);
    let found = client.diagnostics(&uri);
    assert_eq!(found.len(), 1, "{found:?}");
    let rule = code(&found[0]).to_string();
    let actions = client.code_actions(&uri, line(2), Some(vec![CodeActionKind::QUICKFIX]));
    let action = actions
        .iter()
        .find(|a| a.title == format!("Suppress `{rule}` on this line"))
        .expect("yaml has comments");
    let fixed = apply_action(text, &uri, action.edit.as_ref().unwrap());
    assert_eq!(
        fixed,
        format!("a:\n  b: 1\n  # sudsy-disable-next-line {rule}\n  b: 2\n")
    );
    client.change(&uri, 2, None, &fixed);
    assert!(client.diagnostics(&uri).is_empty());

    // Plain JSON has no comments, so no action.
    let text = "{\n  \"a\": 1, \"a\": 2\n}\n";
    let uri = client.open("p.json", "json", text);
    assert!(suppress(&mut client, &uri, text).is_none());
    client.shutdown();
}

#[test]
fn formatting_honors_layout_rules_and_follows_the_config() {
    let t = tempfile::tempdir().unwrap();
    std::fs::create_dir(t.path().join("rules")).unwrap();
    write(
        t.path(),
        "rules/arrays.yaml",
        "id: arrays-one-per-line\nlanguage: json\nlayout: [break-all]\nrule: { kind: Array }\n",
    );
    write(
        t.path(),
        "sudsy.yaml",
        "plugins:\n  patterns: [\"rules/*.yaml\"]\n",
    );
    let mut client = Client::start(t.path(), providers());
    let text = "{ \"a\": [1, 2] }\n";
    let uri = client.open("a.json", "json", text);
    assert_eq!(
        apply(text, &client.format(&uri)),
        "{\n  \"a\": [\n    1,\n    2\n  ]\n}\n"
    );

    write(
        t.path(),
        "sudsy.yaml",
        "plugins:\n  patterns: [\"rules/*.yaml\"]\nlint:\n  rules:\n    arrays-one-per-line: \"off\"\n",
    );
    client.file_changed("sudsy.yaml");
    assert!(client.format(&uri).is_empty());
    client.shutdown();
}

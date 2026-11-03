//! The sample rules, tested natively with the SDK's harness (the same one the built-in rules use).

use plugin_sdk::testing::{expect, extensions, fix_with, lint_with, provider};
use sample_plugin::{BreakArrays, DuplicateName, NoDebugFlag};

#[test]
fn no_debug_flag_reports_and_fixes() {
    let p = provider("json");
    let ext = extensions("json", &NoDebugFlag);
    let text = expect(
        &*p,
        &ext,
        "a.json",
        "{\n  \"debug\": true,\n//^^^^^^^^^^^^^ no-debug-flag\n  \"other\": true\n}\n",
        "",
    );
    assert_eq!(
        fix_with(&*p, &ext, "a.json", &text, "", false),
        "{\n  \"debug\": false,\n  \"other\": true\n}\n"
    );
    let found = lint_with(
        &*p,
        &ext,
        "a.json",
        &text,
        "no-debug-flag:\n  level: error\n  options: { key: other }",
    );
    assert_eq!(found.len(), 1, "{found:?} {text:?}");
    assert!(found[0].message.contains("other"));
}

#[test]
fn project_scope_rule_collects_and_finishes() {
    let ext = extensions("json", &DuplicateName);
    let p = provider("json");
    let report = lint_with(&*p, &ext, "a.json", "{ \"name\": \"x\" }", "");
    assert!(
        report.is_empty(),
        "project scope reports only in the project phase"
    );
}

#[test]
fn break_arrays_attaches_directives_natively() {
    use plugin_sdk::Rule;
    // The rule is layout-only: lint runs nothing of it, and the harness sees no diagnostic.
    assert_eq!(BreakArrays.layout_role(), plugin_sdk::LayoutRole::Only);
    let p = provider("json");
    let ext = extensions("json", &BreakArrays);
    assert!(lint_with(&*p, &ext, "a.json", "[1, 2, 3]", "").is_empty());
}

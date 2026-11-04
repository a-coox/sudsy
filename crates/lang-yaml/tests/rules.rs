//! YAML rules: expected diagnostics (snapshots) and fix output.

use insta::assert_snapshot;
use lang_yaml::YamlProvider;
use provider_api::testing::{lint, render};

fn report(text: &str, rules: &str) -> String {
    render(text, &lint(&YamlProvider::new(), "a.yaml", text, rules))
}

#[test]
fn suppression_comment() {
    let text = "a: 1\n# sudsy-disable-next-line no-duplicate-keys\na: 2\na: 3\n";
    assert_snapshot!(report(text, ""));
}

fn dup_count(text: &str) -> usize {
    report(text, "").matches("no-duplicate-keys").count()
}

#[test]
fn block_line_and_file_suppressions() {
    assert_eq!(dup_count("a: 1\na: 2\n"), 1);
    // Line, with and without a rule list; another rule's name does not apply.
    assert_eq!(
        dup_count("a: 1\na: 2 # sudsy-disable-line no-duplicate-keys\n"),
        0
    );
    assert_eq!(dup_count("a: 1\na: 2 # sudsy-disable-line\n"), 0);
    assert_eq!(
        dup_count("a: 1\na: 2 # sudsy-disable-line max-line-length\n"),
        1
    );
    // Block: only between disable and enable; enable may name one rule.
    let block = |open: &str, close: &str| format!("{open}\na: 1\na: 2\n{close}\nb: 1\nb: 2\n");
    assert_eq!(dup_count(&block("# sudsy-disable", "# sudsy-enable")), 1);
    assert_eq!(
        dup_count(&block(
            "# sudsy-disable no-duplicate-keys",
            "# sudsy-enable"
        )),
        1
    );
    assert_eq!(
        dup_count(&block(
            "# sudsy-disable",
            "# sudsy-enable no-duplicate-keys"
        )),
        1
    );
    assert_eq!(
        dup_count(&block("# sudsy-disable", "# sudsy-enable max-line-length")),
        0
    );
    assert_eq!(dup_count(&block("# sudsy-disable", "# nothing")), 0);
    // File: anywhere in the file.
    assert_eq!(
        dup_count("a: 1\na: 2\n# sudsy-disable-file no-duplicate-keys\n"),
        0
    );
    assert_eq!(dup_count("# sudsy-disable-file\na: 1\na: 2\n"), 0);
}

#[test]
fn ambiguous_booleans_flag_every_casing_and_honor_check_keys_and_allowed() {
    let text = "on: True\nlist: [FALSE, false, true]\n? Yes\n: y\n";
    let all = report(text, "ambiguous-booleans: warn");
    assert_eq!(all.matches("ambiguous-booleans").count(), 4, "{all}");
    let rules = "ambiguous-booleans:\n  level: warn\n  options:\n    check_keys: false";
    let values = report(text, rules);
    assert_eq!(values.matches("ambiguous-booleans").count(), 2, "{values}");
    let rules = "ambiguous-booleans:\n  level: warn\n  options:\n    allowed: [\"True\", \"on\"]";
    let allowed = report(text, rules);
    assert_eq!(
        allowed.matches("ambiguous-booleans").count(),
        2,
        "{allowed}"
    );
}

#[test]
fn max_line_length_allows_non_breakable_words() {
    let u = "u".repeat(30);
    let text = format!("# {u}\n- {u}\n- k: {u}\n");
    let rules = |allow: bool| {
        format!(
            "max-line-length:\n  level: warn\n  options:\n    max: 20\n    allow_non_breakable_words: {allow}"
        )
    };
    let kept = report(&text, &rules(true));
    assert_eq!(kept.matches("max-line-length").count(), 1, "{kept}");
    assert_eq!(
        report(&text, &rules(false))
            .matches("max-line-length")
            .count(),
        3
    );
}

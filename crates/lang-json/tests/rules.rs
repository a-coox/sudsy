//! JSON rules: expected diagnostics (snapshots) and fix output.

use insta::assert_snapshot;
use lang_json::JsonProvider;
use provider_api::testing::{fix, lint, render};

const ALL: &str = "no-duplicate-keys: error\nsorted-keys: warn";

fn report(text: &str, rules: &str) -> String {
    report_at("a.json", text, rules)
}

fn report_at(path: &str, text: &str, rules: &str) -> String {
    render(text, &lint(&JsonProvider::new(), path, text, rules))
}

#[test]
fn duplicate_keys_are_reported_at_the_later_key() {
    let text = "{\"a\": 1, \"b\": {\"x\": 1, \"x\": 2}, \"a\": 3, \"a\": 4}\n";
    assert_snapshot!(report(text, "no-duplicate-keys: error"));
}

#[test]
fn duplicate_keys_are_on_by_default_and_can_be_turned_off() {
    let text = "{\"a\": 1, \"a\": 2}";
    assert!(report(text, "").contains("error[no-duplicate-keys]"));
    assert_eq!(report(text, "no-duplicate-keys: \"off\""), "");
    assert!(report(text, "no-duplicate-keys: warn").contains("warning[no-duplicate-keys]"));
}

#[test]
fn sorted_keys_is_opt_in() {
    assert_eq!(report("{\"b\": 1, \"a\": 2}", ""), "");
    assert_snapshot!(report(
        "{\"b\": 1, \"a\": 2, \"c\": {\"z\": 1, \"y\": 2}}",
        ALL
    ));
}

#[test]
fn sorted_keys_fix_sorts_every_level_and_is_idempotent() {
    let text = "{\n  \"b\": 1,\n  \"a\": {\"y\": 2, \"x\": [3]},\n  \"c\": null\n}\n";
    // Unsafe: nothing happens without `--unsafe`.
    assert_eq!(fix(&JsonProvider::new(), "a.json", text, ALL, false), text);
    assert_snapshot!(fix(&JsonProvider::new(), "a.json", text, ALL, true));
}

#[test]
fn sorted_keys_order_option_puts_listed_keys_first() {
    let rules = "sorted-keys:\n  level: warn\n  options:\n    order: [name, version]";
    let text = "{\"zeta\": 1, \"version\": 2, \"alpha\": 3, \"name\": 4}";
    assert_snapshot!(fix(&JsonProvider::new(), "a.json", text, rules, true));
}

#[test]
fn sorted_keys_keeps_comments_in_place_in_jsonc() {
    let text = "{\n  // about b\n  \"b\": 1, // one\n  \"a\": 2 // two\n}\n";
    assert_snapshot!(fix(&JsonProvider::new(), "a.jsonc", text, ALL, true));
}

#[test]
fn suppression_comments_work_in_jsonc() {
    let reported = |text: &str| -> Vec<String> {
        report_at("a.jsonc", text, "no-duplicate-keys: error")
            .lines()
            .filter(|l| !l.starts_with(' '))
            .map(|l| l.split(' ').next().unwrap().to_string())
            .collect()
    };
    // The directive covers the next line only.
    let text = "{\n  \"a\": 1,\n  // sudsy-disable-next-line no-duplicate-keys\n  \"a\": 2,\n  \"a\": 3\n}\n";
    assert_eq!(reported(text), ["5:3"]);
    // Another rule's name does not suppress this one.
    let text = "{\n  \"a\": 1,\n  // sudsy-disable-next-line sorted-keys\n  \"a\": 2\n}\n";
    assert_eq!(reported(text), ["4:3"]);
    // Without a rule name, every rule is suppressed on that line.
    let text = "{\n  \"a\": 1,\n  // sudsy-disable-next-line\n  \"a\": 2\n}\n";
    assert!(reported(text).is_empty());
}

#[test]
fn shared_rule_max_line_length() {
    let text = "{\"key\": \"0123456789\"}\n";
    assert_snapshot!(report(
        text,
        "max-line-length:\n  level: warn\n  options:\n    max: 10"
    ));
    assert_eq!(report(text, ""), "");
}

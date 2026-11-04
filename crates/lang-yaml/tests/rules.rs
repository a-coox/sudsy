//! YAML rules: expected diagnostics (snapshots) and fix output.

use insta::assert_snapshot;
use lang_yaml::YamlProvider;
use provider_api::testing::{fix, lint, render};

fn report(text: &str, rules: &str) -> String {
    render(text, &lint(&YamlProvider::new(), "a.yaml", text, rules))
}

fn fixed(text: &str, rules: &str, allow_unsafe: bool) -> String {
    fix(&YamlProvider::new(), "a.yaml", text, rules, allow_unsafe)
}

#[test]
fn sorted_keys_order_option() {
    let rules = "sorted-keys:\n  level: warn\n  options:\n    order: [name]";
    assert_snapshot!(fixed("b: 1\nname: x\na: 2\n", rules, true));
}

#[test]
fn suppression_comment() {
    let text = "a: 1\n# sudsy-disable-next-line no-duplicate-keys\na: 2\na: 3\n";
    assert_snapshot!(report(text, ""));
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

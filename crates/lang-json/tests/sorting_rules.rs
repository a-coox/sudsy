//! JSON rules added to `sorted-keys` and the opt-in key and value rules.

use lang_json::JsonProvider;
use provider_api::testing::{fix, lint, render};

fn report(text: &str, rules: &str) -> String {
    render(text, &lint(&JsonProvider::new(), "a.json", text, rules))
}

fn opts(rule: &str, options: &str) -> String {
    format!("{rule}:\n  level: warn\n  options:\n    {options}")
}

fn fixed(rule: &str, options: &str, text: &str) -> String {
    fix(
        &JsonProvider::new(),
        "a.json",
        text,
        &opts(rule, options),
        true,
    )
}

fn count(out: &str, rule: &str) -> usize {
    out.matches(&format!("[{rule}]")).count()
}

#[test]
fn duplicate_keys_compare_decoded_keys() {
    let out = report("{\"a\": 1, \"\\u0061\": 2, \"b\\/\": 3, \"b/\": 4}", "");
    assert_eq!(count(&out, "no-duplicate-keys"), 2, "{out}");
}

#[test]
fn sorted_keys_directions_modes_and_thresholds() {
    let keys = |o: &str, t: &str| fixed("sorted-keys", o, t);
    assert_eq!(
        keys("direction: desc", "{\"a\": 1, \"c\": 2, \"b\": 3}"),
        "{\"c\": 2, \"b\": 3, \"a\": 1}"
    );
    assert_eq!(
        keys(
            "compare: case_insensitive",
            "{\"b\": 1, \"A\": 2, \"a\": 3}"
        ),
        "{\"A\": 2, \"a\": 3, \"b\": 1}"
    );
    assert_eq!(
        keys("compare: natural", "{\"a10\": 1, \"a2\": 2}"),
        "{\"a2\": 2, \"a10\": 1}"
    );
    assert_eq!(
        keys("min_keys: 4", "{\"b\": 1, \"a\": 2, \"c\": 3}"),
        "{\"b\": 1, \"a\": 2, \"c\": 3}"
    );
    assert_eq!(
        keys("ignored_keys: [\"^_\"]", "{\"b\": 1, \"_x\": 2, \"a\": 3}"),
        "{\"a\": 3, \"_x\": 2, \"b\": 1}"
    );
    assert_eq!(
        keys(
            "blank_line_groups: true",
            "{\n  \"b\": 1,\n  \"a\": 2,\n\n  \"d\": 3,\n  \"c\": 4\n}"
        ),
        "{\n  \"a\": 2,\n  \"b\": 1,\n\n  \"c\": 4,\n  \"d\": 3\n}"
    );
}

#[test]
fn sorted_keys_reports_the_keys_that_have_to_move() {
    let out = report("{\"c\": 1, \"b\": 2, \"a\": 3}", "sorted-keys: warn");
    assert_eq!(count(&out, "sorted-keys"), 2, "{out}");
}

#[test]
fn empty_keys() {
    let out = report(
        "{\"\": 1, \" \\t\": 2, \"a\": 3, \"\\u0020\": 4}",
        "no-empty-keys: warn",
    );
    assert_eq!(count(&out, "no-empty-keys"), 3, "{out}");
}

#[test]
fn key_name_casing() {
    let text = "{\"camelCase\": 1, \"snake_case\": 2, \"kebab-case\": 3, \"PascalCase\": 4, \"SCREAMING_SNAKE\": 5, \"a b\": 6}";
    let out = report(text, "key-name-casing: warn");
    assert_eq!(count(&out, "key-name-casing"), 5, "{out}");
    let rules = opts(
        "key-name-casing",
        "allowed: [snake_case, kebab-case]\n    ignored_keys: [\"^P\"]",
    );
    let out = report(text, &rules);
    assert_eq!(count(&out, "key-name-casing"), 3, "{out}");
}

#[test]
fn unsafe_numbers() {
    let text = "[9007199254740991, 9007199254740992, -12345678901234567890, 1e999, 1e-999, 5e-324, 0, 0.0e5, 1.5, 12345678901234567890.5]";
    let out = report(text, "unsafe-numbers: warn");
    assert_eq!(count(&out, "unsafe-numbers"), 5, "{out}");
}

#[test]
fn sorted_array_values() {
    let values = |o: &str, t: &str| fixed("sorted-array-values", o, t);
    assert_eq!(
        values("direction: asc", "[\"b\", \"a\", \"c\"]"),
        "[\"a\", \"b\", \"c\"]"
    );
    assert_eq!(values("direction: asc", "[10, 9, 100]"), "[9, 10, 100]");
    let mixed = "[\"a\", 1, \"b\", \"A\"]";
    assert_eq!(values("direction: asc", mixed), mixed);
    assert_eq!(values("min_values: 3", "[2, 1]"), "[2, 1]");
    assert_eq!(report("[\"b\", \"a\"]", ""), "");
}

#[test]
#[should_panic(expected = "option `direction` must be one of `asc`, `desc`, not `up`")]
fn invalid_direction_is_a_config_error() {
    fixed("sorted-keys", "direction: up", "{}");
}

#[test]
#[should_panic(expected = "rule `key-name-casing`")]
fn invalid_casing_is_a_config_error() {
    fixed("key-name-casing", "allowed: [Camel]", "{}");
}

#[test]
#[should_panic(expected = "option `ignored_keys` has an invalid regular expression `(`")]
fn invalid_ignored_keys_regex_is_a_config_error() {
    fixed("sorted-keys", "ignored_keys: [\"(\"]", "{}");
}

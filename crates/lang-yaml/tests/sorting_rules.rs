//! YAML `sorted-keys` options and `no-duplicate-keys` merge key handling.

use lang_yaml::YamlProvider;
use provider_api::testing::{fix, lint, render};

fn opts(rule: &str, options: &str) -> String {
    format!("{rule}:\n  level: warn\n  options:\n    {options}")
}

fn report(text: &str, rule: &str, options: &str) -> String {
    let rules = opts(rule, options);
    render(text, &lint(&YamlProvider::new(), "a.yaml", text, &rules))
}

fn sorted(options: &str, text: &str) -> String {
    fix(
        &YamlProvider::new(),
        "a.yaml",
        text,
        &opts("sorted-keys", options),
        true,
    )
}

#[test]
fn sorted_keys_directions_modes_and_thresholds() {
    assert_eq!(
        sorted("direction: desc", "a: 1\nc: 2\nb: 3\n"),
        "c: 2\nb: 3\na: 1\n"
    );
    assert_eq!(
        sorted("compare: case_insensitive", "b: 1\nA: 2\na: 3\n"),
        "A: 2\na: 3\nb: 1\n"
    );
    assert_eq!(
        sorted("compare: natural", "a10: 1\na2: 2\n"),
        "a2: 2\na10: 1\n"
    );
    assert_eq!(sorted("min_keys: 3", "b: 1\na: 2\n"), "b: 1\na: 2\n");
    assert_eq!(
        sorted("ignored_keys: [\"^_\"]", "b: 1\n_x: 2\na: 3\n"),
        "a: 3\n_x: 2\nb: 1\n"
    );
    assert_eq!(
        sorted("blank_line_groups: true", "b: 1\na: 2\n\nd: 3\nc: 4\n"),
        "a: 2\nb: 1\n\nc: 4\nd: 3\n"
    );
}

#[test]
fn ignored_keys_are_not_reported_between_other_keys() {
    let out = report(
        "a: 1\n_x: 2\nb: 3\n",
        "sorted-keys",
        "ignored_keys: [\"^_\"]",
    );
    assert_eq!(out, "");
}

#[test]
fn merge_keys_can_be_forbidden() {
    let text = "a:\n  <<: *x\n  <<: *y\n";
    let allowed = report(text, "no-duplicate-keys", "merge_keys: allow");
    assert!(!allowed.contains("duplicate key"), "{allowed}");
    let forbidden = report(text, "no-duplicate-keys", "merge_keys: forbid");
    assert!(forbidden.contains("duplicate key `<<`"), "{forbidden}");
}

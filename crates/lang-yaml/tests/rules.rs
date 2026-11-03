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
fn duplicate_keys_in_block_and_flow_mappings() {
    let text = "a: 1\nb:\n  x: 1\n  x: 2\n'a': 3\n? a\n: 4\nflow: {k: 1, k: 2}\nmerge:\n  <<: *one\n  <<: *two\n";
    assert_snapshot!(report(text, ""));
}

#[test]
fn quoted_and_plain_keys_are_the_same_key() {
    assert!(report("\"a\": 1\na: 2\n", "").contains("duplicate key `a`"));
    assert_eq!(report("a: 1\n---\na: 2\n", ""), "");
}

#[test]
fn ambiguous_booleans_are_opt_in_and_quoted_by_the_unsafe_fix() {
    let text =
        "on: yes\nlist: [no, Off, true]\nsafe: !!str yes\nquoted: \"yes\"\nword: yesterday\n";
    assert_eq!(report(text, ""), "");
    assert_snapshot!(report(text, "ambiguous-booleans: warn"));
    assert_eq!(fixed(text, "ambiguous-booleans: warn", false), text);
    assert_snapshot!(fixed(text, "ambiguous-booleans: warn", true));
}

#[test]
fn document_start_adds_the_marker_with_a_safe_fix() {
    let rules = "document-start: warn";
    assert_snapshot!(fixed("# comment\na: 1\n", rules, false));
    for already in [
        "---\na: 1\n",
        "%YAML 1.2\n---\na: 1\n",
        "# c\n---\na: 1\n",
        "",
    ] {
        assert_eq!(report(already, rules), "", "{already:?}");
    }
}

#[test]
fn sorted_keys_sorts_block_mappings_and_keeps_comments_in_place() {
    let text = "# head\nb: 1 # one\nz:\n  - 1\n  - 2\na:\n  y: |\n    text\n  x: 2\n";
    assert_snapshot!(report(text, "sorted-keys: warn"));
    assert_eq!(fixed(text, "sorted-keys: warn", false), text);
    assert_snapshot!(fixed(text, "sorted-keys: warn", true));
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

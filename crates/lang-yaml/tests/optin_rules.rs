//! The opt-in YAML rules of yamllint's anchors, empty-values, float-values, octal-values and
//! quoted-strings (the differential test in tests/parity/yamllint covers the positions).

use lang_yaml::YamlProvider;
use provider_api::testing::{fix, lint};

fn count(text: &str, rules: &str, rule: &str) -> usize {
    lint(&YamlProvider::new(), "a.yaml", text, rules)
        .iter()
        .filter(|d| d.rule == rule)
        .count()
}

fn fixed(text: &str, rules: &str) -> String {
    fix(&YamlProvider::new(), "a.yaml", text, rules, true)
}

#[test]
fn anchors_duplicate_and_unused() {
    let text = "- &a 1\n- &a 2\n- *a\n- &b 3\n---\n- &a 4\n";
    assert_eq!(
        count(text, "no-duplicate-anchors: warn", "no-duplicate-anchors"),
        1
    );
    assert_eq!(
        count(text, "no-unused-anchors: warn", "no-unused-anchors"),
        3
    );
    assert_eq!(
        fixed("a: &x 1\nb: &y\n  c: 1\nd: *y\n", "no-unused-anchors: warn"),
        "a: 1\nb: &y\n  c: 1\nd: *y\n"
    );
    assert_eq!(fixed("a: &x\n", "no-unused-anchors: warn"), "a:\n");
}

#[test]
fn empty_values_and_options() {
    let text = "a:\nb: {c: }\nd:\n  -\ne: !!null\nf: &x\n";
    assert_eq!(count(text, "no-empty-values: warn", "no-empty-values"), 3);
    let only_flow = "no-empty-values:\n  level: warn\n  options:\n    block_mappings: false\n    block_sequences: false";
    assert_eq!(count(text, only_flow, "no-empty-values"), 1);
}

#[test]
fn floats_and_octals() {
    let floats = "float-style:\n  level: warn\n  options:\n    require_numeral_before_decimal: true\n    forbid_scientific_notation: true\n    forbid_nan: true\n    forbid_inf: true";
    assert_eq!(
        count(
            "a: [.5, 1e3, .nan, -.inf, 0.5, '.5', !!float .5]",
            floats,
            "float-style"
        ),
        4
    );
    assert_eq!(count("a: .5\n", "float-style: warn", "float-style"), 0);
    assert_eq!(fixed("a: [.5, -.5e3]\n", floats), "a: [0.5, -0.5e3]\n");
    let text = "a: 0755\nb: 0o755\nc: '0755'\nd: 08\ne: !!int 0755\n";
    assert_eq!(count(text, "no-octal-values: warn", "no-octal-values"), 2);
}

#[test]
fn quoted_strings_modes_and_fixes() {
    let always =
        "quoted-strings:\n  level: warn\n  options:\n    required: always\n    quote_type: double";
    let text = "a: x\nb: 'y'\nc: \"z\"\nd: 12\ne: true\n";
    assert_eq!(count(text, always, "quoted-strings"), 2);
    assert_eq!(
        fixed(text, always),
        "a: \"x\"\nb: \"y\"\nc: \"z\"\nd: 12\ne: true\n"
    );
    let needed = "quoted-strings:\n  level: warn\n  options:\n    required: only_when_needed";
    let text = "a: 'x'\nb: 'yes'\nc: 'a: b'\nd: ''\ne: '1e3'\n";
    assert_eq!(count(text, needed, "quoted-strings"), 2);
    assert_eq!(
        fixed(text, needed),
        "a: x\nb: 'yes'\nc: 'a: b'\nd: ''\ne: 1e3\n"
    );
    let optional = "quoted-strings:\n  level: warn\n  options:\n    required: optional\n    quote_type: single\n    extra_required: [\"^http\"]";
    let text = "a: \"x\"\nb: http://x\nc: plain\n";
    assert_eq!(count(text, optional, "quoted-strings"), 2);
}

#[test]
#[should_panic(expected = "option `quote_type` must be one of")]
fn quoted_strings_rejects_unknown_quote_type() {
    count(
        "a: 1\n",
        "quoted-strings:\n  level: warn\n  options:\n    quote_type: backtick",
        "x",
    );
}

#[test]
#[should_panic(expected = "invalid regular expression")]
fn quoted_strings_rejects_a_bad_regex() {
    count(
        "a: 1\n",
        "quoted-strings:\n  level: warn\n  options:\n    extra_required: [\"(\"]",
        "x",
    );
}

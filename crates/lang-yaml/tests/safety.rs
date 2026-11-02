//! The data-model safety check, and the files the formatter refuses.

mod common;

use core_format::NodeFormatter;
use core_format::testing::Subject;
use lang_yaml::YamlFormatter;

fn check(before: &str, after: &str) -> Result<(), String> {
    let tree = |text: &str| lang_yaml::parse(text).tree;
    YamlFormatter.extra_check(&tree(before), &tree(after))
}

#[test]
fn quote_style_and_layout_changes_keep_the_model() {
    assert!(check("a: 'x'\n", "a: \"x\"\n").is_ok());
    assert!(check("a: [1,2]\n", "a:\n  [1, 2]\n").is_ok());
    assert!(check("a: |\n    x\n", "a: |\n  x\n").is_ok());
    assert!(check("a: b\n  c\n", "a: b c\n").is_ok());
    assert!(check("a: &x 1\nb: *x\n", "a: &x 1\nb: *x\n").is_ok());
}

#[test]
fn a_changed_value_type_or_structure_is_found() {
    // A string became an integer.
    assert!(
        check("a: '1'\n", "a: 1\n")
            .unwrap_err()
            .contains("document 1")
    );
    // Indentation changed the nesting.
    assert!(check("a:\n  b: 1\n", "a:\nb: 1\n").is_err());
    // A document disappeared.
    assert!(
        check("a\n---\nb\n", "a\n")
            .unwrap_err()
            .contains("2 documents became 1")
    );
    // `yes` is a string in YAML 1.2 and stays one; quoting `true` makes it a string.
    assert!(check("a: yes\n", "a: 'yes'\n").is_ok());
    assert!(check("a: true\n", "a: 'true'\n").is_err());
    // Chomping changes the value of a block scalar.
    assert!(check("a: |\n  x\n", "a: |-\n  x\n").is_err());
    // A tag is part of the model.
    assert!(check("a: !t x\n", "a: x\n").is_err());
}

#[test]
fn files_with_syntax_errors_are_refused_and_left_alone() {
    let s = common::subject(&[]);
    let err = s.format("a: b: c\n").unwrap_err();
    assert!(err.contains("mapping"), "{err}");
    assert!(s.format("key: [1, 2\n").is_err());
    assert!(s.format("\tkey: 1\n").is_err());
}

#[test]
fn significant_blanks_at_the_end_of_block_scalars_survive() {
    let s = common::subject(&[]);
    assert_eq!(s.format("a: |\n  x  \n").unwrap(), "a: |\n  x  \n");
    assert_eq!(s.format("a: |+\n  x\n\n\n").unwrap(), "a: |+\n  x\n\n\n");
}

#[test]
fn the_deepest_allowed_nesting_formats_without_overflowing_the_stack() {
    let s = common::subject(&[]);
    let flow = format!(
        "{}x{}\n",
        "[".repeat(lang_yaml::MAX_DEPTH),
        "]".repeat(lang_yaml::MAX_DEPTH)
    );
    s.format(&flow).unwrap();
    let block: String = (0..(lang_yaml::MAX_DEPTH / 2 - 1))
        .map(|i| format!("{}- a:\n", "  ".repeat(i)))
        .collect();
    let block = format!("{block}{}x: 1\n", "  ".repeat(lang_yaml::MAX_DEPTH / 2 - 1));
    s.format(&block).unwrap();
}

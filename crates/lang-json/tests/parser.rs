//! Parser behavior: error recovery, dialects, and properties that hold for any input.

mod common;

use common::{JsonSubject, subject};
use core_format::testing::{Subject, check};
use core_syntax::generated::json::kind as k;
use core_syntax::{SyntaxKind, SyntaxNode, print, trivia};
use lang_json::JsonProvider;
use proptest::prelude::*;
use provider_api::{LanguageProvider, SourceFile};

fn parse(path: &str, text: &str) -> (SyntaxNode, Vec<String>) {
    let r = JsonProvider::new()
        .parse(&SourceFile::new(path, 1, text))
        .unwrap();
    (r.tree, r.errors.into_iter().map(|e| e.code).collect())
}

fn assert_lossless(tree: &SyntaxNode, text: &str) {
    assert_eq!(print(tree), text);
    let g = core_syntax::grammars::json();
    trivia::check_attachment(tree, |kind| g.is_trivia(kind)).unwrap();
}

#[test]
fn valid_documents_have_no_errors_and_the_expected_shape() {
    let (tree, errors) = parse("a.json", "{\"a\": [1, true, null, \"s\"], \"b\": {}}");
    assert!(errors.is_empty());
    let kinds: Vec<SyntaxKind> = tree.descendants().map(|n| n.kind()).collect();
    assert_eq!(kinds.iter().filter(|k| **k == k::MEMBER).count(), 2);
    assert_eq!(kinds.iter().filter(|k| **k == k::OBJECT).count(), 2);
    assert_eq!(kinds.iter().filter(|k| **k == k::ARRAY).count(), 1);
    assert_eq!(kinds.iter().filter(|k| **k == k::LITERAL).count(), 4);
}

#[test]
fn dialects_differ_in_comments_and_trailing_commas() {
    let text = "{\"a\": [1,], // c\n}";
    let (_, json) = parse("a.json", text);
    assert_eq!(
        json,
        ["trailing-comma", "trailing-comma", "comment-in-json"]
    );
    let (tree, jsonc) = parse("a.jsonc", text);
    assert!(jsonc.is_empty());
    assert_lossless(&tree, text);
}

#[test]
fn errors_are_recovered_into_error_nodes_with_every_token_kept() {
    for text in [
        "{\"a\" 1}",
        "{\"a\": }",
        "{1: 2}",
        "[1 2]",
        "[1,, 2]",
        "[,]",
        "{,}",
        "[1, 2",
        "{\"a\": [1}",
        "]",
        "1 2",
        "",
        "   // only a comment\n",
        "{\"a\": 'b'}",
        "[nul]",
        "[1.]",
        "\"unterminated",
        r#"["a\qb"]"#,
    ] {
        let (tree, errors) = parse("a.jsonc", text);
        assert!(!errors.is_empty(), "{text:?} should have errors");
        assert_lossless(&tree, text);
    }
}

#[test]
fn stray_tokens_become_error_nodes() {
    let (tree, errors) = parse("a.json", "[1, ] 2");
    assert!(!errors.is_empty());
    assert!(tree.descendants().any(|n| n.kind() == SyntaxKind::ERROR));
}

#[test]
fn error_positions_point_at_the_problem() {
    let r = JsonProvider::new()
        .parse(&SourceFile::new("a.json", 1, "{\"a\": tru}"))
        .unwrap();
    let e = &r.errors[0];
    assert_eq!((e.start, e.len), (6, 3));
    assert_eq!(e.code, "unexpected-word");
}

#[test]
fn nesting_beyond_the_limit_is_an_error_not_a_crash() {
    let depth = lang_json::MAX_DEPTH + 10;
    let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
    let (tree, errors) = parse("a.json", &text);
    assert!(errors.iter().any(|c| c == "nesting-too-deep"));
    assert_lossless(&tree, &text);
}

#[test]
fn trailing_trivia_stays_inside_the_node_of_its_token() {
    let text = "[1, // one\n  2]";
    let (tree, errors) = parse("a.jsonc", text);
    assert!(errors.is_empty());
    let first = tree.descendants().find(|n| n.kind() == k::LITERAL).unwrap();
    assert_eq!(first.text().to_string(), "1");
    // The comma and its trailing comment are siblings in the array.
    let array = tree.descendants().find(|n| n.kind() == k::ARRAY).unwrap();
    assert!(
        array
            .children_with_tokens()
            .any(|e| e.kind() == k::LINE_COMMENT)
    );
}

fn soup() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop::sample::select(vec![
            "{", "}", "[", "]", ":", ",", " ", "\n", "\r\n", "\t", "\"", "\"a\"", "\"\n\"", "1",
            "-0.5e3", "true", "null", "//", "/*", "*/", "x", "'", "\\", "\u{e9}", "\u{2028}",
        ]),
        0..40,
    )
    .prop_map(|v| v.concat())
}

proptest! {
    #[test]
    fn any_input_parses_losslessly(text in soup()) {
        for path in ["a.json", "a.jsonc"] {
            let (tree, _) = parse(path, &text);
            prop_assert_eq!(print(&tree), text.clone());
            let g = core_syntax::grammars::json();
            prop_assert!(trivia::check_attachment(&tree, |kind| g.is_trivia(kind)).is_ok());
        }
    }

    #[test]
    fn anything_that_parses_formats_idempotently(text in soup()) {
        let s: JsonSubject = subject("a.jsonc", &[]);
        if s.parse(&text).is_ok() {
            prop_assert!(check(&s, &text).is_ok(), "{:?}", check(&s, &text));
        }
    }
}

fn value() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        Just("1".to_string()),
        Just("-2.5e3".to_string()),
        Just("true".to_string()),
        Just("null".to_string()),
        "[a-z ]{0,6}".prop_map(|s| format!("\"{s}\"")),
    ];
    let ws = prop::sample::select(vec!["", " ", "\n", "\n\n  ", " // c\n", " /* c */ "]);
    leaf.prop_recursive(4, 40, 4, move |inner| {
        let ws = ws.clone();
        prop_oneof![
            (prop::collection::vec((ws.clone(), inner.clone(), ws.clone()), 0..4)).prop_map(
                |items| {
                    let body: Vec<String> = items
                        .into_iter()
                        .map(|(a, v, b)| format!("{a}{v}{b}"))
                        .collect();
                    format!("[{}]", body.join(","))
                }
            ),
            (prop::collection::vec((ws.clone(), "[a-z]{1,3}", inner, ws), 0..4)).prop_map(
                |items| {
                    let body: Vec<String> = items
                        .into_iter()
                        .map(|(a, k, v, b)| format!("{a}\"{k}\":{v}{b}"))
                        .collect();
                    format!("{{{}}}", body.join(","))
                }
            ),
        ]
    })
}

proptest! {
    #[test]
    fn generated_documents_format_idempotently_and_keep_their_meaning(text in value()) {
        for options in [vec![], vec![("collapse", common::s("never"))], vec![("collapse", common::s("always")), ("line_width", core_config::OptionValue::Int(20))]] {
            let s = subject("a.jsonc", &options);
            prop_assert!(check(&s, &text).is_ok(), "{:?}", check(&s, &text));
        }
    }
}

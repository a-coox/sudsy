//! Parser behavior: tree shape, error recovery, and properties that hold for any input.

use core_syntax::generated::yaml::kind as k;
use core_syntax::{SyntaxKind, SyntaxNode, print, trivia};
use lang_yaml::parse;
use proptest::prelude::*;

fn kinds(tree: &SyntaxNode) -> Vec<SyntaxKind> {
    tree.descendants().map(|n| n.kind()).collect()
}

fn count(tree: &SyntaxNode, kind: SyntaxKind) -> usize {
    kinds(tree).iter().filter(|k| **k == kind).count()
}

fn assert_lossless(text: &str) -> lang_yaml::Parsed {
    let parsed = parse(text);
    assert_eq!(print(&parsed.tree), text);
    if parsed.errors.is_empty() {
        let g = core_syntax::grammars::yaml();
        trivia::check_attachment(&parsed.tree, |kind| g.is_trivia(kind)).unwrap();
    }
    parsed
}

#[test]
fn block_structure_has_one_node_per_collection_and_entry() {
    let p = assert_lossless("a: 1\nb:\n  - x\n  - y: z\n");
    assert!(p.errors.is_empty());
    assert_eq!(count(&p.tree, k::BLOCK_MAPPING), 2);
    assert_eq!(count(&p.tree, k::BLOCK_MAPPING_ENTRY), 3);
    assert_eq!(count(&p.tree, k::BLOCK_SEQUENCE), 1);
    assert_eq!(count(&p.tree, k::BLOCK_SEQUENCE_ENTRY), 2);
    assert_eq!(count(&p.tree, k::SCALAR), 6);
}

#[test]
fn flow_collections_and_pairs() {
    let p = assert_lossless("m: {a: 1, b, ? c : d}\ns: [x, y: z, \"q\"]\n");
    assert!(p.errors.is_empty());
    assert_eq!(count(&p.tree, k::FLOW_MAPPING), 1);
    assert_eq!(count(&p.tree, k::FLOW_SEQUENCE), 1);
    // Three entries of the mapping and the single pair in the sequence.
    assert_eq!(count(&p.tree, k::FLOW_PAIR), 4);
}

#[test]
fn properties_wrap_their_node() {
    let p = assert_lossless("a: &x !!str b\nc: *x\nd: !t\n");
    assert!(p.errors.is_empty());
    assert_eq!(count(&p.tree, k::PROPERTY_NODE), 2);
    assert_eq!(count(&p.tree, k::ALIAS_NODE), 1);
}

#[test]
fn block_scalars_split_header_and_content() {
    let p = assert_lossless("a: |+ # c\n  x\n\n\nb: >\n  y\n");
    assert!(p.errors.is_empty());
    assert_eq!(count(&p.tree, k::BLOCK_SCALAR), 2);
    let content: Vec<String> = p
        .tree
        .descendants_with_tokens()
        .filter_map(|e| e.into_token())
        .filter(|t| t.kind() == k::BLOCK_CONTENT)
        .map(|t| t.text().to_string())
        .collect();
    assert_eq!(content, ["  x\n\n\n", "  y"]);
}

#[test]
fn documents_markers_and_directives() {
    let p = assert_lossless("%YAML 1.2\n---\na\n...\n---\nb\n--- c\n");
    assert!(p.errors.is_empty());
    assert_eq!(count(&p.tree, k::DOCUMENT), 3);
    let p = assert_lossless("# only a comment\n");
    assert!(p.errors.is_empty());
    assert_eq!(count(&p.tree, k::DOCUMENT), 0);
}

#[test]
fn comments_follow_the_attachment_rule() {
    let p = assert_lossless("# top\na: 1 # one\n# before b\nb: 2\n");
    assert!(p.errors.is_empty());
}

#[test]
fn errors_become_an_error_node_that_keeps_every_byte() {
    for text in [
        "a: b: c\n",
        "key: [1, 2\n",
        "- a\n b: c\n",
        "a: 'unterminated\n",
        "\ta: 1\n",
        "a:\n  - b\n c: d\n",
        "--- a: b\n",
        "%YAML 1.2\n",
        "a: \"\\q\"\n",
    ] {
        let p = assert_lossless(text);
        assert!(!p.errors.is_empty(), "{text:?} should be an error");
        assert!(count(&p.tree, SyntaxKind::ERROR) >= 1, "{text:?}");
    }
}

#[test]
fn an_error_in_one_document_does_not_hide_the_next() {
    let p = assert_lossless("a: b: c\n---\nd: e\n");
    assert!(!p.errors.is_empty());
    assert_eq!(count(&p.tree, k::DOCUMENT), 2);
    assert_eq!(count(&p.tree, SyntaxKind::ERROR), 1);
}

#[test]
fn crlf_and_unicode_are_kept() {
    let p = assert_lossless("é: \"ü\"\r\nlist:\r\n- ☃\r\n");
    assert!(p.errors.is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// Whatever the input, the tree prints back to it and nothing panics.
    #[test]
    fn any_input_round_trips(text in r"[a-z0-9 :\-\[\]{},#&*!|>'\x22%?\\\n\t.é]{0,80}") {
        let parsed = parse(&text);
        prop_assert_eq!(print(&parsed.tree), text);
        if parsed.errors.is_empty() {
            let g = core_syntax::grammars::yaml();
            prop_assert!(trivia::check_attachment(&parsed.tree, |kind| g.is_trivia(kind)).is_ok());
            // Loading a well-formed tree must not fail on anything but undefined aliases.
            if let Err(e) = lang_yaml::model::load_tree(&parsed.tree) {
                prop_assert!(e.contains("undefined alias") || e.contains("invalid escape"), "{}", e);
            }
        }
    }
}

//! Formatter snapshots: one case per construct, per option value and per hook. Every case also
//! goes through the shared checks (round-trip, meaning, idempotency, comments kept) via
//! `common::fmt`.

mod common;

use common::{fmt, n, s};
use core_config::OptionValue;

macro_rules! snap {
    ($name:ident, $input:expr) => {
        snap!($name, $input, []);
    };
    ($name:ident, $input:expr, [$(($k:expr, $v:expr)),* $(,)?]) => {
        #[test]
        fn $name() {
            let input: &str = $input;
            let out = fmt(input, &[$(($k, $v)),*]);
            insta::assert_snapshot!(format!("{input}\n-----\n{out}"));
        }
    };
}

fn flag(b: bool) -> OptionValue {
    OptionValue::Bool(b)
}

// Block mappings and sequences ------------------------------------------------------------------

snap!(
    mapping_spacing_is_normalized,
    "a:    1\nb:\t2\nlong key  :   value   \n"
);
snap!(
    nested_mappings_are_reindented,
    "a:\n      b:\n            c: 1\n      d: 2\ne: 3\n"
);
snap!(
    sequences_keep_their_indentation_by_default,
    "flush:\n- a\n- b\nindented:\n    - c\n    - d\n"
);
snap!(
    sequence_indent_indented,
    "flush:\n- a\n- b\nindented:\n    - c\n    - d\ntop:\n- x\n",
    [("sequence_indent", s("indented"))]
);
snap!(
    sequence_indent_flush,
    "flush:\n- a\n- b\nindented:\n    - c\n    - d\nnested:\n  inner:\n      - e\n",
    [("sequence_indent", s("flush"))]
);
snap!(
    sequence_indent_indented_with_four_spaces,
    "a:\n- b:\n  - c\n  - d\n",
    [("sequence_indent", s("indented")), ("indent_size", n(4))]
);
snap!(
    sequences_of_mappings_and_sequences,
    "- name: a\n  value: 1\n-   - x\n    - y\n- - z\n-\n  k: v\n- k2:   v2\n  list:\n  - 1\n"
);
snap!(empty_values_and_null_entries, "a:\nb: ~\nc:\n-\n- \n- d\n");
snap!(
    explicit_keys,
    "? a\n: b\n? [x, y]\n: z\n? |\n  block key\n: v\n? only key\n? - s1\n  - s2\n: - s3\n"
);
snap!(
    aliases_as_keys_get_a_space,
    "a: &x 1\n*x : b\n&k key: v\n!!str : tagged\n"
);

// Properties ----------------------------------------------------------------------------------

snap!(
    anchors_tags_and_aliases,
    "base: &b\n  k: v\nuse: *b\nt: !!str  12\nm: !!map\n  a: b\ns: !!seq\n- x\nlist:\n- &i item\n- !t {a: 1}\n- &c\n  k: v\n"
);
snap!(properties_on_separate_lines, "&a1\n!!str\nscalar\n");

// Scalars ---------------------------------------------------------------------------------------

snap!(
    multi_line_flow_scalars_are_reindented,
    "plain: first line\n    second line\n\n    after blank\nquoted: \"one\n    two\"\nsingle: 'a\n b'\nseq:\n- plain\n    more\n"
);
snap!(
    quotes_are_kept_by_default,
    "a: 'single'\nb: \"double\"\nc: plain\nd: \"it's\"\ne: 'say \"hi\"'\n"
);
snap!(
    quote_style_single,
    "a: \"double\"\nb: 'single'\nc: \"it's\"\nd: \"say \\\"hi\\\"\"\ne: \"tab\\t\"\nf: \"\\u00e9\"\ng: \"\"\n",
    [("quote_style", s("single"))]
);
snap!(
    quote_style_double,
    "a: 'single'\nb: \"double\"\nc: 'it''s'\nd: 'say \"hi\"'\ne: 'back\\slash'\nf: ''\n",
    [("quote_style", s("double"))]
);
snap!(
    quote_style_leaves_multi_line_scalars_alone,
    "a: \"one\n  two\"\nb: 'x\n  y'\n",
    [("quote_style", s("single"))]
);
snap!(
    block_scalars_are_reindented_not_rewrapped,
    "lit: |\n      deep text\n        more   \n      end\nfold: >-\n   folded\n   text\n\n   para\nlist:\n  - |\n        item\n  - next\n"
);
snap!(
    block_scalar_chomping_and_blank_lines,
    "strip: |-\n  text\n\nclip: |\n  text\n\n\nkeep: |+\n  text\n\n\nlast: x\n"
);
snap!(
    block_scalar_indentation_indicator_follows_the_indent,
    "a: |2\n    two extra\nb:\n  - >1\n     one extra\n",
    [("indent_size", n(4))]
);
snap!(
    block_scalar_with_header_comment_and_properties,
    "a: !!str | # comment\n  text\nb: &x >\n  folded\n"
);
snap!(document_level_block_scalar, "--- |\n  top level\n  text\n");

// Flow collections ----------------------------------------------------------------------------

snap!(
    flow_collections_stay_on_one_line,
    "m: {a: 1,b: 2}\ns: [1,2,   3]\ne: {}\nn: []\nnested: {a: [1, 2], b: {c: d}}\n"
);
snap!(
    flow_collection_that_was_broken_stays_broken,
    "s: [\n  1, 2,\n  3]\nm: {a: 1,\n  b: 2}\n"
);
snap!(
    flow_collection_that_does_not_fit_moves_to_its_own_line,
    "key: [alpha, beta, gamma, delta, epsilon, zeta, eta, theta, iota, kappa, lambda, mu]\nshort: [a]\n",
    [("line_width", n(60))]
);
snap!(
    flow_collection_in_a_sequence_stays_valid_when_broken,
    "- [alpha, beta, gamma, delta, epsilon, zeta, eta, theta, iota, kappa, lambda, mu]\n- {alpha: 1, beta: 2, gamma: 3, delta: 4, epsilon: 5, zeta: 6, eta: 7, theta: 8}\n",
    [("line_width", n(40))]
);
snap!(
    flow_pairs_and_explicit_flow_keys,
    "a: &a x\nb: [x: 1, y]\nc: {? k : v, z, ? w}\nd: [\"q\": 1]\ne: { *a : 1 }\n"
);
snap!(
    flow_keys_are_never_broken,
    "[aaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb, cccccccccccccccccccc]: v\n",
    [("line_width", n(30))]
);
snap!(
    flow_comments,
    "a: [1, # one\n  2, 3] # after\nb: {x: 1,\n  # own line\n  y: 2}\n"
);

// Comments ------------------------------------------------------------------------------------

snap!(
    comments_are_kept_where_they_are,
    "# top\n\n# second\na: 1   # trailing\n# before b\nb:   # on key\n  c: 2\n  # about c\nd: 3\n# end\n"
);
snap!(
    comment_before_a_nested_value,
    "key:\n  # about the value\n  value\nother: # on colon\n  value\n"
);
snap!(
    sequence_comments_between_and_after_entries,
    "list:\n  - a\n  # between\n  - b   # trailing b\n    # inside b?\n  - c\n  # after the list\nnext: 1\n"
);
snap!(
    comments_after_nested_collections_return_to_their_level,
    "a:\n  b:\n    c: 1\n    # about c\n  # about b\n# about a\nd: 2\n    # indented at the end\n"
);
snap!(
    comment_after_a_dash,
    "- # first\n  a: 1\n-\n  # own line\n  b: 2\n- # third\n  - nested\n"
);
snap!(
    comments_at_the_end_of_the_file,
    "a:\n  - b\n\n\n  # deep\n# shallow\n"
);
snap!(
    comments_in_a_stream_without_documents,
    "# only a comment\n\n\n# another\n"
);
snap!(
    ignore_next_entry,
    "a:    1\n# sudsy-ignore\nb:   [1,    2]\nc:   3\n"
);
snap!(
    ignore_region,
    "a:    1\n# sudsy-ignore-start\nb:   [1,    2]\n  # inside\nc:    3\n# sudsy-ignore-end\nd:    4\n"
);

// Blank lines ---------------------------------------------------------------------------------

snap!(
    blank_lines_are_kept_once,
    "a: 1\n\n\n\nb: 2\nc:\n\n  d: 1\n\n\n  e: 2\n"
);
snap!(
    max_blank_lines_zero,
    "a: 1\n\n\nb:\n  - x\n\n  - y\n\n# c\nc: 3\n",
    [("max_blank_lines", n(0))]
);
snap!(
    max_blank_lines_three,
    "a: 1\n\n\n\n\n\nb: 2\n\n\n# c\n\n\n\nc: 3\n",
    [("max_blank_lines", n(3))]
);

// Documents -----------------------------------------------------------------------------------

snap!(several_documents, "a: 1\n---\nb: 2\n...\n---\n- c\n...\n");
snap!(
    directives_and_inline_bodies,
    "%YAML 1.2\n%TAG !e! tag:example.com,2000:\n--- !e!foo\nk: v\n--- text\n--- |\n  literal\n--- [a, b]\n--- # comment\nz: 1\n"
);
snap!(document_start_preserve, "a: 1\n---\nb: 2\n");
snap!(
    document_start_always,
    "a: 1\n---\nb: 2\n...\n# comment\nc: 3\n",
    [("document_start", s("always"))]
);
snap!(
    document_start_never,
    "---\na: 1\n---\nb: 2\n",
    [("document_start", s("never"))]
);
snap!(
    document_start_never_keeps_what_it_needs,
    "%YAML 1.2\n---\na: 1\n...\n--- # keep\n",
    [("document_start", s("never"))]
);
snap!(
    document_start_never_keeps_the_comments,
    "--- # about the document\n# first\na: 1\n",
    [("document_start", s("never"))]
);
snap!(
    document_end_always,
    "a: 1\n---\nb: 2\n...\n---\n- c\n# trailing\n",
    [("document_end", s("always"))]
);
snap!(
    document_end_never,
    "a: 1\n...\n---\nb: 2\n... # about it\n",
    [("document_end", s("never"))]
);
snap!(
    document_end_never_keeps_what_a_directive_needs,
    "a: 1\n...\n%YAML 1.2\n---\nb: 2\n...\n",
    [("document_end", s("never"))]
);
snap!(
    document_end_never_keeps_what_a_bare_or_empty_document_needs,
    "a: 1\n...\nb: 2\n...\n# only a comment\n...\n",
    [("document_end", s("never"))]
);
snap!(
    document_end_always_keeps_the_blank_lines_of_a_keep_scalar,
    "a: |+\n  x\n\n\n",
    [("document_end", s("always"))]
);
snap!(
    empty_and_comment_only_documents,
    "---\n# only a comment\n---\n---\nx\n"
);

// Other options -------------------------------------------------------------------------------

snap!(
    indent_size_four,
    "a:\n  b:\n    - c\n    - d: 1\n      e: |\n        text\n",
    [("indent_size", n(4))]
);
snap!(
    indent_size_one,
    "a:\n  b:\n    - c\n    - d: 1\n",
    [("indent_size", n(1)), ("sequence_indent", s("indented"))]
);
snap!(
    final_newline_off,
    "a: 1\n\n\n",
    [("final_newline", flag(false))]
);
snap!(
    final_newline_is_added,
    "a: 1",
    [("final_newline", flag(true))]
);

#[test]
fn line_endings_are_kept() {
    let out = fmt("a:   1\r\nb:\r\n  - x\r\n", &[]);
    assert_eq!(out, "a: 1\r\nb:\r\n  - x\r\n");
}

#[test]
fn undefined_aliases_are_plain_tokens_to_the_formatter() {
    assert_eq!(
        fmt(
            "a:   *nope
b: [ *x,*y ]
",
            &[]
        ),
        "a: *nope
b: [*x, *y]
"
    );
}

#[test]
fn an_empty_file_stays_empty() {
    assert_eq!(fmt("", &[]), "");
    assert_eq!(fmt("\n\n", &[]), "");
}

#[test]
fn formatted_output_is_a_fixed_point_for_the_config_file_itself() {
    let sample = "extends:\n- preset:csharp/microsoft\nformat:\n    line_width:   100\nlint:\n  rules:\n      max-line-length: warn\nyaml:\n  lint: {rules: {ambiguous-booleans: warn}}\n";
    let once = fmt(sample, &[]);
    assert_eq!(fmt(&once, &[]), once);
}

// Comment spacing --------------------------------------------------------------------------------

snap!(
    inline_comment_gap,
    "#!shebang-like but not first\n#no space\n### run\n#\na: 1 # one\nb: 2   #two\nc:\n  - x #y\n  #z\nd: {k: v} #f\n",
    [("comment_spacing", n(2))]
);

snap!(
    comments_are_kept_as_written_by_default,
    "#nospace\na: 1   #c\n"
);

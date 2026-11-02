//! Formatter snapshots: one case per shape and per option value. Every case also goes through the
//! shared checks (round-trip, meaning, idempotency, comments kept) via `common::fmt_path`.

mod common;

use common::{fmt_path, s};
use core_config::OptionValue;

macro_rules! snap {
    ($name:ident, $path:expr, $input:expr) => {
        snap!($name, $path, $input, []);
    };
    ($name:ident, $path:expr, $input:expr, [$(($k:expr, $v:expr)),* $(,)?]) => {
        #[test]
        fn $name() {
            let input: &str = $input;
            let out = fmt_path($path, input, &[$(($k, $v)),*]);
            insta::assert_snapshot!(format!("{input}\n-----\n{out}"));
        }
    };
}

fn int(n: i64) -> OptionValue {
    OptionValue::Int(n)
}

fn flag(b: bool) -> OptionValue {
    OptionValue::Bool(b)
}

const NESTED_FLAT: &str =
    r#"{"name":"sudsy","tags":["a","b"],"nested":{"x":1,"y":[1,2,3]},"empty":{},"none":[]}"#;
const NESTED_BROKEN: &str = r#"{
  "name": "sudsy", "tags": ["a", "b"],
  "nested": {"x": 1,
    "y": [
      1, 2, 3]},
  "empty": {}, "none": []
}"#;

// Delimited list and block shape defaults ------------------------------------------------------

snap!(object_flat_stays_flat, "a.json", NESTED_FLAT);
snap!(
    object_with_break_after_brace_stays_expanded,
    "a.json",
    NESTED_BROKEN
);
snap!(scalars_at_top_level, "a.json", "  \"just a string\"  ");
snap!(empty_containers, "a.json", "[ ]\n");
snap!(empty_object_with_newline, "a.json", "{\n}\n");
snap!(
    long_flat_array_breaks_to_fit,
    "a.json",
    "[\"alpha\", \"beta\", \"gamma\", \"delta\", \"epsilon\", \"zeta\", \"eta\", \"theta\", \"iota\"]"
);
snap!(
    long_flat_object_breaks_to_fit,
    "a.json",
    r#"{"alpha": "aaaaaaaaaaaa", "beta": "bbbbbbbbbbbb", "gamma": "cccccccccccc", "delta": [1, 2]}"#
);
snap!(
    blank_lines_between_items_are_kept_once,
    "a.json",
    "{\n\n  \"a\": 1,\n\n\n\n  \"b\": 2,\n  \"c\": 3\n\n}\n"
);
snap!(
    blank_lines_between_array_items,
    "a.json",
    "[\n  1,\n\n\n  2,\n  3\n]"
);
snap!(
    blank_lines_vanish_when_collapsed,
    "a.json",
    "{\"a\": 1,\n\n\"b\": 2}"
);
snap!(crlf_input, "a.json", "{\r\n  \"a\": [1,\r\n 2]\r\n}\r\n");
snap!(
    string_escapes_and_unicode_are_untouched,
    "a.json",
    r#"["aé\n\"", "日本語", 1.50E+10, -0, 0.1e-2]"#
);

// collapse -------------------------------------------------------------------------------------

snap!(
    collapse_preserve,
    "a.json",
    NESTED_BROKEN,
    [("collapse", s("preserve"))]
);
snap!(
    collapse_always,
    "a.json",
    NESTED_BROKEN,
    [("collapse", s("always"))]
);
snap!(
    collapse_never,
    "a.json",
    NESTED_FLAT,
    [("collapse", s("never"))]
);
snap!(
    collapse_never_keeps_empty_containers_empty,
    "a.json",
    "{\"a\": [], \"b\": {}}",
    [("collapse", s("never"))]
);
snap!(
    collapse_always_still_breaks_what_does_not_fit,
    "a.json",
    "[\n[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28]]",
    [("collapse", s("always"))]
);

// indent, width, final newline -----------------------------------------------------------------

snap!(
    indent_size_4,
    "a.json",
    NESTED_BROKEN,
    [("indent_size", int(4))]
);
snap!(
    indent_size_1,
    "a.json",
    NESTED_BROKEN,
    [("indent_size", int(1))]
);
snap!(
    indent_tabs,
    "a.json",
    NESTED_BROKEN,
    [("indent_style", s("tab"))]
);
snap!(
    line_width_20,
    "a.json",
    NESTED_FLAT,
    [("line_width", int(20))]
);
snap!(
    line_width_120,
    "a.json",
    "[\"alpha\", \"beta\", \"gamma\", \"delta\", \"epsilon\", \"zeta\", \"eta\", \"theta\", \"iota\", \"kappa\"]",
    [("line_width", int(120))]
);
snap!(
    no_final_newline,
    "a.json",
    "{\"a\": 1}\n\n\n",
    [("final_newline", flag(false))]
);
snap!(
    final_newline_added,
    "a.json",
    "[1]",
    [("final_newline", flag(true))]
);

// dialects and trailing commas -----------------------------------------------------------------

snap!(
    trailing_commas_never,
    "a.jsonc",
    "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2,\n  ],\n}",
    [("trailing_commas", s("never"))]
);
snap!(
    trailing_commas_multiline,
    "a.jsonc",
    "{\n  \"a\": 1,\n  \"b\": [\n    1,\n    2\n  ]\n}",
    [("trailing_commas", s("multiline"))]
);
snap!(
    trailing_commas_multiline_not_added_to_one_line_lists,
    "a.jsonc",
    "{\"a\": [1, 2]}",
    [("trailing_commas", s("multiline"))]
);
snap!(
    trailing_commas_ignored_for_plain_json,
    "a.json",
    "{\n  \"a\": 1\n}",
    [("trailing_commas", s("multiline"))]
);
snap!(
    tsconfig_is_jsonc,
    "tsconfig.json",
    "{\"compilerOptions\": {\n// strict\n\"strict\": true,\n}}",
    []
);
snap!(
    vscode_settings_are_jsonc,
    ".vscode/settings.json",
    "{\n \"a\": 1, // one\n}",
    []
);
snap!(
    dialect_option_forces_jsonc,
    "a.json",
    "[1, // one\n 2]",
    [("dialect", s("jsonc"))]
);

// comments (JSONC) ------------------------------------------------------------------------------

snap!(
    comments_leading_trailing_and_blank_lines,
    "a.jsonc",
    "// file header\n\n{\n  // about a\n  \"a\": 1, // trailing a\n\n  /* about b */\n  \"b\": 2 /* inline */,\n  \"c\": 3 // last\n}\n// footer\n"
);
snap!(
    comment_after_open_bracket,
    "a.jsonc",
    "[ // first\n  1,\n  2\n]"
);
snap!(
    comment_in_empty_object,
    "a.jsonc",
    "{\n  // nothing here\n}"
);
snap!(
    comment_in_empty_array_same_line,
    "a.jsonc",
    "[ /* empty */ ]"
);
snap!(
    comment_before_closing_bracket,
    "a.jsonc",
    "{\n  \"a\": 1\n  // trailing thoughts\n}"
);
snap!(
    inline_block_comments_stay_inline,
    "a.jsonc",
    "[1, /* two */ 2, 3 /* three */]"
);
snap!(
    multi_line_block_comment,
    "a.jsonc",
    "{\n  /* line one\n     line two */\n  \"a\": 1\n}"
);
snap!(
    comment_after_comma,
    "a.jsonc",
    "{\"a\": 1, // after comma\n \"b\": 2}"
);
snap!(
    line_comment_forces_expansion,
    "a.jsonc",
    "[1, 2 // two\n, 3]"
);
snap!(
    comments_before_and_after_a_comma_stay_separate,
    "a.jsonc",
    "[
  1 // before
  , // after
  2 /* a */ // b
]"
);
snap!(
    comment_between_key_and_value,
    "a.jsonc",
    "{\"a\": /* v */ 1}"
);

// sudsy-ignore ---------------------------------------------------------------------------------

snap!(
    ignore_next_member,
    "a.jsonc",
    "{\n  \"a\":   1,\n  // sudsy-ignore\n  \"b\":   [1,2,    3],\n  \"c\":   [1,2,    3]\n}"
);
snap!(
    ignore_whole_document_value,
    "a.jsonc",
    "// sudsy-ignore\n{\"a\":   1,\n\n\n  \"b\":2}\n"
);
snap!(
    ignore_region_in_object,
    "a.jsonc",
    "{\n  \"a\":   1,\n  // sudsy-ignore-start\n  \"b\":   [1,2,    3],\n  \"c\":   {  \"d\":4  },\n  // sudsy-ignore-end\n  \"e\":   5\n}"
);
snap!(
    ignore_region_in_array_runs_to_the_end,
    "a.jsonc",
    "[\n  1,\n  /* sudsy-ignore-start */\n  2,    3,\n  4\n]"
);
snap!(
    ignore_block_comment_marker,
    "a.jsonc",
    "[\n  /* sudsy-ignore */ {\"a\":   1},\n  {\"b\":   2}\n]"
);
snap!(
    not_an_ignore_marker,
    "a.jsonc",
    "[\n  // sudsy-ignored later\n  {\"a\":   1}\n]"
);

#[test]
fn deeply_nested_input_is_formatted() {
    // Within the parser's depth limit; this must not overflow the stack of a test thread.
    let depth = 200;
    let input = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
    let out = fmt_path("a.json", &input, &[]);
    assert_eq!(out.lines().count(), 2 * depth + 1);
}

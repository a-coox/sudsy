use core_format::doc::*;
use core_format::printer::{PrintOptions, print};

fn opts(width: usize) -> PrintOptions {
    PrintOptions {
        line_width: width,
        indent_width: 2,
        use_tabs: false,
    }
}

fn run(mut d: Doc, width: usize) -> String {
    print(&mut d, &opts(width))
}

fn t(s: &str) -> Doc {
    text(s)
}

fn call(args: &[&str]) -> Doc {
    group(concat(vec![
        t("f("),
        indent(concat(vec![
            softline(),
            join(&concat(vec![t(","), line()]), args.iter().map(|a| t(a))),
        ])),
        softline(),
        t(")"),
    ]))
}

#[test]
fn text_and_concat() {
    assert_eq!(run(concat(vec![t("a"), nil(), t("b")]), 80), "ab");
}

#[test]
fn group_fits_flat_or_breaks() {
    let args = ["aaaa", "bbbb", "cccc"];
    assert_eq!(run(call(&args), 80), "f(aaaa, bbbb, cccc)");
    assert_eq!(run(call(&args), 12), "f(\n  aaaa,\n  bbbb,\n  cccc\n)");
}

#[test]
fn nested_groups_break_outermost_first() {
    let inner = call(&["x", "y"]);
    let outer = group(concat(vec![
        t("outer("),
        indent(concat(vec![
            softline(),
            inner,
            t(","),
            line(),
            t("zzzzzzzz"),
        ])),
        softline(),
        t(")"),
    ]));
    assert_eq!(run(outer.clone(), 80), "outer(f(x, y), zzzzzzzz)");
    assert_eq!(run(outer, 20), "outer(\n  f(x, y),\n  zzzzzzzz\n)");
}

#[test]
fn hardline_forces_enclosing_groups_to_break() {
    let d = group(concat(vec![
        t("a"),
        indent(concat(vec![hardline(), t("b")])),
        line(),
        t("c"),
    ]));
    assert_eq!(run(d, 80), "a\n  b\nc");
}

#[test]
fn break_parent_breaks_the_group() {
    let d = group(concat(vec![t("a"), line(), t("b"), break_parent()]));
    assert_eq!(run(d, 80), "a\nb");
}

#[test]
fn group_broken_and_softline() {
    let d = group_broken(concat(vec![t("a"), softline(), t("b")]));
    assert_eq!(run(d, 80), "a\nb");
    let d = group(concat(vec![t("a"), softline(), t("b")]));
    assert_eq!(run(d, 80), "ab");
}

#[test]
fn indent_and_align() {
    let d = indent(concat(vec![
        hardline(),
        t("a"),
        align(3, concat(vec![hardline(), t("b")])),
    ]));
    assert_eq!(run(d, 80), "\n  a\n     b");
}

#[test]
fn tabs_indent_with_tab_characters() {
    let mut d = indent(concat(vec![t("a"), hardline(), t("b")]));
    let o = PrintOptions {
        line_width: 80,
        indent_width: 4,
        use_tabs: true,
    };
    assert_eq!(print(&mut d, &o), "a\n\tb");
}

#[test]
fn if_break_picks_by_group_mode() {
    let list = |w| {
        run(
            group(concat(vec![
                t("["),
                indent(concat(vec![softline(), t("aaaa"), if_break(t(","), nil())])),
                softline(),
                t("]"),
            ])),
            w,
        )
    };
    assert_eq!(list(80), "[aaaa]");
    assert_eq!(list(5), "[\n  aaaa,\n]");
}

#[test]
fn line_suffix_goes_to_the_end_of_the_line() {
    let d = concat(vec![
        t("a,"),
        line_suffix(t(" // c")),
        hardline(),
        t("b"),
        line_suffix(t(" // end")),
    ]);
    assert_eq!(run(d, 80), "a, // c\nb // end");
}

#[test]
fn line_suffix_does_not_count_towards_fitting() {
    let d = group(concat(vec![
        t("a"),
        line_suffix(t(" // long long long")),
        line(),
        t("b"),
    ]));
    assert_eq!(run(d, 10), "a b // long long long");
}

#[test]
fn fill_wraps_at_the_width() {
    let words: Vec<Doc> = "one two three four five six".split(' ').map(t).collect();
    let mut parts = Vec::new();
    for (i, w) in words.into_iter().enumerate() {
        if i > 0 {
            parts.push(line());
        }
        parts.push(w);
    }
    assert_eq!(run(fill(parts.clone()), 80), "one two three four five six");
    assert_eq!(run(fill(parts), 12), "one two\nthree four\nfive six");
}

#[test]
fn conditional_group_takes_the_first_candidate_that_fits() {
    let hug = concat(vec![
        t("f("),
        t("x"),
        t(", "),
        group_broken(concat(vec![
            t("{"),
            indent(concat(vec![hardline(), t("body")])),
            hardline(),
            t("}"),
        ])),
        t(")"),
    ]);
    let all_broken = concat(vec![
        t("f("),
        indent(concat(vec![hardline(), t("x,"), hardline(), t("{ body }")])),
        hardline(),
        t(")"),
    ]);
    let flat = t("f(x, { body })");
    let d = |w| {
        run(
            conditional_group(vec![flat.clone(), hug.clone(), all_broken.clone()]),
            w,
        )
    };
    assert_eq!(d(80), "f(x, { body })");
    assert_eq!(d(8), "f(x, {\n  body\n})");
    assert_eq!(d(2), "f(\n  x,\n  { body }\n)");
}

#[test]
fn trailing_whitespace_is_trimmed() {
    let d = concat(vec![
        t("a "),
        hardline(),
        indent(concat(vec![hardline(), t("b")])),
    ]);
    assert_eq!(run(d, 80), "a\n\n  b");
}

#[test]
fn multi_line_text_resets_the_column() {
    let d = group(concat(vec![t("/* a\n b */"), line(), t("c")]));
    assert_eq!(run(d, 10), "/* a\n b */ c");
}

#[test]
fn wide_characters_count_by_display_width() {
    let d = group(concat(vec![t("日本語"), line(), t("x")]));
    assert_eq!(run(d.clone(), 7), "日本語\nx");
    assert_eq!(run(d, 8), "日本語 x");
}

#[test]
fn text_without_content_is_nil() {
    assert_eq!(text(""), nil());
}

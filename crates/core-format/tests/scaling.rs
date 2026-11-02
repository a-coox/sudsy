//! The printer's work grows linearly with the document. The check counts the steps `fits` takes
//! instead of measuring time, so it cannot flake: a measurement that rescans what follows its
//! group (or a document that is walked once per nesting level) would make the count grow
//! quadratically and fail the ratio bound.

use core_format::doc::*;
use core_format::printer::{PrintOptions, print_counting};

fn steps(mut d: Doc) -> usize {
    print_counting(&mut d, &PrintOptions::default())
        .2
        .fits_steps
}

/// An object that has to break, holding `n` short groups that fit on their own line.
fn many_groups(n: usize) -> Doc {
    let items = (0..n).flat_map(|i| {
        [
            hardline(),
            group(concat(vec![
                text("["),
                indent(concat(vec![
                    softline(),
                    text(format!("item{i}")),
                    line(),
                    text("x"),
                ])),
                softline(),
                text("]"),
            ])),
        ]
    });
    group(concat(vec![
        text("{"),
        indent(concat(items.collect())),
        hardline(),
        text("}"),
    ]))
}

/// A chain of `depth` groups, each holding a short line and the next group.
fn nested(depth: usize) -> Doc {
    let mut inner = text("leaf");
    for i in 0..depth {
        inner = group(concat(vec![
            text(format!("n{i}")),
            hardline(),
            group(concat(vec![text("a"), line(), text("b")])),
            hardline(),
            inner,
        ]));
    }
    inner
}

/// A fill of `n` words.
fn words(n: usize) -> Doc {
    let mut parts = Vec::new();
    for i in 0..n {
        if i > 0 {
            parts.push(line());
        }
        parts.push(text(format!("word{i}")));
    }
    fill(parts)
}

/// Quadrupling the document may at most quadruple the work (plus a little slack); a quadratic
/// printer would multiply it by sixteen.
fn assert_linear(name: &str, build: impl Fn(usize) -> Doc, small: usize) {
    let (a, b) = (steps(build(small)), steps(build(small * 4)));
    assert!(a > 0, "{name}: nothing was measured");
    assert!(
        b <= a * 4 + 64,
        "{name}: {small} units took {a} steps but {} took {b}",
        small * 4
    );
}

#[test]
fn groups_in_a_long_list_are_measured_in_linear_time() {
    assert_linear("many groups", many_groups, 500);
}

#[test]
fn nesting_depth_does_not_make_the_printer_quadratic() {
    assert_linear("nested", nested, 40);
}

#[test]
fn a_fill_is_measured_in_linear_time() {
    assert_linear("fill", words, 500);
}

#[test]
fn each_group_costs_a_bounded_number_of_steps() {
    let n = 2000;
    let per_group = steps(many_groups(n)) as f64 / n as f64;
    assert!(per_group < 40.0, "{per_group} fits steps per group");
}

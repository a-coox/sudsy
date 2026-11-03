//! Fix selection, application and the loop around them, with hand-made diagnostics.

use core_rules::fixes::{MAX_ITERATIONS, apply, select};
use core_rules::*;
use core_syntax::{TextRange, TextSize};

fn range(start: u32, end: u32) -> TextRange {
    TextRange::new(TextSize::from(start), TextSize::from(end))
}

fn diagnostic(rule: &str, category: Category, fix: Fix) -> Diagnostic {
    Diagnostic {
        rule: rule.into(),
        category,
        severity: Severity::Warning,
        message: String::new(),
        range: fix.edits[0].range,
        related: Vec::new(),
        fixes: vec![fix],
    }
}

fn replace(title: &str, kind: FixKind, start: u32, end: u32, text: &str) -> Fix {
    Fix::new(title, kind, vec![Edit::replace(range(start, end), text)])
}

#[test]
fn only_the_kinds_asked_for_are_selected() {
    let diagnostics = vec![
        diagnostic(
            "a",
            Category::Style,
            replace("safe", FixKind::Safe, 0, 1, "x"),
        ),
        diagnostic(
            "b",
            Category::Style,
            replace("unsafe", FixKind::Unsafe, 2, 3, "x"),
        ),
        diagnostic(
            "c",
            Category::Style,
            replace("hint", FixKind::Suggestion, 4, 5, "x"),
        ),
    ];
    let titles = |allow_unsafe| -> Vec<String> {
        select(&diagnostics, |f| f.kind.applies(allow_unsafe))
            .iter()
            .map(|f| f.title.clone())
            .collect()
    };
    assert_eq!(titles(false), ["safe"]);
    assert_eq!(titles(true), ["safe", "unsafe"]);
}

#[test]
fn overlapping_fixes_are_resolved_by_category_then_rule() {
    let diagnostics = vec![
        diagnostic(
            "style-rule",
            Category::Style,
            replace("style", FixKind::Safe, 0, 5, "S"),
        ),
        diagnostic(
            "zz",
            Category::Correctness,
            replace("late", FixKind::Safe, 3, 8, "C"),
        ),
        diagnostic(
            "aa",
            Category::Correctness,
            replace("early", FixKind::Safe, 6, 9, "C"),
        ),
    ];
    let chosen = select(&diagnostics, |_| true);
    let titles: Vec<&str> = chosen.iter().map(|f| f.title.as_str()).collect();
    // `aa` wins over `zz`, which overlaps it; the style fix overlaps neither winner... except
    // `zz` was dropped, so it fits.
    assert_eq!(titles, ["early", "style"]);
}

#[test]
fn a_fix_with_several_edits_is_applied_whole_or_not_at_all() {
    let two = Fix::new(
        "two",
        FixKind::Safe,
        vec![
            Edit::replace(range(0, 1), "a"),
            Edit::replace(range(6, 7), "b"),
        ],
    );
    let diagnostics = vec![
        diagnostic(
            "x",
            Category::Correctness,
            replace("blocker", FixKind::Safe, 5, 8, "!"),
        ),
        diagnostic("y", Category::Style, two),
    ];
    let chosen = select(&diagnostics, |_| true);
    assert_eq!(chosen.len(), 1);
    assert_eq!(chosen[0].title, "blocker");
}

#[test]
fn apply_handles_inserts_deletes_and_order() {
    let fixes = [Fix::new(
        "all",
        FixKind::Safe,
        vec![
            Edit::insert(TextSize::from(5), ">"),
            Edit::delete(range(1, 2)),
            Edit::insert(TextSize::from(0), "<"),
            Edit::replace(range(3, 4), "XY"),
        ],
    )];
    let refs: Vec<&Fix> = fixes.iter().collect();
    assert_eq!(apply("abcdef", &refs), "<acXYe>f");
}

/// A pretend rule: every `a` becomes `b`; the text parses while it is not empty.
fn analyze_a(text: &str) -> Result<Analysis, ()> {
    let diagnostics = text
        .match_indices('a')
        .map(|(i, _)| {
            diagnostic(
                "a-to-b",
                Category::Style,
                replace("fix", FixKind::Safe, i as u32, i as u32 + 1, "b"),
            )
        })
        .collect();
    Ok(Analysis {
        diagnostics,
        parses: !text.is_empty(),
    })
}

#[test]
fn the_loop_runs_until_nothing_is_left_and_is_idempotent() {
    let out = fix_loop("banana".into(), |_| true, analyze_a).unwrap();
    assert_eq!(out.text, "bbnbnb");
    assert_eq!(out.applied, 3);
    assert_eq!(out.broke, None);
    let again = fix_loop(out.text.clone(), |_| true, analyze_a).unwrap();
    assert_eq!(
        (again.text, again.applied, again.iterations),
        (out.text, 0, 0)
    );
}

#[test]
fn the_loop_stops_after_ten_rounds() {
    // Each round turns the first `a` into `aa`... two fixes that keep feeding each other.
    let analyze = |text: &str| -> Result<Analysis, ()> {
        Ok(Analysis {
            diagnostics: vec![diagnostic(
                "grow",
                Category::Style,
                replace("grow", FixKind::Safe, 0, 1, "ab"),
            )],
            parses: !text.is_empty(),
        })
    };
    let out = fix_loop("a".into(), |_| true, analyze).unwrap();
    assert_eq!(out.iterations, MAX_ITERATIONS);
    assert_eq!(out.text.len(), 1 + MAX_ITERATIONS);
}

#[test]
fn a_round_that_breaks_the_file_is_undone() {
    let analyze = |text: &str| -> Result<Analysis, ()> {
        let diagnostics = if text == "keep" {
            vec![diagnostic(
                "breaker",
                Category::Style,
                replace("delete everything", FixKind::Safe, 0, 4, ""),
            )]
        } else {
            Vec::new()
        };
        Ok(Analysis {
            diagnostics,
            parses: !text.is_empty(),
        })
    };
    let out = fix_loop("keep".into(), |_| true, analyze).unwrap();
    assert_eq!(out.text, "keep");
    assert_eq!(out.broke, Some(vec!["breaker".to_string()]));
}

#[test]
fn a_file_that_does_not_parse_is_left_alone() {
    let out = fix_loop(String::new(), |_| true, analyze_a).unwrap();
    assert_eq!(out.text, "");
    assert_eq!(out.iterations, 0);
}

//! Rules the fixtures cannot express.

use lang_css::{CssProvider, Dialect};
use provider_api::testing::{lint, render};

fn report(dialect: Dialect, path: &str, text: &str, rules: &str) -> String {
    render(text, &lint(&CssProvider::new(dialect), path, text, rules))
}

#[test]
fn an_empty_file_is_reported_but_a_comment_is_not() {
    let rules = "no-empty-source: warn";
    assert!(report(Dialect::Css, "a.css", " \n", rules).contains("no-empty-source"));
    assert_eq!(report(Dialect::Css, "a.css", "/* c */\n", rules), "");
}

#[test]
fn the_rules_serve_scss_and_less_too() {
    let rules = "block-no-empty: warn";
    assert!(report(Dialect::Scss, "a.scss", "a {}\n", rules).contains("block-no-empty"));
    assert!(report(Dialect::Less, "a.less", "a {}\n", rules).contains("block-no-empty"));
}

#[test]
fn scss_and_less_constructs_are_not_unknown() {
    let rules = "at-rule-no-unknown: warn\nfunction-no-unknown: warn\nproperty-no-unknown: warn\n\
                 selector-type-no-unknown: warn\nselector-pseudo-class-no-unknown: warn\nunit-no-unknown: warn";
    let scss = "@use \"a\";\n@mixin m($a) { color: darken($a, 10%) }\n\
                .a { @include m(red); &-b { color: red } &:hover { colour: y } #{$p}-c: 1; $v: 1 }\n\
                .x { @if $a { color: red } @else { color: blue } }\n%ph { color: red }\n\
                @function f() { @return 1 }\n@foo {}\n";
    let found = report(Dialect::Scss, "a.scss", scss, rules);
    assert_eq!(found.matches("-no-unknown").count(), 2, "{found}");
    let less = "@v: 1px;\n@plugin \"p\";\n.m(@a: red) { color: @a }\n\
                .b { .m(); .m(blue); @detached(); width: ~\"calc(1px)\"; &:extend(.a); @r: { color: red } }\n\
                .c when (default()) { color: red }\n@media (min-width: @v) { a { color: red } }\n";
    let found = report(Dialect::Less, "a.less", less, rules);
    assert_eq!(found, "", "{found}");
}

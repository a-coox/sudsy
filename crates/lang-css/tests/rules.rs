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

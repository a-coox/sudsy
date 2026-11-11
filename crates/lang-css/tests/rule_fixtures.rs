//! `tests/rules/css/<rule>/*`: see docs/development/testing.md.

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures(&lang_css::CssProvider::new(lang_css::Dialect::Css));
}

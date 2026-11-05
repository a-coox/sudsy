//! `tests/rules/editorconfig/<rule>/*`: see docs/development/testing.md.

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures(&lang_editorconfig::provider());
}

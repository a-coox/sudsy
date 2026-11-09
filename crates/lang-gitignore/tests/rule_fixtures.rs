//! `tests/rules/gitignore/<rule>/*`: see docs/development/testing.md.

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures(&lang_gitignore::provider());
}

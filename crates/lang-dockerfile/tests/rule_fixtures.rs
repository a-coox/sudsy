//! `tests/rules/dockerfile/<rule>/*`: see docs/development/testing.md.

#[test]
fn rule_fixtures() {
    let shell: &'static _ = Box::leak(Box::new(lang_shell::provider()));
    provider_api::fixtures::rule_fixtures_with(&lang_dockerfile::provider(), &[shell]);
}

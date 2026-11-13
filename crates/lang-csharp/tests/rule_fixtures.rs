//! `tests/rules/csharp/<rule>/*` through the real worker: see docs/development/testing.md.

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures(&lang_csharp::provider(&crate::common::worker_dll()));
}

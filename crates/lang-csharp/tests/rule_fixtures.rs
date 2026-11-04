//! `tests/rules/csharp/<rule>/*` through the real worker: see docs/development/testing.md.

mod common;

#[test]
fn rule_fixtures() {
    provider_api::fixtures::rule_fixtures(&lang_csharp::provider(&common::worker_dll()));
}

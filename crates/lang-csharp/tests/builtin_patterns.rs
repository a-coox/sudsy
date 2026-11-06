//! The built-in pattern rules (`crates/lang-csharp/rules/*.yaml`): listed without the worker,
//! compiled when first used.

mod common;

use lang_csharp::{provider, provider_with_command};
use provider_api::LanguageProvider;

#[test]
fn listing_the_rules_does_not_start_the_worker() {
    let p = provider_with_command(vec!["no-such-host".into()]);
    for rule in p.rules() {
        let _ = (rule.meta().id, rule.meta().options, rule.layout_role());
    }
    assert!(p.rules().iter().any(|r| r.meta().id == "no-regions"));
    assert!(!p.proxy().started());
}

#[test]
fn every_embedded_pattern_compiles() {
    let p = provider(&common::worker_dll());
    // `kinds()` compiles a pattern rule; a broken one is unavailable.
    for rule in p.rules() {
        let _ = rule.kinds();
        assert_eq!(rule.unavailable(), None, "{}", rule.meta().id);
    }
}

/// A built-in rule with a `pattern:` parses snippets on first use: with the worker down it is
/// skipped and says why, instead of panicking.
#[test]
fn a_rule_that_cannot_compile_is_skipped_not_a_panic() {
    const SOURCE: &str = "id: needs-worker
language: csharp
message: m
description: d
example: e
severity: warn
category: style
rule:
  pattern: Foo($A)
";
    let p = provider_with_command(vec!["no-such-host".into()]).with_pattern_rules(&[SOURCE]);
    let rule = p
        .rules()
        .iter()
        .find(|r| r.meta().id == "needs-worker")
        .unwrap();
    assert!(rule.kinds().is_empty());
    assert!(rule.unavailable().unwrap().contains("needs-worker"));
}

/// `cargo test -p lang-csharp --release --test builtin_patterns -- --ignored --nocapture`:
/// what building the provider (the embedded files read, no pattern compiled) and compiling
/// every embedded pattern cost.
#[test]
#[ignore = "a measurement"]
fn measure_startup_cost() {
    let command = || vec!["no-such-host".into()];
    let n = 200;
    let start = std::time::Instant::now();
    for _ in 0..n {
        std::hint::black_box(provider_with_command(command()).rules());
    }
    println!(
        "provider with pattern rules: {:?} each",
        start.elapsed() / n
    );
    let p = provider(&common::worker_dll());
    let start = std::time::Instant::now();
    for rule in p.rules() {
        let _ = rule.kinds();
    }
    println!(
        "first use of all rules (compiles the patterns): {:?}",
        start.elapsed()
    );
}

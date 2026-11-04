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
    // `kinds()` compiles a pattern rule and panics naming the rule when it is broken.
    for rule in p.rules() {
        let _ = rule.kinds();
    }
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
        std::hint::black_box(provider_with_command(command()));
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

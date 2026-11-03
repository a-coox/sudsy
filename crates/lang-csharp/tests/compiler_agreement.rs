//! The flow rules report what the compiler reports (plan.md M5 gate: CS0162, CS0168, CS0219).
//! `tests/data/flow-agreement.cs` has the constructs; the corpus run in
//! `perf-bench semantic` checks the same on real code.

mod common;

use lang_csharp::provider;
use provider_api::testing::lint;
use provider_api::{LanguageProvider, SourceFile};
use std::collections::BTreeSet;

const RULES: &str = "unreachable-code:\n  level: warn\n  options:\n    non_returning_calls: false\nunused-variable: warn";

#[test]
fn flow_rules_agree_with_the_compiler_on_the_fixture() {
    let path = common::repo_root().join("crates/lang-csharp/tests/data/flow-agreement.cs");
    let text = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    let provider = provider(&common::worker_dll());
    let ours: BTreeSet<(String, u32)> = lint(&provider, "flow-agreement.cs", &text, RULES)
        .iter()
        .filter_map(|d| {
            let id = match d.rule.as_str() {
                "unreachable-code" => "CS0162",
                "unused-variable" if d.message.contains("declared but") => "CS0168",
                "unused-variable" => "CS0219",
                _ => return None,
            };
            Some((id.to_string(), u32::from(d.range.start())))
        })
        .collect();
    let compiler: BTreeSet<(String, u32)> = provider
        .compiler_diagnostics(
            &SourceFile::new("flow-agreement.cs", 1, text.clone()),
            &["CS0162", "CS0168", "CS0219"],
        )
        .unwrap()
        .into_iter()
        .map(|d| (d.id, d.start))
        .collect();
    let at = |o: u32| {
        let line = text[..o as usize].matches('\n').count() + 1;
        format!(
            "line {line}: {}",
            text.lines().nth(line - 1).unwrap().trim()
        )
    };
    let show = |set: &BTreeSet<(String, u32)>| -> Vec<String> {
        set.iter()
            .map(|(id, o)| format!("{id} {}", at(*o)))
            .collect()
    };
    let only_ours: BTreeSet<_> = ours.difference(&compiler).cloned().collect();
    let only_compiler: BTreeSet<_> = compiler.difference(&ours).cloned().collect();
    assert!(
        !compiler.is_empty(),
        "the fixture must make the compiler warn"
    );
    assert!(
        only_ours.is_empty() && only_compiler.is_empty(),
        "ours only: {:#?}\ncompiler only: {:#?}",
        show(&only_ours),
        show(&only_compiler)
    );
}

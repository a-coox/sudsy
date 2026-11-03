//! The rules over the flow, binding and value layers, against the real worker.

mod common;

use insta::assert_snapshot;
use lang_csharp::provider;
use provider_api::OutOfProcessProvider;
use provider_api::testing::{fix, lint, render};

fn csharp() -> OutOfProcessProvider {
    provider(&common::worker_dll())
}

fn report(text: &str, rules: &str) -> String {
    render(text, &lint(&csharp(), "a.cs", text, rules))
}

const FLOW: &str = "using System;
class A {
  int F(int a) {
    int unused;
    int k = 5;
    int z = G();
    try { G(); } catch (Exception e) { }
    if (false) { G(); }
    while (true) { if (a > 0) break; }
    return z;
    G();
    G();
  }
  int G() => 1;
  void Quit() { Environment.Exit(1); G(); }
  void Lambda() { int c = 1; Action f = () => Console.WriteLine(c); f(); }
}
";

#[test]
fn flow_rules_report_what_the_compiler_reports() {
    assert_snapshot!(report(FLOW, ""));
}

#[test]
fn constant_conditions_and_jumps_both_make_code_unreachable() {
    let text = "class A { int F() { return 1; G(); G(); }
 int H(int x) { if (false) { G(); } throw null; x++; } int G() => 1; }";
    assert_snapshot!(report(text, ""));
}

#[test]
fn dead_stores_are_found_with_liveness() {
    let text = "class A { int F() { int n = 0; n = G(); return n; } int H() { int m = 0; m = 1; m = G(); return m; } int G() => 1; }";
    assert_snapshot!(report(text, "unused-assignment: warn"));
}

#[test]
fn unused_variable_fix_removes_the_declaration_and_the_catch_name() {
    let text = "class A { void F() {\n    int k = 5;\n    int a = 1, b = 2;\n    try { F(); } catch (System.Exception e) { }\n    System.Console.WriteLine(b + a);\n} }\n";
    assert_snapshot!(fix(&csharp(), "a.cs", text, "", false));
}

#[test]
fn summaries_from_the_attribute_make_code_after_the_call_unreachable() {
    let text = "using System.Diagnostics.CodeAnalysis;\nclass A { [DoesNotReturn] static void Fail() => throw null; void F() { Fail(); F(); } }";
    assert_snapshot!(report(text, ""));
}

#[test]
fn unused_usings_need_a_clean_binding() {
    let clean = "using System;\nusing System.Text;\nclass A { Exception e; }\n";
    assert_snapshot!(report(clean, "remove-unused-usings: warn"));
    assert_snapshot!(fix(
        &csharp(),
        "a.cs",
        clean,
        "remove-unused-usings: warn",
        false
    ));
    let broken = "using System.Text;\nclass A { Missing m; }\n";
    assert_eq!(report(broken, "remove-unused-usings: warn"), "");
}

#[test]
fn nullable_analysis_answers_the_dereference_rule() {
    let text = "#nullable enable\nclass A { string? s; int N() => s.Length; int M() => s!.Length; int P() { if (s != null) return s.Length; return 0; } }\n";
    assert_snapshot!(report(text, "possible-null-dereference: warn"));
}

#[test]
fn a_body_without_a_flow_graph_falls_back_to_the_syntactic_check() {
    // `Missing` does not resolve, so Roslyn builds no graph for the body; a guard `throw` or an
    // empty statement after a jump is not reported, a call is.
    let text = "class A { int F() { Missing(); return 1; throw null; } int G() { Missing(); return 1; ; } int H() { Missing(); return 1; Missing(); } }";
    assert_snapshot!(report(text, ""));
}

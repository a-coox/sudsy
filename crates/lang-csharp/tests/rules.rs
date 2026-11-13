//! C# rules: expected diagnostics (snapshots) and fix output, through the real worker.

use insta::assert_snapshot;
use lang_csharp::provider;
use provider_api::OutOfProcessProvider;
use provider_api::testing::{fix, lint, render};

fn csharp() -> OutOfProcessProvider {
    provider(&crate::common::worker_dll())
}

fn report(text: &str, rules: &str) -> String {
    render(text, &lint(&csharp(), "a.cs", text, rules))
}

fn fixed(text: &str, rules: &str, allow_unsafe: bool) -> String {
    fix(&csharp(), "a.cs", text, rules, allow_unsafe)
}

#[test]
fn using_order_sorts_plain_usings_and_leaves_the_rest() {
    let text = "using Zeta;\nusing System.Linq; // linq\nusing static Foo.Bar;\nusing Alpha;\nusing System;\nusing X = Y.Z;\n\nclass A { }\n";
    let rules = "using-order: warn";
    assert_snapshot!(report(text, rules));
    assert_snapshot!(fixed(text, rules, false));
    assert_eq!(report(&fixed(text, rules, false), rules), "");
    let no_system_first = "using-order:\n  level: warn\n  options:\n    system_first: false";
    assert_snapshot!(fixed(text, no_system_first, false));
}

#[test]
fn using_order_follows_the_options() {
    let text = "using Zeta;
using static Foo.Bar;
using Alpha;
using X = Y.Z;
using System;
using static System.Math;
using Aa;
using aa;
using W = Y.Z;

class A { }
";
    let rules = "using-order:
  level: warn
  options:
    compare: culture
    static_position: after
    alias_position: after";
    assert_snapshot!(report(text, rules));
    let sorted = fixed(text, rules, false);
    assert_snapshot!(sorted);
    assert_eq!(report(&sorted, rules), "");
}

const LEXICAL: &str = "comment-space: warn\nno-empty-comment: warn\ndoc-comment-space: warn\ndirective-keyword-space: warn\nno-regions: warn\nno-empty-statement: warn\nno-comment-before-block: warn\nuse-string-empty: warn\nseparate-attributes: warn\nno-empty-attribute-parens: warn\nno-empty-delegate-parens: warn";

#[test]
fn structure_rules_follow_stylecop() {
    let text = "using System.Diagnostics.CodeAnalysis;\n[assembly: SuppressMessage(\"C\", \"I\")]\n[assembly: SuppressMessage(\"C\", \"I\", Justification = \"<Pending>\")]\nnamespace N { class Other { void M() {\n#region R\n#endregion\n} } class B { } struct S { } namespace M { } }\n";
    let rules = "no-region-in-body: warn\nno-regions: warn\nsingle-class-per-file: warn\nsingle-namespace-per-file: warn\nsuppression-justification: warn\nfile-name-matches-type: warn";
    assert_snapshot!(report(text, rules));
}

#[test]
fn lexical_rules_are_off_by_default() {
    let text = "class A { void F() { ; } //x\n}\n";
    assert_eq!(report(text, ""), "");
    assert!(!report(text, LEXICAL).is_empty());
}

#[test]
fn member_order_reports_like_stylecop() {
    let text = "class A {
 void M() { }
 private int b;
 public int a;
 const int C = 1;
 static int s;
 int i;
 readonly int r;
 static readonly int sr;
 public A() { }
}
class B { }
enum E { }
";
    assert_snapshot!(report(text, "member-order: warn"));
    let access_only = "member-order:
  level: warn
  options:
    element_order: [accessibility]";
    assert_snapshot!(report(text, access_only));
}

//! The rules over symbols and types (`symbol_findings`), against the real worker.

mod common;

use insta::assert_snapshot;
use lang_csharp::provider;
use provider_api::OutOfProcessProvider;
use provider_api::testing::{fix, lint, render};

fn csharp() -> OutOfProcessProvider {
    provider(&common::worker_dll())
}

const RULES: &str = "no-unneeded-base-prefix: warn
no-default-value-type-constructor: warn
qualify-namespace-usings: warn
use-tuple-syntax: warn
tuple-element-names-in-signatures: warn
debug-assert-message: warn
debug-fail-message: warn
inheritdoc-on-inheriting-member: warn";

const SAMPLE: &str = "using System;
using System.Diagnostics;
namespace A.B {
  using C.D;
  struct S { }
  interface I { (int, int) Pair(); }
  class P { public virtual void Run() { } }
  class Q : P, I {
    public (int, int) Pair() => (1, 2);
    public (int, int) Other() => (1, 2);
    public void Go(ValueTuple<int, int> t) {
      base.Run();
      var s = new S();
      var d = new DateTime();
      var g = ValueTuple.Create(1, 2);
      Debug.Assert(true);
      Debug.Assert(true, \"why\");
      Debug.Fail(\"\");
    }
    /// <inheritdoc/>
    public void Own() { }
  }
}
namespace C.D { }
namespace A.C.D { }
";

#[test]
fn symbol_rules_use_the_type_layer() {
    let text = SAMPLE;
    assert_snapshot!(render(text, &lint(&csharp(), "a.cs", text, RULES)));
}

#[test]
fn symbol_rule_fixes() {
    assert_snapshot!(fix(&csharp(), "a.cs", SAMPLE, RULES, false));
}

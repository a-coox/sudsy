//! C# rules: expected diagnostics (snapshots) and fix output, through the real worker.

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

fn fixed(text: &str, rules: &str, allow_unsafe: bool) -> String {
    fix(&csharp(), "a.cs", text, rules, allow_unsafe)
}

#[test]
fn empty_catch_needs_neither_statements_nor_comments() {
    let text = "class A { void F() {\n try { } catch (Exception) { }\n try { } catch { /* ignored on purpose */ }\n try { } catch { Log(); }\n} }\n";
    assert_snapshot!(report(text, ""));
    assert_eq!(report(text, "no-empty-catch: \"off\""), "");
}

#[test]
fn async_suffix_applies_to_task_returning_methods_only() {
    let text = "class A {\n async Task A1() { }\n async Task<int> B1() { }\n async ValueTask C1() { }\n async System.Threading.Tasks.Task D1() { }\n async void E1() { }\n async Task OkAsync() { }\n public override async Task F1() { }\n async Task I.G1() { }\n async Task<int> Main() { }\n Task H1() { }\n}\n";
    assert_snapshot!(report(text, ""));
}

#[test]
fn async_suffix_asks_the_binding_layer() {
    // The `Task` here is the file's own type, and `Run` implements an interface member whose name
    // the class does not choose.
    let text = "using System.Threading.Tasks;
interface I { Task Run(); }
class A : I {
 public async Task Run() { }
 public async Task Other() { }
}
class B {
 class Task { }
 async Task Mine() { }
}
";
    assert_snapshot!(report(text, ""));
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
fn using_order_skips_lists_with_directives() {
    let text = "using Zeta;\n#if X\nusing Alpha;\n#endif\nclass A { }\n";
    assert_eq!(report(text, "using-order: warn"), "");
}

#[test]
fn suppression_comment() {
    let text = "class A { void F() {\n // sudsy-disable-next-line no-empty-catch\n try { } catch { }\n try { } catch { }\n} }\n";
    assert_snapshot!(report(text, ""));
}

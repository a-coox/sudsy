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

const LEXICAL: &str = "comment-space: warn\nno-empty-comment: warn\ndoc-comment-space: warn\ndirective-keyword-space: warn\nno-regions: warn\nno-empty-statement: warn\nno-comment-before-block: warn\nuse-string-empty: warn\nseparate-attributes: warn\nno-empty-attribute-parens: warn\nno-empty-delegate-parens: warn";

#[test]
fn comment_rules_follow_stylecop() {
    let text = "class A {\n //no space\n //\n // paragraph\n //\n // text\n //\n int a; //tail\n /// <summary>x</summary>\n ///<summary>y</summary>\n ///   <summary>z</summary>\n /* */\n int b;\n}\n";
    let rules = "comment-space: warn\nno-empty-comment: warn\ndoc-comment-space: warn";
    assert_snapshot!(report(text, rules));
    assert_snapshot!(fixed(text, rules, false));
}

#[test]
fn directives_and_regions() {
    let text = "class A {\n# region R\n int a;\n#  endregion\n# if X\n int b;\n#endif\n}\n";
    let rules = "directive-keyword-space: warn\nno-regions: warn";
    assert_snapshot!(report(text, rules));
    assert_snapshot!(fixed(text, rules, false));
}

#[test]
fn empty_statements_and_comments_before_blocks() {
    let text = "class A { void F(bool x, int y) {\n ;\n if (x); else { }\n while (x) /* c */ { }\n switch (y) { case 1: ; break; default: ; }\n L: ;\n int z = 1;;\n} }\n";
    let rules = "no-empty-statement: warn\nno-comment-before-block: warn";
    assert_snapshot!(report(text, rules));
    assert_snapshot!(fixed(text, rules, false));
}

#[test]
fn string_empty_skips_constant_positions() {
    let text = "class A {\n const string C = \"\";\n string F(string s = \"\") {\n  if (s is \"\") { }\n  switch (s) { case \"\": break; }\n  return s ?? @\"\";\n }\n [System.Obsolete(\"\")] void G() { }\n}\n";
    assert_snapshot!(report(text, "use-string-empty: warn"));
    assert_snapshot!(fixed(text, "use-string-empty: warn", false));
}

#[test]
fn attribute_forms() {
    let text = "[assembly: A, B]\nclass C {\n [A(), B]\n void F([A, B] int p) { }\n [A, // why\n  B]\n int x;\n Action a = delegate() { };\n}\n";
    let rules = "separate-attributes: warn\nno-empty-attribute-parens: warn\nno-empty-delegate-parens: warn";
    assert_snapshot!(report(text, rules));
    assert_snapshot!(fixed(text, rules, false));
    assert_snapshot!(fixed(text, rules, true));
}

const SYNTAX: &str = "constant-on-right: warn\nnullable-shorthand: warn\nseparate-field-declarations: warn\nuse-lambda: warn\nuse-literal-suffix: warn";

#[test]
fn syntax_rules_report_and_fix() {
    let text = "class A {\n int a, b;\n Nullable<int> n = (long)1 < 2 ? null : null;\n void F(object o, int i) {\n  var f = delegate (int x) { return x; };\n  var g = null == o || 1 < i;\n  var h = (float)1 + (double)2.5;\n }\n}\n";
    assert_snapshot!(report(text, SYNTAX));
    let fixed = fixed(text, SYNTAX, false);
    assert_snapshot!(fixed);
    assert_eq!(report(&fixed, SYNTAX), "");
}

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

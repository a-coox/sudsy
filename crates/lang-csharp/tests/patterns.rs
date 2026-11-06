//! Pattern rules on C#: snippets parsed by the real worker, matched against the shared tree.

mod common;

use core_rules::Extensions;
use lang_csharp::provider;
use provider_api::OutOfProcessProvider;
use provider_api::testing::{expect, fix_with, lint_with, pattern_rules};

fn csharp() -> OutOfProcessProvider {
    provider(&common::worker_dll())
}

fn rules(source: &str) -> Extensions {
    pattern_rules(&csharp(), source)
}

const CONSOLE: &str = r#"
id: no-console-writeline
language: csharp
message: Log with the logger, not Console.WriteLine
severity: warn
category: suspicious
rule:
  pattern: Console.WriteLine($$$ARGS)
  not-inside:
    kind: MethodDeclaration
    has:
      pattern: Main
fix: _logger.LogInformation($$$ARGS)
fix-kind: unsafe
"#;

#[test]
fn an_expression_pattern_matches_calls_with_any_arguments() {
    let p = csharp();
    let ext = rules(CONSOLE);
    let text = expect(
        &p,
        &ext,
        "a.cs",
        r#"class A
{
    void F(int x)
    {
        Console.WriteLine("hi {0}", x);
//      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-console-writeline
        Console.WriteLine();
//      ^^^^^^^^^^^^^^^^^^^ no-console-writeline
        Console.Write("not this");
        var s = string.Format("{0}", x);
    }
}
"#,
        "",
    );
    assert!(text.contains("Console.Write(\"not this\")"));
}

#[test]
fn the_fix_substitutes_the_captured_arguments_and_goes_through_the_fix_loop() {
    let p = csharp();
    let ext = rules(CONSOLE);
    let text = "class A { void F(int x) { Console.WriteLine(\"hi\", x); Console.WriteLine(); } }\n";
    // Unsafe fixes are not applied by plain `fix`.
    assert_eq!(fix_with(&p, &ext, "a.cs", text, "", false), text);
    assert_eq!(
        fix_with(&p, &ext, "a.cs", text, "", true),
        "class A { void F(int x) { _logger.LogInformation(\"hi\", x); _logger.LogInformation(); } }\n"
    );
}

#[test]
fn statement_patterns_use_a_semicolon_after_a_multiple_metavariable() {
    let p = csharp();
    let ext = rules(
        r#"
id: empty-then
language: csharp
message: "if ($C) has an empty body"
rule:
  pattern: "if ($C) { }"
fix: "$C;"
fix-kind: suggestion
"#,
    );
    let text = "class A { void F(bool b, int x) { if (b) { } if (x > 1) { x++; } } }\n";
    let found = lint_with(&p, &ext, "a.cs", text, "");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].message, "if (b) has an empty body");
    assert_eq!(found[0].fixes[0].kind, core_rules::FixKind::Suggestion);
}

#[test]
fn inside_has_and_alternatives() {
    let p = csharp();
    let ext = rules(
        r#"
id: datetime-now
language: csharp
message: "DateTime.Now in a method of a *Service class"
rule:
  any:
    - pattern: DateTime.Now
    - pattern: DateTime.UtcNow
  inside:
    kind: ClassDeclaration
    regex: 'class \w+Service'
"#,
    );
    let text = "class OrderService { DateTime F() => DateTime.Now; DateTime G() => DateTime.UtcNow; }\nclass Other { DateTime F() => DateTime.Now; }\n";
    let found = lint_with(&p, &ext, "a.cs", text, "");
    assert_eq!(found.len(), 2);
}

#[test]
fn a_suppression_comment_silences_a_pattern_rule_like_any_other() {
    let p = csharp();
    let ext = rules(CONSOLE);
    let text = "class A { void F() {\n // sudsy-disable-next-line no-console-writeline\n Console.WriteLine();\n Console.WriteLine();\n} }\n";
    assert_eq!(lint_with(&p, &ext, "a.cs", text, "").len(), 1);
}

const OPTIONS: &str = r#"
id: banned-call
language: csharp
message: "$CALL is not allowed"
severity: warn
options:
  - { name: banned, type: list, default: [Thread.Sleep], description: Calls that are reported }
rule:
  pattern: $CALL($$$ARGS)
constraints:
  CALL: { in-option: banned }
"#;

#[test]
fn options_are_declared_and_used_by_constraints() {
    let p = csharp();
    let ext = rules(OPTIONS);
    let names: Vec<_> = ext.rules[0]
        .rule
        .meta()
        .options
        .iter()
        .map(|o| o.name)
        .collect();
    assert_eq!(names, ["banned"]);
    let text = "class A { void F() { Thread.Sleep(1); Task.Delay(1); } }\n";
    let found = lint_with(&p, &ext, "a.cs", text, "banned-call: warn");
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].message, "Thread.Sleep is not allowed");
    let configured = "banned-call:\n  level: warn\n  options:\n    banned: [Task.Delay]";
    let found = lint_with(&p, &ext, "a.cs", text, configured);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].message, "Task.Delay is not allowed");
}

#[test]
fn a_regex_option_constraint_matches_what_the_option_says() {
    let p = csharp();
    let ext = rules(
        r#"
id: call-by-regex
language: csharp
message: "$CALL"
options:
  - { name: names, type: regex, default: '^Thread\.', description: Calls that are reported }
rule:
  pattern: $CALL($$$ARGS)
constraints:
  CALL: { regex-option: names }
"#,
    );
    let text = "class A { void F() { Thread.Sleep(1); Task.Delay(1); } }\n";
    assert_eq!(
        lint_with(&p, &ext, "a.cs", text, "call-by-regex: warn").len(),
        1
    );
    let configured = "call-by-regex:\n  level: warn\n  options:\n    names: '^Task'";
    assert_eq!(lint_with(&p, &ext, "a.cs", text, configured).len(), 1);
}

#[test]
fn option_declarations_and_uses_are_checked() {
    let head = "id: x\nlanguage: csharp\nmessage: m\nrule: { pattern: 'F($N)' }\n";
    for bad in [
        "options:\n  - { name: a, type: bool, default: 1, description: d }\n",
        "options:\n  - { name: a, type: regex, default: 'x', choices: [x], description: d }\n",
        "constraints:\n  N: { in-option: nope }\n",
        "constraints:\n  N: { regex: '\\w{1000}\\w{1000}\\w{1000}' }\n",
        "options:\n  - { name: a, type: bool, default: true, description: d }\nconstraints:\n  N: { in-option: a }\n",
    ] {
        let source = format!("{head}{bad}");
        assert!(
            provider_api::testing::pattern_error(&csharp(), &source).is_some(),
            "{bad}"
        );
    }
}

#[test]
fn report_on_and_fix_on_select_the_span_and_the_edit() {
    let p = csharp();
    let ext = rules(
        r#"
id: second-arg
language: csharp
message: "the second argument is always 0"
rule:
  pattern: Log($A, $B)
report-on: $B
fix: "0"
fix-on: $B
fix-kind: safe
"#,
    );
    let text = "class A { void F() { Log(1, 2); } }\n";
    let found = lint_with(&p, &ext, "a.cs", text, "");
    assert_eq!(found.len(), 1);
    assert_eq!(
        usize::from(found[0].range.start()),
        text.find("2)").unwrap()
    );
    assert_eq!(usize::from(found[0].range.len()), 1);
    assert_eq!(
        fix_with(&p, &ext, "a.cs", text, "", false),
        "class A { void F() { Log(1, 0); } }\n"
    );
}

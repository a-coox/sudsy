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

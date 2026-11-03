# Pattern rules (`core-patterns`)

ast-grep-style YAML rule files; listed by `sudsy rules`, configured under `lint.rules`, suppressed, fixed and shown in the editor like any built-in rule. Why YAML: C# teams write everyday rules without a Rust build; the matcher is shared with `plugin-sdk`.

```yaml
# sudsy.yaml
plugins:
  patterns: ["rules/*.yaml"]      # globs relative to this file
```

A rule file holds one rule, or several separated by `---`:

```yaml
id: no-console-writeline            # plain id, or team/no-console-writeline
language: csharp                    # csharp | json | yaml
message: Log with the logger, not Console.WriteLine($$$ARGS)
severity: warn                      # off | warn | error: the level when the config is silent (default warn)
category: suspicious                # correctness | suspicious | style (default) | cleanup | performance
rule:
  pattern: Console.WriteLine($$$ARGS)
  not-inside:
    kind: MethodDeclaration
    has: { pattern: Main }
constraints:
  ARGS: { regex: '^"' }             # regex | not-regex | kind, on what a metavariable captured
fix: _logger.LogInformation($$$ARGS)
fix-title: Use the logger           # optional
fix-kind: unsafe                    # safe | unsafe (default: a template is not known to preserve meaning) | suggestion
```
Also `description` and `example`. Unknown keys are errors.

## Matching
- A pattern is a snippet of the language, parsed by its own provider. Metavariables are first written as identifiers (`__SUDSY_S_X__`, `__SUDSY_M_X__`), or as strings when that does not parse (JSON). The snippet is tried as is, with a `;` after it, and inside `{ }` (a JSON member); the node spanning all of it is the pattern.
- `$X` matches one node or token; `$$$ARGS` any run of siblings (also none, separators included, by backtracking); the same name twice must capture equal text; `$_X` captures nothing. In statement position write `$$$BODY;`.
- `rule` keys: `pattern`, `kind` (node kinds of the language's grammar), `regex` (node text without trivia), `inside`, `not-inside`, `has`, `not-has` (nested rules), `all`, `any`, `not`. Keys at one level all must hold. `inside`/`has` drop their captures.
- A rule must pin where it starts: a `pattern` or `kind` at the top (or in every `any` branch). `kinds()` is that root kind, so the engine calls the rule for those kinds only. A rule that pins none (regex or relation alone) is a config error: it would cost 1.6 ms per file per rule.
- `message` and `fix` substitute captured source text. The fix replaces the matched node; fix loop and formatter do the rest.
- A pattern rule has no options. `type_matches` is not available (see roadmap).

## Loading
`core-patterns` compiles a file into `PatternRule`, a plain `core_rules::Rule`. Patterns compile when the config loads (a C# pattern file starts the worker then); a bad pattern is a config error naming its file. Ids are used as written; an id equal to a built-in's or defined twice for a language is a config error. Rules are leaked once per distinct file text (`Rule::meta` returns `&'static`), so an unchanged reload costs nothing. Fixtures: `provider_api::testing::expect` / `fix_with`.

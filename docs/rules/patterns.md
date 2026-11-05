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

## Span, fix target, options
```yaml
id: no-empty-attribute-parens
language: csharp
message: the attribute has empty parentheses
rule:
  kind: AttributeArgumentList
  regex: '^\(\s*\)$'
report-on: first-token           # the diagnostic points at the `(`
fix: ""
fix-trim-before: true            # the fix also takes the spaces before the parentheses
fix-kind: safe
```
- `report-on` sets the diagnostic span: `$NAME` (what the metavariable captured), `first-token` (the first token of the matched node) or `first-token $NAME`. Default: the matched node.
- `fix-on: $NAME` makes the fix replace what the metavariable captured instead of the matched node; `fix-trim-before: true` extends the replaced range back over the spaces and tabs before it. `fix: ""` deletes. Both need a `fix`; `$NAME` must be captured by a pattern outside `inside`/`has`/`not`.
- `options` declares typed options with a default, validated and shown in the schema and `sudsy rules <id>` like a Rust rule's: `{ name, type, default, choices, description }`, `type` being `bool`, `int`, `string`, `list` (of strings) or `regex` (a string that must compile); `choices` limits a `string` or each item of a `list`. A user sets them under the rule in `lint.rules` (`options:`).
- A constraint can read an option: `in-option: NAME` (the captured text is an item of the `list` option, or equals the `string` option) and `regex-option: NAME` (it matches the `regex` option).

```yaml
options:
  - { name: banned, type: list, default: [Thread.Sleep], description: Calls that are reported }
rule: { pattern: $CALL($$$ARGS) }
constraints:
  CALL: { in-option: banned }
```

## Layout
`layout:` asks the formatter for a layout; each entry is `<directive> [argument] [on $NAME]`:

```yaml
# rules/service-chains.yaml: every call of a builder chain on `services` on its own line
id: services-chain
language: csharp
layout: [break-all]
rule:
  kind: InvocationExpression
  regex: '^services\s*\.'          # the node's text changes once it is broken: allow whitespace
---
id: items-one-per-line
language: json
layout:
  - break-all on $V                 # a metavariable: the node it captured
  - blank-line-before 1
rule:
  pattern: '"items": $V'
```
- Directives: `break-all`, `keep-together`, `break-before`, `break-after`, `blank-line-before N`, `align`, `preserve`, `style NAME` (what each does and where it is honored: [../formatting/engine.md](../formatting/engine.md)). The target is the matched node, or with `on $NAME` the single node that metavariable captured (a token or a run captures no node: nothing is attached). `$NAME` must be captured by a pattern outside `inside`/`has`/`not`.
- Without `message` the rule is layout-only: no diagnostic, not run when linting, `fix` not allowed; with `message` it reports and attaches. A layout-only rule has an id like any other: `sudsy rules` lists it, `lint.rules` sets its level (`off` attaches nothing), `sudsy-disable-next-line <id>` silences the node on the next line.
- Layout rules see syntax only. Two rules asking for conflicting layouts of one node (`break-all` + `keep-together`, ...) make that file fail to format, naming both. A bad directive is a config error naming the file.

## Matching
- Cost bound: a rule gets 1,000,000 match steps per node (its `inside`/`has` walks included), because several `$$$` in one pattern are O(n^k). Past that the rule reports "the pattern took too many steps" once and is off for the rest of that file.
- A pattern is a snippet of the language, parsed by its own provider. Metavariables are first written as identifiers (`__SUDSY_S_X__`, `__SUDSY_M_X__`), or as strings when that does not parse (JSON). The snippet is tried as is, with a `;` after it, and inside `{ }` (a JSON member); the node spanning all of it is the pattern.
- `$X` matches one node or token; `$$$ARGS` any run of siblings (also none, separators included, by backtracking); the same name twice must capture equal text; `$_X` captures nothing. In statement position write `$$$BODY;`.
- `rule` keys: `pattern`, `kind` (node kinds of the language's grammar), `regex` (node text without trivia), `inside`, `not-inside`, `has`, `not-has` (nested rules), `all`, `any`, `not`. Keys at one level all must hold. `inside`/`has` drop their captures.
- A rule must pin where it starts: a `pattern` or `kind` at the top (or in every `any` branch). `kinds()` is that root kind, so the engine calls the rule for those kinds only. A rule that pins none (regex or relation alone) is a config error: it would cost 1.6 ms per file per rule.
- `message` and `fix` substitute captured source text. The fix replaces the matched node; fix loop and formatter do the rest.
- `type_matches` is not available (see roadmap).

## Loading
`core-patterns` compiles a file into `PatternRule`, a plain `core_rules::Rule`. Patterns compile when the config loads (a C# pattern file starts the worker then); a bad pattern is a config error naming its file. Ids are used as written; an id equal to a built-in's or defined twice for a language is a config error. Rules are leaked once per distinct file text (`Rule::meta` returns `&'static`), so an unchanged reload costs nothing. Fixtures: `provider_api::testing::expect` / `fix_with`.

## Built-in pattern rules
A language crate ships rules as pattern files: `crates/lang-<language>/rules/*.yaml` (every language). Adding a file is the whole change: the crate's `build = "../../build/embed-rules.rs"` lists the directory for `include_str!` (`provider_api::pattern_files!()`), the provider hands the text to `provider_api::with_pattern_rules` with its Rust rules (`OutOfProcessProvider::with_pattern_rules`, `Spec::pattern_files`), and they are built-in rules in every respect (`sudsy rules`, the generated rule table and schema, levels, suppressions, fixes). They use the same format and features as a user's file; `severity` is the default level and a rule that is off by default says `severity: off`.

The YAML is read when the provider is built (metadata and options need nothing more); the patterns compile when the rule first runs, through the provider's worker, once per process. A run that never lints that language never starts its worker for them. If the compile fails then (the worker is down), the rule is skipped for that file, the failure is not remembered (the next file tries again) and it is reported once per run through the same warning as an unavailable semantic model (`Rule::unavailable`). A broken file is a bug: `every_embedded_pattern_compiles` (`crates/lang-csharp/tests/builtin_patterns.rs`) compiles all of them. A rule that needs semantics, flow, project scope or logic a pattern cannot say stays in Rust; the fixture folder under `tests/rules/<language>/<id>/` is the same either way.

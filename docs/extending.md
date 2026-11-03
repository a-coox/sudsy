# Extending Sudsy: pattern rules and Wasm plugins

Both add rules to the ones Sudsy ships. They are listed by `sudsy rules`, configured under
`lint.rules`, silenced with `sudsy-disable-next-line`, fixed by `sudsy fix` and shown in the editor
like any built-in rule.

```yaml
# sudsy.yaml
plugins:
  patterns: ["rules/*.yaml"]      # globs relative to this file
  wasm: ["rules/team.wasm"]
```

## Pattern rules

A rule file is YAML, one rule per document:

```yaml
id: no-console-writeline            # a plain id, or team/no-console-writeline
language: csharp                    # csharp | json | yaml
message: Log with the logger, not Console.WriteLine($$$ARGS)
severity: warn                      # off | warn | error: the level when the config is silent
category: suspicious                # correctness | suspicious | style | cleanup | performance
rule:
  pattern: Console.WriteLine($$$ARGS)
  not-inside:
    kind: MethodDeclaration
    has: { pattern: Main }
constraints:
  ARGS: { regex: '^"' }             # checks on what a metavariable captured
fix: _logger.LogInformation($$$ARGS)
fix-kind: unsafe                    # safe | unsafe (default) | suggestion
```

* A pattern is a snippet of the language, parsed by its own parser. `$X` matches one node or
  token, `$$$ARGS` any run of siblings (also none); the same name twice must match the same text;
  `$_X` captures nothing. In statement position write `$$$BODY;`. A snippet that is not a
  complete construct on its own is retried with a `;` after it, and inside `{ }` (a JSON member).
* `rule` holds `pattern`, `kind` (node kind names from `grammars/<language>.grammar`), `regex`
  (on the node's text), `inside`, `not-inside`, `has`, `not-has` (each a nested rule), and
  `all`, `any`, `not`. Keys at one level all must hold. A rule must say where it starts: a `pattern` or `kind`
  at the top (or in every branch of `any`), so that it runs on nodes of those kinds only.
* `message` and `fix` may use the captured metavariables. The fix replaces the matched node; the
  fix loop and the formatter do the rest. A rule that fixes without `fix-kind` is `unsafe`.
* Write the expected diagnostics of a rule in a fixture with `provider_api::testing::expect`
  (`// ^^^ rule-id` under the line), and fixes with `fix_with`.

## Wasm plugins (API v0, unstable)

A plugin is a Rust crate using `plugin-sdk`; the rule is the same `Rule` trait the built-in rules
implement. See `tests/plugins/sample` for a complete one.

```toml
[lib]
crate-type = ["cdylib"]
[dependencies]
plugin-sdk = { path = "..." }
[dev-dependencies]
plugin-sdk = { path = "...", features = ["testing"] }   # run rules natively with the harness
```

```rust
plugin_sdk::export_plugin!(name: "team", rules: [("csharp", &NoDirectDateTime)]);
```

Build with `cargo build --release --target wasm32-wasip2` (`rustup target add wasm32-wasip2`) and
list the `.wasm` file under `plugins: wasm:`. The rule is `team/no-direct-datetime`.

* Rules may use node, body (with the flow graph: `plugin_sdk::flow`), file and project scope.
* A plugin gets a fuel, time and memory limit per call and no files, network or clock. One that
  traps, runs out of a limit or reports a span outside the file is disabled for the run, with a
  diagnostic saying why.
* The plugin API changes without notice until v1. A plugin built for another API version, or
  against another grammar, is refused; rebuild it against the Sudsy version you run.
* The interface is `wit/rules.wit`; a plugin in another language needs only that file.

# Rule engine (`core-rules`)

Three rule sources (built-in Rust, pattern files, Wasm plugins) report into one diagnostics model and one fix engine.

## Rule trait
- Object-safe; rules are zero-sized statics (`&'static dyn Rule`), no state. Options are data: a rule declares `OptionDecl`s (name, typed default, description) in `RuleMeta`; `RuleOptions::resolve` checks names/types and the rule reads typed values from `ctx.options`. Rule options appear in the generated JSON Schema (one entry per rule id under `lint.rules`) and in `sudsy rules <id>`.
- `RuleMeta`: id, category (`correctness`, `suspicious`, `style`, `cleanup`, `performance`; also fix priority order), default level, fix kind, `requires` (layers needed) and `uses` (layers that improve it).
- Ids are short (`no-empty-catch`); the section or the file's language supplies the language. Shared rules (`max-line-length`, in `core_rules::shared`) apply to every language and no language rule may reuse their id. Plugin and pattern ids may carry a prefix (`team/x`).
- Providers hand rules over via `LanguageProvider::rules()` and `body_kinds()`.

## Scopes
- Node: `kinds()` + `check`. One traversal per file dispatches nodes to subscribed rules; files run in parallel.
- File: `check_file`.
- Body: `check_body`, once per node whose kind the provider lists in `body_kinds`; `BodyContext.flow` carries the flow graph.
- Project: `collect(file)` gathers string facts, `finish(project)` reports across files; runs only at `analysis: deep`. Project diagnostics ignore suppression comments; project options come from the first file of the language.

## Layout
A rule can also ask for layout: `Rule::layout_role()` is `None` (default), `Also` (reports and attaches) or `Only` (attaches, reports nothing, is not run when linting), and the rule calls `ctx.attach(node, Directive)` from `check`/`check_file` (ignored when linting). `format` runs the layout rules (`layout_role().attaches()`, at their configured level, syntax only) in a pass of their own through the same traversal (`core_rules::run_layout`), drops directives a suppression silences, and builds the `DirectiveTable`; two rules asking for conflicting layouts of one node is an error. See [../formatting/engine.md](../formatting/engine.md).

## Layers and depth
A rule is skipped (never failed) when the provider does not answer a required layer or the depth does not allow it. Depth `fast`: syntax only (no layers); `standard` (default): all layers except `project`; `deep`: all. Rules with `uses` degrade: `unreachable-code` reports the statement after a jump at `fast` and adds constant conditions and non-returning calls at `standard`, one rule with one code path per mode. `--analysis` overrides the config key.

## Levels and config
- `off`, `warn`, `error`. `RuleMeta::default_level` applies when config is silent: `error` for `no-duplicate-keys`, `warn` for correctness-ish rules, `off` for style choices and anything noisy on existing repos.
- Unknown rule ids are config errors with position and suggestion (`validate_rule_ids`, covers pattern ids). A rule named in the wrong language's section is ignored there. Invalid options are errors when the rule is activated, naming rule and source.
- A file with syntax errors reports `syntax-error` errors and no rule runs.

## Diagnostics and fixes
- Diagnostic: span, severity, message, rule id, related spans, fixes. Fix kinds: `Safe` (applied by `fix`), `Unsafe` (`--unsafe`, or editor quick fix), `Suggestion` (editor only).
- Fix loop: per round `select` takes for each diagnostic the first applicable fix in priority order (category, rule id, position) and drops a fix if any edit overlaps an accepted one (found again next round). Stops when nothing applies/changes or after 10 rounds. After each round the file is re-parsed; if it no longer parses the round is undone and the rules are named in a warning (`FixOutcome::broke`). `sudsy fix` then formats.
- Reordering fixes (`sorted-keys`, `using-order`) move text between the entries' source slots: own-line comments stay in place; trailing comments move with a YAML key or using directive; JSON comments stay in place.

## Suppressions
`sudsy-disable-next-line [rule[, rule]...]` in a line or block comment (marker from the grammar's `suppress` line). No rule names suppresses all on that line. A directive naming a rule that does not fire is not reported. Plain JSON: config-only (`overrides` with `"off"`).

## Test harness (`provider_api::testing`)
`lint`, `render`, `fix`, `lint_with`/`fix_with` (extension rules), `pattern_rules`, `pattern_error`, `expect`: fixtures with expected diagnostics written under the line (`// ^^^ rule-id`, `#` in YAML; carets give the span length). Fixes are checked to keep the file parseable and be idempotent. `plugin_sdk::testing` re-exports it.

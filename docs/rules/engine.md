# Rule engine (`core-rules`)

Three rule sources (built-in Rust, pattern files, Wasm plugins) report into one diagnostics model and one fix engine.

## Rule trait
- Object-safe; rules are zero-sized statics (`&'static dyn Rule`), no state. Options are data: a rule declares `OptionDecl`s (name, typed default, `Check`, description) in `RuleMeta`; a `Check` is `Any`, `OneOf(values)` (enum options; also the JSON Schema `enum`) or `Regex` (every string must compile); `RuleOptions::resolve` checks names, types and checks and the rule reads typed values from `ctx.options`. Rule options appear in the generated JSON Schema (one entry per rule id under `lint.rules`) and in `sudsy rules <id>`.
- A Rust rule is written with `core_rules::rule!`, which declares the struct, its `RuleMeta` and the `meta`/`kinds`/`tags` methods; the `check*` functions follow as ordinary items (`crates/lang-csharp/src/rules/directives.rs` is the smallest example, the macro's doc comment lists the fields). `requires`, `uses` and `options` default to empty, `fix` to none. `ctx.report_fix(range, message, (title, edits))` reports with a fix of the declared `fix` kind (`None` for the fix reports without one).
- Similar rules share Rust code as a family that a built-in YAML file instantiates with `params` ([patterns.md](patterns.md#families)).
- `RuleMeta`: id, category (`correctness`, `suspicious`, `style`, `cleanup`, `performance`; also fix priority order), default level, fix kind, `requires` (layers needed) and `uses` (layers that improve it).
- Ids are short (`no-empty-catch`); the section or the file's language supplies the language. Shared rules (`core_rules::shared`) apply to every language and no language rule may reuse their id: `max-line-length`, `comment-space`, `no-empty-comment`, and the rules on keys and sequences (`no-duplicate-keys`, `no-empty-keys`, `key-name-casing`, `sorted-keys`, `sorted-array-values`), which are written against the grammar tags `#mapping`, `#key-value` and `#sequence` and so run in every language that tags its grammar, with no code of its own. Plugin and pattern ids may carry a prefix (`team/x`).
- Providers hand rules over via `LanguageProvider::rules()` and `body_kinds()`.

## Scopes
- Node: `kinds()` and `tags()` + `check`. One traversal per file dispatches nodes to subscribed rules; files run in parallel. A rule subscribes by kind and/or by grammar tag (`Rule::tags`); the engine resolves each tag to the node kinds carrying it in the file's grammar (`Grammar::nodes_with_tag`, a table built when the grammar loads) before the traversal, so dispatch stays a lookup by kind.
- File: `check_file`.
- Body: `check_body`, once per node whose kind the provider lists in `body_kinds`; `BodyContext.flow` carries the flow graph.
- Project: `collect(file)` gathers string facts, `finish(project)` reports across files; runs only at `analysis: deep`. Project diagnostics ignore suppression comments; project options come from the first file of the language.

## Language hooks
`FileContext::hooks` is the language's `RuleHooks` (`LanguageProvider::rule_hooks`, default `DEFAULT_HOOKS`), what shared rules cannot know from the grammar: `key_name` (the identity of a key; the default is its text with one pair of matching quotes stripped; JSON decodes escapes, YAML uses the scalar's value, TOML decodes quoted parts and joins dotted ones with `.`), `key_parts` (the separately named parts of a key that is a path, TOML's `a.b`, which `key-name-casing` checks one by one; the default is the whole key), `merge_key` (a key that may repeat and merges mappings: `<<` in YAML, none by default; the `merge_keys` option of `no-duplicate-keys` applies only where there is one) `scalar` (a sequence item that is a string or a number, for `sorted-array-values`; items it does not answer are not sorted) and `inline_mapping_line` (a line that is a `key: value` entry with an unbreakable value, for `max-line-length`'s `allow_non_breakable_inline_mappings`; YAML only). The entries of a `#mapping` are its `#key-value` children and the key is their item labelled `key` (`Grammar::slot`). `sorted-keys` and `sorted-array-values` can be limited to part of a document with `path_patterns` (regexes on the key path `a.b[0].c`, `["odd key"]` for keys that are not identifiers, empty for the root; a TOML table counts as its header key); `sorted-keys` also takes `has_properties` (keys the mapping must have). A reordering fix moves an entry together with the comment after it in block style, and without it when entries are separated by a token (JSON, flow YAML), because the separator would end up inside the comment.

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
- Reordering fixes (`sorted-keys`, `sorted-array-values`, `using-order`, `accessor-order`) move text between the entries' source slots: own-line comments stay in place; trailing comments move with a block-style entry or a using directive; comments next to comma-separated entries (JSON, flow YAML) stay in place.

## Suppressions
Comments in the language's own comment syntax (line or block), markers from the grammar's `suppress` lines, so every language with comments has them with no code of its own. Each takes an optional rule list (`a, b` or spaces); no rule names means every rule:
- `sudsy-disable-next-line [rules]`: the next line; `sudsy-disable-line [rules]`: the line the comment ends on.
- `sudsy-disable [rules]` ... `sudsy-enable [rules]`: a block, from the `disable` comment's line until an `enable` naming the rule (or any `enable` without rules); without an `enable` it runs to the end of the file. Several blocks and partial enables compose in document order.
- `sudsy-disable-file [rules]`: the whole file, wherever the comment stands.
A directive naming a rule that does not fire (or does not exist) is not reported. The editor quick fix inserts `disable-next-line`. Plain JSON: config-only (`overrides` with `"off"`). Plain JSON: config-only (`overrides` with `"off"`).

## Test harness (`provider_api::testing`)
`lint`, `render`, `fix`, `lint_with`/`fix_with` (extension rules), `pattern_rules`, `pattern_error`, `expect`: fixtures with expected diagnostics written under the line (`// ^^^ rule-id`, `#` in YAML; carets give the span length). Fixes are checked to keep the file parseable and be idempotent. `plugin_sdk::testing` re-exports it.
Rule tests are fixture files, see [../development/testing.md](../development/testing.md#rule-fixtures).

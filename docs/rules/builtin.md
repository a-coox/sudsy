# Built-in rules

`sudsy rules` lists them, `sudsy rules <id>` explains one. Default levels in the table.

| Rule | Language | Category | Default | Fix | Notes |
|---|---|---|---|---|---|
| `max-line-length` | all | style | off | none | shared; options `max`, `allow_non_breakable_words` (default true: a line with no space after its indentation, a leading `#` run or `- `, such as a URL, is not reported) |
| `comment-space` | all | style | off | safe | shared; line comment needs a space after the marker; `////`, `--` lines, whitespace-only and a first-line `#!` are exempt; SA1005, yamllint `require-starting-space` |
| `no-empty-comment` | all | style | off | none | shared; empty block comments, and an empty line comment at the start or end of a comment run; SA1120 |
| `no-duplicate-keys` | json | correctness | error | none | keys compared after decoding escapes |
| `sorted-keys` | json | style | off | unsafe | sorting options below; reports the keys that would have to move (the fewest, like eslint-plugin-jsonc), the first report carries the fix; keys compared after decoding escapes |
| `no-empty-keys` | json | suspicious | off | none | empty or whitespace-only key after decoding escapes |
| `key-name-casing` | json | style | off | none | option `allowed` (default `[camelCase]`; also `PascalCase`, `snake_case`, `kebab-case`, `SCREAMING_SNAKE_CASE`) and `ignored_keys` (regexes); the checks are eslint-plugin-jsonc's, so a key with punctuation or whitespace never matches |
| `unsafe-numbers` | json | suspicious | off | none | integer beyond 2^53 - 1, or a value that overflows to infinity, underflows to zero or is subnormal as a double; lone surrogates are not checked |
| `sorted-array-values` | json | style | off | unsafe | arrays of only strings or only numbers (numbers compare as numbers), mixed or other arrays skipped; options `direction`, `compare`, `min_values` (default 2), `blank_line_groups`; reports like `sorted-keys` |
| `no-duplicate-keys` | yaml | correctness | error | none | scalars compared after unquoting; option `merge_keys` (`allow` default skips `<<`, `forbid` reports a second `<<`) |
| `no-undefined-alias` | yaml | correctness | error | none | alias without an earlier `&anchor` in its document; the formatter does not need it resolved |
| `no-duplicate-anchors` | yaml | suspicious | off | none | an `&name` defined again in the same document; yamllint `forbid-duplicated-anchors` |
| `no-unused-anchors` | yaml | cleanup | off | safe | an anchor no alias refers to (an alias counts for the latest anchor above it); the fix removes the anchor; yamllint `forbid-unused-anchors` |
| `no-empty-values` | yaml | style | off | none | `key:` or `-` without a value, reported at the `:` or `-`; options `block_mappings`, `flow_mappings`, `block_sequences` (all true); a value with only an anchor or tag counts as present; yamllint `empty-values` |
| `float-style` | yaml | style | off | safe | plain floats; options (all false) `require_numeral_before_decimal` (fix: `.5` to `0.5`), `forbid_scientific_notation`, `forbid_nan`, `forbid_inf`; quoted and tagged scalars skipped; yamllint `float-values` |
| `no-octal-values` | yaml | suspicious | off | none | plain `0755` (`implicit`) and `0o755` (`explicit`), both options true; quoted and tagged skipped; yamllint `octal-values` |
| `quoted-strings` | yaml | style | off | unsafe | options `quote_type` (`any`, `single`, `double`, `consistent`), `required` (`always` default, `optional`, `only_when_needed`), `extra_required`, `extra_allowed` (regexes), `allow_quoted_quotes`, `check_keys`. Plain non-strings (`12`, `true`, `null`, dates), block scalars and `!!` tags are never checked. Fixes: quote a plain string and switch quote type are safe (the formatter's restyle, value kept); removing quotes is unsafe (`1e3` is a number to YAML 1.2 readers). Which strings need quotes follows yamllint (PyYAML's YAML 1.1 resolver). yamllint `quoted-strings` |
| `ambiguous-booleans` | yaml | suspicious | off | unsafe | plain `yes`/`no`/`on`/`off` in three casings and `True`/`TRUE`/`False`/`FALSE`; options `check_keys` (default true) and `allowed` (spellings not reported); unsafe because quoting changes meaning for YAML 1.1 parsers; off because every GitHub workflow's `on:` would fire |
| `document-start` | yaml | style | off | safe | |
| `sorted-keys` | yaml | style | off | unsafe | block mappings only, every key that sorts below an earlier key of its mapping is reported (like yamllint), the first report carries the fix; sorting options below; moving an anchor below its alias is why it is unsafe |
| `no-empty-catch` | csharp | suspicious | warn | none | no statements and no comment |
| `async-suffix` | csharp | style | warn | none | uses the type layer to check Task/ValueTask and interface implementation; name-based at `fast`; overrides, explicit implementations and `Main` skipped; no fix (rename touches other files) |
| `using-order` | csharp | cleanup | off | safe | System first (`system_first`), names by `compare` (`ordinal`, `culture`, `culture_ignore_case`), `static_position` / `alias_position` (`keep`, `after`: `using static` then aliases behind the plain usings, sorted); reports like StyleCop SA1208 to SA1211, SA1216, SA1217; `global using` keeps its place; the fix is not offered for lists with `#` directives; overlaps the `dotnet_sort_system_directives_first` .editorconfig key. The formatter's `sort_usings` uses the same ordering code |
| `unreachable-code` | csharp | suspicious | off | suggestion | option `non_returning_calls` (default true); overlaps CS0162 and IDE0035 |
| `unused-variable` | csharp | cleanup | off | safe | overlaps compiler CS0168 and CS0219 for declaration and catch variables |
| `unused-assignment` | csharp | cleanup | off | none | dead store via liveness; overlaps IDE0059 |
| `remove-unused-usings` | csharp | cleanup | off | safe | overlaps compiler CS8019 and IDE0005; silent when the file has errors or `#if` |
| `possible-null-dereference` | csharp | correctness | off | none | maybe-null receivers; off because it repeats the compiler's CS8602 |
| `unused-private-member` | csharp | cleanup | warn | none | project scope, `analysis: deep` only; overlaps IDE0051 and IDE0052 |
| `doc-comment-space` | csharp | style | off | safe | `///` followed by exactly one space on the first line of a doc comment; SA1004 |
| `directive-keyword-space` | csharp | style | off | safe | `# if` -> `#if`; SA1006 |
| `no-regions` | csharp | style | off | none | every `#region` outside a block; SA1124 |
| `no-empty-statement` | csharp | suspicious | off | safe | lone `;` (not after a label); embedded bodies become `{ }`; SA1106 |
| `no-comment-before-block` | csharp | style | off | none | comment between a statement head and its `{`; SA1108 |
| `use-string-empty` | csharp | style | off | safe | `""` -> `string.Empty` outside constant positions; SA1122 |
| `separate-attributes` | csharp | style | off | safe | `[A, B]` -> `[A]` `[B]` (not on parameters, type parameters, lists with comments); SA1133 |
| `no-empty-attribute-parens` | csharp | style | off | safe | `[A()]` -> `[A]`; SA1411 |
| `no-empty-delegate-parens` | csharp | style | off | unsafe | `delegate() { }` -> `delegate { }`; unsafe because overload resolution can change; SA1410 |
| `no-region-in-body` | csharp | style | off | none | `#region` inside a block (`no-regions` reports the others); SA1123 |
| `nullable-shorthand` | csharp | style | off | safe | `Nullable<int>` -> `int?`, name-based; not in using aliases, `nameof` or before a member access; SA1125 |
| `use-lambda` | csharp | style | off | safe | anonymous method -> lambda; fix only with a parameter list and no comment before the body; SA1130 |
| `constant-on-right` | csharp | style | off | safe | `null == x` -> `x == null`, `1 < x` -> `x > 1`; constants are literals, `nameof`, `int.MaxValue`-style members; SA1131 |
| `use-literal-suffix` | csharp | style | off | safe | `(long)1` -> `1L`; not hex or binary literals cast to floating-point; SA1139 |
| `separate-field-declarations` | csharp | style | off | safe | `int a, b;` -> two declarations (fields, constants, events); SA1132 |
| `suppression-justification` | csharp | style | off | none | `SuppressMessage` without a meaningful `Justification` (missing, null, blank, `<Pending>`); SA1404 |
| `single-class-per-file` | csharp | style | off | none | classes and records after the first; structs, interfaces, enums, delegates and more parts of the first partial class not counted; SA1402 |
| `single-namespace-per-file` | csharp | style | off | none | every namespace after the first, nested ones included; SA1403 |
| `member-order` | csharp | style | off | none | member kinds, access, constants, static, readonly fields compared with the member before, in the order of option `element_order`; SA1201 to SA1204, SA1214; no fix (comments and directives would have to move along) |
| `accessor-order` | csharp | style | off | safe | `get` before `set`/`init`, `add` before `remove`; the fix swaps them unless a comment is in the way; SA1212, SA1213 |
| `file-name-matches-type` | csharp | style | off | none | file name up to the first dot (ignoring case, `Name{T}` for generics) is the first type; partial first type skipped; SA1649 |
| `no-unneeded-base-prefix` | csharp | style | off | safe | `base.M()` / `base[i]` where `this` binds to the same member; types layer; SA1100 |
| `no-default-value-type-constructor` | csharp | style | off | safe | `new S()` of a value type -> `default(S)`, `Guid.Empty`, `CancellationToken.None`, `IntPtr.Zero` or the enum member 0; no fix for `new()`; types layer; SA1129 |
| `qualify-namespace-usings` | csharp | style | off | safe | `using` in a namespace not written fully qualified; no fix with type arguments; types layer; SA1135 |
| `use-tuple-syntax` | csharp | style | off | safe | `ValueTuple<..>` types and `ValueTuple.Create(..)` -> tuple syntax (not in expression trees); types layer; SA1141 |
| `tuple-element-names-in-signatures` | csharp | style | off | none | unnamed tuple elements in signatures, not on overrides or interface implementations; types layer; SA1414 |
| `debug-assert-message` | csharp | style | off | none | `Debug.Assert` without a message or with a blank constant; by the written name `Debug.Assert` at `fast`, exact with the types layer; SA1405 |
| `debug-fail-message` | csharp | style | off | none | `Debug.Fail` with a blank constant message; same depths as `debug-assert-message`; SA1406 |
| `inheritdoc-on-inheriting-member` | csharp | style | off | none | `<inheritdoc />` without `cref` on a type without a base list or a member that neither overrides nor implements; types layer; SA1648 |

Rules that repeat a compiler or SDK diagnostic say so in `sudsy rules <id>`; those that fire as compiler warnings anyway are off by default. Sudsy does not reimplement SDK style rules such as IDE0161 (file-scoped namespaces) or IDE0029/IDE0030 (null coalescing).

Semantic rules' details: [semantics.md](semantics.md). A rule that is a style choice is off by default so zero-config adds no noise.

## Sorting options
`sorted-keys` (JSON, YAML) and `sorted-array-values` share one implementation (`core-rules::sorting`) and these options:

| Option | Default | Meaning |
|---|---|---|
| `order` | `[]` | `sorted-keys` only: keys that come first, in this order |
| `direction` | `asc` | `asc` or `desc` |
| `compare` | `ordinal` | `ordinal` (code points), `case_insensitive`, `natural` (digit runs compare as numbers, the rest by code point) |
| `ignored_keys` | `[]` | `sorted-keys` and `key-name-casing`: regexes searched in the key; with `sorted-keys` matching keys are not compared and stay where they are; a pattern that does not compile is a config error |
| `min_keys` / `min_values` | `2` | objects or arrays with fewer entries are not checked |
| `blank_line_groups` | `false` | sort each run of entries between blank lines on its own |

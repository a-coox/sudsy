# Built-in rules

`sudsy rules` lists them, `sudsy rules <id>` explains one. Default levels in the table.

| Rule | Language | Category | Default | Fix | Notes |
|---|---|---|---|---|---|
| `max-line-length` | all | style | off | none | shared; options `max`, `allow_non_breakable_words` (default true: a line with no space after its indentation, a leading `#` run or `- `, such as a URL, is not reported) |
| `no-duplicate-keys` | json | correctness | error | none | keys compared as written between quotes |
| `sorted-keys` | json | style | off | unsafe | option `order` |
| `no-duplicate-keys` | yaml | correctness | error | none | scalars compared after unquoting; `<<` skipped |
| `no-undefined-alias` | yaml | correctness | error | none | alias without an earlier `&anchor` in its document; the formatter does not need it resolved |
| `ambiguous-booleans` | yaml | suspicious | off | unsafe | plain `yes`/`no`/`on`/`off` in three casings and `True`/`TRUE`/`False`/`FALSE`; options `check_keys` (default true) and `allowed` (spellings not reported); unsafe because quoting changes meaning for YAML 1.1 parsers; off because every GitHub workflow's `on:` would fire |
| `document-start` | yaml | style | off | safe | |
| `sorted-keys` | yaml | style | off | unsafe | block mappings only, every key below a greater one is reported (like yamllint), the first report carries the fix; moving an anchor below its alias is why it is unsafe |
| `no-empty-catch` | csharp | suspicious | warn | none | no statements and no comment |
| `async-suffix` | csharp | style | warn | none | uses the type layer to check Task/ValueTask and interface implementation; name-based at `fast`; overrides, explicit implementations and `Main` skipped; no fix (rename touches other files) |
| `using-order` | csharp | cleanup | off | safe | overlaps the `dotnet_sort_system_directives_first` .editorconfig key; plain usings, ordinal, `system_first`; lists with `#` directives skipped |
| `unreachable-code` | csharp | suspicious | off | suggestion | option `non_returning_calls` (default true); overlaps CS0162 and IDE0035 |
| `unused-variable` | csharp | cleanup | off | safe | overlaps compiler CS0168 and CS0219 for declaration and catch variables |
| `unused-assignment` | csharp | cleanup | off | none | dead store via liveness; overlaps IDE0059 |
| `remove-unused-usings` | csharp | cleanup | off | safe | overlaps compiler CS8019 and IDE0005; silent when the file has errors or `#if` |
| `possible-null-dereference` | csharp | correctness | off | none | maybe-null receivers; off because it repeats the compiler's CS8602 |
| `unused-private-member` | csharp | cleanup | warn | none | project scope, `analysis: deep` only; overlaps IDE0051 and IDE0052 |

Rules that repeat a compiler or SDK diagnostic say so in `sudsy rules <id>`; those that fire as compiler warnings anyway are off by default. Sudsy does not reimplement SDK style rules such as IDE0161 (file-scoped namespaces) or IDE0029/IDE0030 (null coalescing).

Semantic rules' details: [semantics.md](semantics.md). A rule that is a style choice is off by default so zero-config adds no noise.

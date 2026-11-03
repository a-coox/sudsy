# Built-in rules

`sudsy rules` lists them, `sudsy rules <id>` explains one. Default levels in the table.

| Rule | Language | Category | Default | Fix | Notes |
|---|---|---|---|---|---|
| `max-line-length` | all | style | off | none | shared |
| `no-duplicate-keys` | json | correctness | error | none | keys compared as written between quotes |
| `sorted-keys` | json | style | off | unsafe | option `order` |
| `no-duplicate-keys` | yaml | correctness | error | none | scalars compared after unquoting; `<<` skipped |
| `no-undefined-alias` | yaml | correctness | error | none | alias without an earlier `&anchor` in its document; the formatter does not need it resolved |
| `ambiguous-booleans` | yaml | suspicious | off | unsafe | plain `yes`/`no`/`on`/`off`; unsafe because quoting changes meaning for YAML 1.1 parsers; off because every GitHub workflow's `on:` would fire |
| `document-start` | yaml | style | off | safe | |
| `sorted-keys` | yaml | style | off | unsafe | block mappings only; moving an anchor below its alias is why it is unsafe |
| `no-empty-catch` | csharp | suspicious | warn | none | no statements and no comment |
| `async-suffix` | csharp | style | warn | none | uses the type layer to check Task/ValueTask and interface implementation; name-based at `fast`; overrides, explicit implementations and `Main` skipped; no fix (rename touches other files) |
| `file-scoped-namespace` | csharp | cleanup | off | unsafe | needs C# 10; formatter re-indents after |
| `using-order` | csharp | cleanup | off | safe | plain usings, ordinal, `system_first`; lists with `#` directives skipped |
| `prefer-null-coalescing` | csharp | style | off | unsafe | `a != null ? a : b` on names/member accesses; type can change |
| `unreachable-code` | csharp | suspicious | warn | suggestion | option `non_returning_calls` (default true) |
| `unused-variable` | csharp | cleanup | warn | safe | compiler CS0168 and CS0219 for declaration and catch variables |
| `unused-assignment` | csharp | cleanup | off | none | dead store via liveness |
| `remove-unused-usings` | csharp | cleanup | off | safe | compiler CS8019; silent when the file has errors or `#if` |
| `possible-null-dereference` | csharp | correctness | off | none | maybe-null receivers; off because it repeats CS8602 |
| `unused-private-member` | csharp | cleanup | warn | none | project scope, `analysis: deep` only |

Semantic rules' details: [semantics.md](semantics.md). A rule that is a style choice is off by default so zero-config adds no noise.

# `.editorconfig`

A fallback source of format options: below the `extends` chain, above provider defaults. Files are found by walking up from the formatted file until one says `root = true`; a closer file overrides a farther one, a later section an earlier one. Globs follow the EditorConfig spec (`*`, `**`, `?`, `[seq]`, `[!seq]`, `{a,b}`, `{n..m}`); a section without `/` matches at any depth.

| `.editorconfig` | Option |
|---|---|
| `indent_style` (`space`, `tab`) | `indent_style` |
| `indent_size` (number; `tab` means `tab_width`) | `indent_size` |
| `tab_width` | `indent_size`, only for `indent_style = tab` without `indent_size` |
| `max_line_length` (number; `off` ignored) | `line_width` |
| `insert_final_newline` | `final_newline` |
| `end_of_line` (`lf`, `crlf`; `cr` ignored) | `end_of_line` |
| `csharp_new_line_before_open_brace` (`all`, `none`, or a list of elements) | `brace_newline` and `wrapped_brace_line: none` for `none`; for a list `brace_newline: all` and `brace_same_line` = the elements not listed (C#) |
| `csharp_new_line_before_else`, `_catch`, `_finally` (`true`, `false`) | `else_line`, `catch_line`, `finally_line` (`new_line`, `same_line`) (C#) |
| `csharp_indent_labels` (`flush_left`, `one_less_than_current`, `no_change`) | `label_indent` (`flush_left`, `one_less`, `no_change`) (C#) |
| `csharp_preserve_single_line_blocks`, `csharp_preserve_single_line_statements` (`true`, `false`) | `preserve_single_line_blocks`, `preserve_single_line_statements` (C#); both default to `false` in Sudsy, whereas `dotnet format` defaults to `true`, so a file that does not set the keys is formatted with the options' defaults |
| `csharp_new_line_between_query_expression_clauses` (`true`, `false`) | `preserve_query_breaks` (C#) |
| `dotnet_sort_system_directives_first`, `dotnet_separate_import_directive_groups` (`true`, `false`) | `sort_usings_system_first`, `separate_using_groups` (C#; they act with `sort_usings`) |

Brace elements follow `dotnet format`: `types` covers namespaces, `methods` covers constructors, operators and local functions (the `local_functions` element has no effect there), `properties` covers indexers and events (`indexers` and `events` have none), `control_blocks` covers `switch` and bare blocks. `dotnet format whitespace` moves the brace of an initializer neither way, so `object_collection_array_initializers` is Sudsy-only (`scripts/parity.sh format dotnet-format`).

## Unmapped formatting keys
A `csharp_*` formatting key without an option, set by a section that applies to a C# file to something other than the behavior Sudsy has, is reported once per `.editorconfig` line on stderr by `format`, `lint`, `fix` and `check` (`warning: <file>:<line>: `key = value` is not supported by Sudsy and is ignored`) and by `config print <file>` (`ignored .editorconfig key: ...`). The keys and the value Sudsy behaves as: the `csharp_space_*` keys (22; the Visual Studio defaults, e.g. `csharp_space_after_cast = false`), `csharp_indent_case_contents`, `_switch_labels`, `_block_contents` (`true`), `csharp_indent_braces` (`false`), `csharp_new_line_before_members_in_object_initializers` and `_in_anonymous_types` (`true`). Setting one to its default is silent. Code-style keys (`dotnet_style_*`, `csharp_style_*`, `dotnet_diagnostic.*`, naming rules) belong to the SDK and are ignored without a warning, as is every other key.

Everything else belongs to other tools and is ignored. A value that is invalid for the option (width 5, `off`) is ignored, not an error: the file is shared with other tools. Options a language does not declare are ignored for its files. The mapping table lives in `crates/core-config/src/editorconfig.rs`.

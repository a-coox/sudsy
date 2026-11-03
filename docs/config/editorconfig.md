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
| `csharp_new_line_before_open_brace` (`all`, `none`; other lists ignored) | `brace_newline` (C#) |

Everything else belongs to other tools and is ignored. A value that is invalid for the option (width 5, `off`) is ignored, not an error: the file is shared with other tools. Options a language does not declare are ignored for its files. The mapping table lives in `crates/core-config/src/editorconfig.rs`.

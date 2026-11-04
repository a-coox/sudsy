# Formatting engine (`core-format`)

Provider formatting code turns the tree into a layout document using shared shapes; a shared Prettier-style printer fits it to the width. Formatting code reads the resolved options directly (no spec layer). A node with no code of its own uses its grammar tag's shape (`#block`, `#delimited-list`, ...).

## Layout document and printer
- Primitives: `text`, `line`, `softline`, `hardline`, `group`, `indent`, `align`, `if_break`, `fill`, `conditional_group`, `line_suffix`, `break_parent`, `Verbatim`, `Shared(Arc)` (O(1) clone, so conditional candidates do not copy), `LineStart`, `NewlineIfNeeded`, group ids with `indent_if_break`.
- Group fits flat if the rest of the line up to the next possible break fits. `conditional_group` takes the first candidate that fits (flat, forced breaks allowed); one whose every candidate has a forced break forces its parents. Line suffixes flush at the next newline; trailing whitespace is trimmed except protected verbatim output.
- Widths are Unicode display width. Text containing a newline breaks enclosing groups; only its first line counts when measuring.
- `format_tree` prints LF, then applies `final_newline` and the line ending: `end_of_line` `lf` or `crlf`, or (`auto`, also for a provider without the option) the file's first line break.

## Shared options
Options declared by more than one language, generated from the option schemas; providers set their own defaults and may omit some (YAML has no `indent_style`). Each language's full list is in its doc.

<!-- generated:shared-options -->
| Option | Values | Defaults |
|---|---|---|
| `array_bracket_spacing` | bool | json `false`, yaml `false`, toml `false` |
| `bracket_spacing` | bool | json `true`, yaml `true`, toml `true` |
| `collapse` | json: `preserve`, `always`, `never`, `opening_break`; toml: `preserve`, `always`, `never` | json `preserve`, toml `preserve` |
| `end_of_line` | `auto`, `lf`, `crlf` | json `auto`, yaml `auto`, toml `auto`, csharp `auto` |
| `final_newline` | bool | json `true`, yaml `true`, toml `true`, csharp `true` |
| `indent_size` | int 1-16 | json `2`, yaml `2`, toml `2`, csharp `4` |
| `indent_style` | `space`, `tab` | json `space`, toml `space`, csharp `space` |
| `line_width` | int 20-1000 | json `80`, yaml `80`, toml `80`, csharp `120` |
| `trailing_commas` | json: `never`, `multiline`; toml: `never`, `multiline`; csharp: `preserve`, `never`, `multiline` | json `never`, toml `multiline`, csharp `preserve` |
<!-- /generated -->

## Shapes (`core-format::shapes`)
Delimited list (layouts: fit, preserve a break, break always, `Flat` never breaks, as TOML inline tables), block, key-value, statement sequence, verbatim, binary chain (operator first on continuation lines), member chain (one line or one group per line), and `delimited_list_between` / `block_between` for delimiters that are not first/last tokens. A shape finds its structure from the grammar: first/last significant token are delimiters, child nodes are items, `#separator` tokens after an item are its separator. List separators are re-created from `ListStyle`; block separators are kept. A child tagged `#list-prefix` is printed first. Blank lines between items are kept up to `max_blank_lines`; they vanish in a list that fits one line.

## Comments
Never positioned by the formatter: a comment is owned by a token (trivia rule) and printed with it (`FormatCtx::token`). Own-line comments stay on their own lines (`NewlineIfNeeded`); a block comment on the same line stays inline; a line comment after a token is a line suffix and breaks the group. Comments before a closing delimiter are dangling and printed inside the container. A second comment on a line already ending in a line comment moves to its own line. Structured trivia are comments: `Lines` (doc comments, line by line), `Directive` (`#verbatim-trivia`, kept at column 0; `#region` follows indentation).

## Layout directives (`DIRECTIVES_VERSION = 1`)
`BreakAll`, `KeepTogether`, `BreakBefore`/`BreakAfter`, `BlankLineBefore(n)`, `Align` (on the parent, only when expanded), `Preserve`, `Style(name)` (read by provider code only; none reads one yet). Rules request layout, the core performs it. Plugins cannot return layout documents. Text form (pattern files, plugin WIT, errors): `break-all`, `keep-together`, `break-before`, `break-after`, `blank-line-before N`, `align`, `preserve`, `style NAME`.

- Where honored: `BreakAll`/`KeepTogether` by every delimited list and by the C# member chain (on its outermost node: every call on its own line / never broken at the dots); `BreakBefore`/`BreakAfter`/`BlankLineBefore` by the gap between items of lists, blocks and statement sequences; `Align` by the key-value shape (the `#key-value` tag, JSON members); `Preserve` by every node.
- Conflicts (`BreakAll`+`KeepTogether`, `Preserve`+anything, differing `BlankLineBefore`/`Style`) are errors detected when added, naming both rules; the file is not formatted.
- Producers: every rule source (built-in, pattern `layout:`, Wasm plugin) through one path, `RuleContext::attach` (see [../rules/engine.md](../rules/engine.md#layout)). The `format` query reads the `layout(file)` query: the layout rules run in a pass of their own (syntax only, no semantic layers), so formatting depends on those rules' directives and not on the rest of the lint results; a lint-only config edit leaves `format` valid. With no layout rule in the language nothing runs.
- Levels and suppressions: an `off` rule attaches nothing; `sudsy-disable-next-line <rule>` silences the directives of nodes starting on the next line. `format`, `check`, the editor and fix-then-format all read the same query; the safety check (debug builds) applies to the result.

## Ignore regions
`sudsy-ignore` before a node keeps it verbatim (outermost node starting at that token); `sudsy-ignore-start` .. `sudsy-ignore-end` among the items of a list/block keeps those items verbatim; an unterminated start runs to the container's end. The marker is the comment's first word. C# honors it on statements, members and list items, not mid-expression.

## Safety check (debug builds and tests)
`provider_api::format_file`: parse, refuse syntax errors, format. Release builds stop there and trust the formatter; debug builds (every test, and the corpus runs through the `corpus` cargo profile, release with debug assertions) then have `format_parsed` re-parse the output and compare node starts/ends and non-trivia tokens. Ignored in the comparison: trivia (structured trivia whole), `#separator` tokens (trailing commas may change; a wrongly removed one still changes structure), `#restyle` positions. `NodeFormatter::extra_check(before, after)` is the hook for YAML's data-model comparison. Any difference aborts that file with an internal error. Idempotency is a test property, not a runtime check.
With C# `preprocessor_symbol_sets` / `derive_symbol_sets`, `NodeFormatter::merge_variants` returns the output with inactive `#if` branches formatted from other parses ([csharp.md](csharp.md#inactive-if-branches)); `format_parsed` then runs the check once per parse that supplied text (reparse under that parse's symbols), in every build because it decides whether the merge is used, and falls back to the unmerged output on any failure.
An option that reorders code (C# `sort_usings`) does it through `NodeFormatter::rewrite_source`: `format_parsed` takes the rewritten text, parses it again (a rewrite that does not parse is an internal error) and formats and checks that tree; the input tree is not compared with the output.

## Testing helper
`core_format::testing::check` enforces round trip, unchanged meaning, idempotency and comments kept. Scaling guard: `core-format/tests/scaling.rs` (steps of `fits` grow at most 4x for 4x the document).

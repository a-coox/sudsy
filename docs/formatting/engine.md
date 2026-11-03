# Formatting engine (`core-format`)

Provider formatting code turns the tree into a layout document using shared shapes; a shared Prettier-style printer fits it to the width. Formatting code reads the resolved options directly (no spec layer). A node with no code of its own uses its grammar tag's shape (`#block`, `#delimited-list`, ...).

## Layout document and printer
- Primitives: `text`, `line`, `softline`, `hardline`, `group`, `indent`, `align`, `if_break`, `fill`, `conditional_group`, `line_suffix`, `break_parent`, `Verbatim`, `Shared(Arc)` (O(1) clone, so conditional candidates do not copy), `LineStart`, `NewlineIfNeeded`, group ids with `indent_if_break`.
- Group fits flat if the rest of the line up to the next possible break fits. `conditional_group` takes the first candidate that fits (flat, forced breaks allowed); one whose every candidate has a forced break forces its parents. Line suffixes flush at the next newline; trailing whitespace is trimmed except protected verbatim output.
- Widths are Unicode display width. Text containing a newline breaks enclosing groups; only its first line counts when measuring.
- `format_tree` prints LF, then restores the file's line ending (first line break decides) and applies `final_newline`.

## Shared options
`line_width`, `indent_size`, `indent_style` (`space`|`tab`), `final_newline`; providers set their own defaults and may omit some (YAML has no `indent_style`).

## Shapes (`core-format::shapes`)
Delimited list, block, key-value, statement sequence, verbatim, binary chain (operator first on continuation lines), member chain (one line or one group per line), and `delimited_list_between` / `block_between` for delimiters that are not first/last tokens. A shape finds its structure from the grammar: first/last significant token are delimiters, child nodes are items, `#separator` tokens after an item are its separator. List separators are re-created from `ListStyle`; block separators are kept. A child tagged `#list-prefix` is printed first. Blank lines between items are kept up to `max_blank_lines`; they vanish in a list that fits one line.

## Comments
Never positioned by the formatter: a comment is owned by a token (trivia rule) and printed with it (`FormatCtx::token`). Own-line comments stay on their own lines (`NewlineIfNeeded`); a block comment on the same line stays inline; a line comment after a token is a line suffix and breaks the group. Comments before a closing delimiter are dangling and printed inside the container. A second comment on a line already ending in a line comment moves to its own line. Structured trivia are comments: `Lines` (doc comments, line by line), `Directive` (`#verbatim-trivia`, kept at column 0; `#region` follows indentation).

## Layout directives (`DIRECTIVES_VERSION = 1`)
`BreakAll`, `KeepTogether`, `BreakBefore`/`BreakAfter`, `BlankLineBefore(n)`, `Align` (on the parent, only when expanded), `Preserve`, `Style(name)` (read by provider code only). Rules request layout, the core performs it. Conflicts (`BreakAll`+`KeepTogether`, `Preserve`+anything, differing `BlankLineBefore`/`Style`) are errors detected when added. Plugins cannot return layout documents. No rule source attaches directives yet (pattern rules have no `layout:` key, the plugin WIT has no directive output); the shapes honor them and tests exercise them.

## Ignore regions
`sudsy-ignore` before a node keeps it verbatim (outermost node starting at that token); `sudsy-ignore-start` .. `sudsy-ignore-end` among the items of a list/block keeps those items verbatim; an unterminated start runs to the container's end. The marker is the comment's first word. C# honors it on statements, members and list items, not mid-expression.

## Safety check (every file, every run, not switchable)
`provider_api::format_file`: parse, refuse syntax errors, format, then `format_parsed` re-parses the output and compares node starts/ends and non-trivia tokens. Ignored in the comparison: trivia (structured trivia whole), `#separator` tokens (trailing commas may change; a wrongly removed one still changes structure), `#restyle` positions. `NodeFormatter::extra_check(before, after)` is the hook for YAML's data-model comparison. Any difference aborts that file with an internal error. Idempotency is a test property, not a runtime check.

## Testing helper
`core_format::testing::check` enforces round trip, unchanged meaning, idempotency and comments kept. Scaling guard: `core-format/tests/scaling.rs` (steps of `fits` grow at most 4x for 4x the document).

# JSON / JSONC (`lang-json`)

## Parser
Hand-written lexer + recursive descent; strings, numbers, escapes validated by the lexer. Stray tokens become `ERROR` nodes; every token is in the tree. One diagnostic per error site: a report overlapping an earlier one (the lexer already flagged that token) is dropped, and a bad member or element is skipped to the next `,` or closing delimiter without further reports; everything after a stray top-level token is one error. Comments and trailing commas are errors in the `json` dialect (same tree). Nesting beyond 256 is an error (rest of file is one `ERROR` node; bounds recursion in parser, formatter, drop). An empty file is an error.

## Dialect
`*.json`, `*.asmdef`, `*.asmref` and `*.inputactions` (Unity) are json; `*.jsonc`, `tsconfig.json`, `.vscode/*.json` are jsonc (most specific matcher wins). Override with option `dialect` (`auto`|`json`|`jsonc`). The effective dialect travels in `SourceFile::language_version`.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `bracket_spacing` | bool | `true` | Spaces inside the braces of a one-line mapping or object: `{ a: 1 }` or `{a: 1}`. |
| `array_bracket_spacing` | bool | `false` | Spaces inside the brackets of a one-line sequence or array: `[ 1, 2 ]` or `[1, 2]`. |
| `collapse` | `preserve`, `always`, `never`, `opening_break` | `preserve` | Objects and arrays on one line: `preserve` keeps one that has a line break between its brackets expanded and puts the others on one line if they fit; `always` puts every one that fits on one line; `never` puts every non-empty one on several lines; `opening_break` keeps an object expanded when a line break follows its opening brace and puts every array that fits on one line. |
| `trailing_commas` | `never`, `multiline` | `never` | Trailing commas after the last item of a multi-line object or array (JSONC only). |
| `dialect` | `auto`, `json`, `jsonc` | `auto` | JSON or JSONC (comments, trailing commas); `auto` decides by file name. |
<!-- /generated -->

`collapse: preserve` keeps an object/array on one line only if the source had no line break between its delimiters (nested items included) and it fits; `always` collapses anything that fits; `never` expands every non-empty one; `opening_break` keeps an object expanded after a line break following `{`.

Objects are tagged `#block` but laid out with the list shape.

## Rules
The shared key and sequence rules (`no-duplicate-keys`, `sorted-keys`, `sorted-array-values`, ...) run on `Object` and `Array`, which carry `#mapping` and `#sequence`; JSON's own are `unsafe-numbers` and its hooks decode string escapes (see [builtin.md](../rules/builtin.md)). Plain JSON has no comments, so suppression there is config-only (`overrides` with `"off"`).

## Conformance
JSONTestSuite pinned to nst/JSONTestSuite `1ef36fa` (`tests/conformance/fetch-json-corpus.sh`); every accepted file is formatted with all checks in both dialects.

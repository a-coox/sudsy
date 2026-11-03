# JSON / JSONC (`lang-json`)

## Parser
Hand-written lexer + recursive descent; strings, numbers, escapes validated by the lexer. Stray tokens become `ERROR` nodes; every token is in the tree. One diagnostic per error site: a report overlapping an earlier one (the lexer already flagged that token) is dropped, and a bad member or element is skipped to the next `,` or closing delimiter without further reports; everything after a stray top-level token is one error. Comments and trailing commas are errors in the `json` dialect (same tree). Nesting beyond 256 is an error (rest of file is one `ERROR` node; bounds recursion in parser, formatter, drop). An empty file is an error.

## Dialect
`*.json` is json; `*.jsonc`, `tsconfig.json`, `.vscode/*.json` are jsonc (most specific matcher wins). Override with option `dialect` (`auto`|`json`|`jsonc`). The effective dialect travels in `SourceFile::language_version`.

## Options
| Option | Values | Default |
|---|---|---|
| `line_width` | int | 80 |
| `indent_size` | int | 2 |
| `indent_style` | `space`, `tab` | `space` |
| `final_newline` | bool | true |
| `dialect` | `auto`, `json`, `jsonc` | `auto` |
| `trailing_commas` | `never`, `multiline` (JSONC only) | `never` |
| `collapse` | `preserve`, `always`, `never`, `opening_break` | `preserve` |

`collapse: preserve` keeps an object/array on one line only if the source had no line break between its delimiters (nested items included) and it fits; `always` collapses anything that fits; `never` expands every non-empty one; `opening_break` keeps an object expanded after a line break following `{`. Objects print `{ "a": 1 }`, arrays `[1, 2]` (fixed).

Objects are tagged `#block` but laid out with the list shape.

## Rules
`no-duplicate-keys`, `sorted-keys` (see [builtin.md](../rules/builtin.md)). Plain JSON has no comments, so suppression there is config-only (`overrides` with `"off"`).

## Conformance
JSONTestSuite pinned to nst/JSONTestSuite `1ef36fa` (`tests/conformance/fetch-json-corpus.sh`); every accepted file is formatted with all checks in both dialects.

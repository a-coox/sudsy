# TOML (`lang-toml`)

## Parser
tree-sitter-toml-ng 0.7 (TOML 1.0) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-toml/lang.yaml`, the tree shape `grammars/toml.grammar`. Every key is a `Key` node (one part or dotted: `a."b c".d`), every value a `Literal`, `Array` or `InlineTable`; `Table` (`[a.b]`) and `ArrayTable` (`[[a.b]]`) hold their pairs. Tree-sitter reports syntax errors ("syntax error", "missing X"). When it finds none, `validate.rs` checks what the grammar cannot and reports the violations as parse errors too, so a document that is not TOML is refused by the formatter and reported by the linter like a syntax error: duplicate keys (decoded, so `a`, `"a"` and `'a'` are one key; dotted keys; inline tables), redefined tables, tables and arrays of tables that conflict, extending an inline table, a value or a dotted-key table through a later header or dotted key, bad escapes and control characters in strings, quote runs in multi-line strings, date and time ranges (month, day by month and leap year, hour, minute, second, offset) and a missing line break after an entry. TOML 1.1 syntax (line breaks in inline tables, `\e` and `\xHH` escapes, seconds-less times) is a syntax error.

Files: `*.toml`. `Cargo.lock` (generated, cargo's own layout) is not claimed, so it is left untouched like other lockfiles.

## Formatting
- `key = value` with one space around `=`; dotted keys without spaces (`a.b`); comments, quoted keys and string quotes kept as written.
- A table header, then its pairs one per line, not indented. Blank lines between items are kept (at most `max_blank_lines`, one by default; after a comment line at most one).
- With `align_comments`, the trailing comments of consecutive entries (no blank line or comment line between them) start in one column, one past the widest entry of the run; a run with an entry on several lines is left alone.
- Inline tables are always on one line, however long (TOML 1.0 forbids line breaks in them): `{ a = 1, b = 2 }` (`bracket_spacing`).
- Arrays follow the shared list shape: on one line if they fit, else one item per line with an indented body; `collapse` chooses whether a line break in the source keeps an array expanded.

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
| `collapse` | `preserve`, `always`, `never` | `preserve` | Arrays on one line: `preserve` keeps one that has a line break between its brackets expanded and puts the others on one line if they fit; `always` puts every one that fits on one line; `never` puts every non-empty one on several lines. |
| `trailing_commas` | `never`, `multiline` | `multiline` | Trailing comma after the last item of a multi-line array. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between entries, tables and comments. |
| `align_comments` | bool | `false` | Start the trailing comments of consecutive entries in one column (entries with no blank line or comment line between them); a run with an entry on several lines is not aligned. |
<!-- /generated -->

## Presets
`toml/taplo-compat` reproduces taplo 0.10's defaults (`collapse: always`, `max_blank_lines: 2`, `align_comments: true`, LF); `scripts/parity.sh format taplo` compares the two on `tests/fixtures/toml` and the toml-test valid files (203 of 213 identical, the other 10 named in `tests/parity/taplo/differences.yaml`).

Where Sudsy differs from taplo by design:

<!-- generated:differences:taplo -->

- `comment-dropped`: taplo drops the comment after an array item that is followed by a comma on the next line; Sudsy keeps it
- `crlf-in-string`: taplo keeps CRLF inside a multi-line string and converts the rest; Sudsy writes LF everywhere
- `empty-file`: taplo writes a newline for an empty file; Sudsy keeps it empty
- `inline-table-array`: taplo expands an array inside a long inline table even when it fits after the break; Sudsy breaks only what does not fit
- `multiline-string-array`: taplo keeps an array with a multi-line string on one line; Sudsy expands it
- `taplo-rejects`: taplo rejects a valid file (`[[ products ]]` with blanks inside the brackets, some Unicode keys, strings, comments and datetimes); Sudsy formats it
<!-- /generated -->

## Rules
Only the shared ones (`comment-space`, `no-empty-comment`, `max-line-length`, and the key and sequence rules `no-duplicate-keys` (it never fires: a duplicate key is a parse error), `no-empty-keys`, `key-name-casing`, `sorted-keys`, `sorted-array-values`), which work through the grammar's tags: `Table`, `ArrayTable`, `InlineTable` and `Document` (its top-level pairs) are `#mapping`, `Array` is `#sequence`. The hooks decode quoted keys and join dotted parts with `.` (`a.b` and `"a"."b"` are one key; `key-name-casing` checks each part of a dotted key and reports the key); sorting reorders pairs only, never tables, and arrays of basic and literal strings, integers and floats are sorted (multi-line strings, booleans and dates are left alone). Suppression comments are `# sudsy-disable-next-line <rule>`.

## Conformance
toml-test v1.6.0, TOML 1.0 file list (`tests/conformance/fetch-toml-corpus.sh`, then `cargo test --profile corpus -p lang-toml --test integration -- --ignored --nocapture conformance`): all 185 valid files parse and format with all checks; all 361 invalid files are rejected (tree-sitter alone rejects 281, the semantic checks the other 80). `tests/fixtures/toml/` holds the `Cargo.toml` files of this repository and a hand-written file of every value form, checked on every test run.

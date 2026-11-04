# TOML (`lang-toml`)

## Parser
tree-sitter-toml-ng 0.7 (TOML 1.0) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-toml/src/mapping.rs`, the tree shape `grammars/toml.grammar`. Every key is a `Key` node (one part or dotted: `a."b c".d`), every value a `Literal`, `Array` or `InlineTable`; `Table` (`[a.b]`) and `ArrayTable` (`[[a.b]]`) hold their pairs. Tree-sitter reports syntax errors only ("syntax error", "missing X"); the formatter refuses a file with one. It does not detect semantic errors (duplicate keys, redefined tables, invalid dates), and TOML 1.1 syntax (line breaks in inline tables, `\e` and `\xHH` escapes, seconds-less times) is a syntax error.

Files: `*.toml`.

## Formatting
- `key = value` with one space around `=`; dotted keys without spaces (`a.b`); comments, quoted keys and string quotes kept as written.
- A table header, then its pairs one per line, not indented. Blank lines between items are kept (at most one).
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
<!-- /generated -->

## Rules
Only the shared ones (`comment-space`, `no-empty-comment`, `max-line-length`), which work through the grammar's tags. Suppression comments are `# sudsy-disable-next-line <rule>`.

## Conformance
toml-test v1.6.0, TOML 1.0 file list (`tests/conformance/fetch-toml-corpus.sh`, then `cargo test --release -p lang-toml --test toml -- --ignored --nocapture conformance`): all 185 valid files parse and format with all checks; 281 of the 361 invalid files are rejected (the rest are semantic errors tree-sitter does not report). `tests/fixtures/toml/` holds the `Cargo.toml` files of this repository and a hand-written file of every value form, checked on every test run.

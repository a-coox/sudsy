# Protobuf (`lang-protobuf`)

## Parser
tree-sitter-proto 0.6 (proto2, proto3 and editions, including the 2024 `export`/`local` modifiers) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-protobuf/src/mapping.rs`, the tree shape `grammars/protobuf.grammar`. The grammar has rules and keywords of the same name (`message`), so the mapping quotes the keywords (`"\"message\""`).

Files: `*.proto`. Tree-sitter reports syntax errors ("syntax error", "missing X"); a valid file never gets one: all 322 files of the corpus below parse.

## Formatting
Layout follows `buf format` (no style options of its own): two-space indentation, one statement per line, one space between the parts of a statement (`name = 1`, `map<string, Bar> m = 1`, `rpc M(A) returns (B)`), blocks (`{ }`) indented with `{}` for an empty one, blank lines kept (at most one), comments and their line positions kept. Quotes, numbers and names are kept as written.
- File header: statements are reordered as buf does: `syntax` or `edition`, `package`, `import`s (`public` first, `weak`, then `option` imports last; by path), file `option`s (built-in ones before custom `(a.b)` ones, by name), then everything else as written. Each statement moves with its comments (the ones in front and a comment on its last line); the blank lines inside the import and option groups go, one stands between package, import and option groups. Done on the source before parsing again (`sort.rs`), so the safety check compares against the reordered tree.
- Compact options `[a = 1]` stay on one line for one option and break one per line, without a trailing comma, from two (or when a comment stands inside).
- Message literals (`option (a) = { ... }`, in `[...]` too): one `name: value` per line, a colon after every name (`c {d: 1}` becomes `c: {d: 1}`), `;` and `,` between fields dropped, blank lines between fields kept; a literal of one scalar field stays on one line (`{d: 1}`), a list of more than one value breaks one per line.
- Adjacent string literals (`"a" "b"`) go one per line, indented one level, after `=`.

## Parity
`scripts/parity.sh format buf` compares with `buf format` 1.75.0 (downloaded on first use, `docs/parity/buf.yaml`) on `tests/parity/buf/` and a corpus of real files (`tests/conformance/fetch-proto-corpus.sh`: googleapis common and service protos, protobuf's own protos including proto2, proto3 and editions test files, grpc's): 320 of 323 files are identical. The other 3 are listed with their reasons in `tests/parity/buf/expected-diff.txt`: empty statements (`;`) are kept where buf drops them; comments before the `;` of a statement stay line comments on their own lines where buf joins them into block comments; comments separated by blank lines between the file options travel with the next statement where buf keeps them in place; a blank line before a comment between literal fields is dropped.
`cargo test --profile corpus -p lang-protobuf --test protobuf -- --ignored --nocapture conformance` checks every corpus file: parses without errors (322 of 322), round-trips, formats with the same tokens and idempotently.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

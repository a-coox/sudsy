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

## Lint
The rules are `off` by default (buf's lint is strict; Google's own protos fail it); `preset:protobuf/buf-standard` turns on buf's STANDARD category. Most are pattern files in `crates/lang-protobuf/rules/` (casing of messages, enums, services, rpcs, fields, oneofs, enum values and packages; package, syntax, version suffix, `import public`, `required`, service suffix, enum aliases and zero-value suffix), the rpc type names, the enum value prefix, the first enum value and the file name; `src/rules.rs` has none. The grammar has no `#mapping`/`#sequence` nodes, so only the shared comment and line-length rules apply.
- Not covered, listed with their reasons in [../parity/buf.yaml](../parity/buf.yaml): everything that needs other files or the module root (package/directory rules, `PACKAGE_SAME_*`, import cycles and use, unique request/response types), CEL validation, the comment rules and the unary-rpc rules (not in buf's default categories). The `_UNSPECIFIED` and `Service` suffixes are not configurable.
- `scripts/parity.sh buf` compares `buf lint` 1.75.0 (`buf.yaml`: STANDARD) with the rules on `tests/rules/protobuf/*/*.proto`: 32 findings, no difference. On the googleapis part of the corpus (573 findings of the mapped rules, compiled as one module; ad hoc, not in the script) the two agree on all 573; the protobuf and grpc trees do not compile as modules without the rest of their repositories.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

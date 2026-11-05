# EditorConfig (`lang-editorconfig`)

## Parser
konomanoasa-tree-sitter-editorconfig 0.9 (MIT, EditorConfig specification 0.17.2) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-editorconfig/src/mapping.rs`, the tree shape `grammars/editorconfig.grammar`. A `Document` holds the preamble pairs and the `Section`s; a `Section` is a `SectionHeader` (`[glob]`) and its `Pair`s; the glob, the key and the value are one token each. Tree-sitter's issue nodes (missing `]`, missing `=`) are unmapped, so a malformed line is a parse error and the file is refused. Chosen over a hand-written parser because the grammar follows the specification and the adapter, formatter shape and comment handling come with it (mapping 30 lines, no parser).

Files: the file name `.editorconfig` (`Spec::file_names`).

## Formatting
- `key = value` with one space around `=` (`key =` for an empty value); keys, globs and values are kept as written (keys are case-insensitive for EditorConfig, so they are not lowercased), no trailing whitespace.
- A blank line before every section (kept at most `max_blank_lines`); the comments in front of a section stay attached to it, so the blank line goes before them.
- Comments (`#` and `;` lines) stay in place. EditorConfig has no inline comments: `a = b ; c` is the value `b ; c`.
- `;` is a comment to the parser, but the grammar declares only `#` as the comment marker, so the shared comment rules see `;` comments as text without a marker.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between entries, sections and comments. |
<!-- /generated -->

## Presets
`editorconfig/default` sets nothing.

## Rules
Only the shared ones that need no grammar tags: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`.

## Conformance
`tests/conformance/fetch-editorconfig-corpus.sh` fetches the `.editorconfig` files of dotnet/runtime, roslyn and aspnetcore at pinned commits; `cargo test --profile corpus -p lang-editorconfig --test editorconfig -- --ignored --nocapture conformance` checks that each parses, round-trips, formats idempotently, and that `core_config::outline` (the reader that applies the file to formatting) finds the same sections and properties before and after.

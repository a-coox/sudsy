# Gitignore syntax (`lang-gitignore`)

## Parser
konomanoasa-tree-sitter-gitignore 0.7 (MIT, git 2.56 gitignore manual) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-gitignore/src/mapping.rs`, the tree shape `grammars/gitignore.grammar`. A `Document` holds one `Entry` per pattern line; an `Entry` is its adjacent `Pattern` tokens (`!`, names, `*`, `/`, escapes, bracket-expression parts). Comment lines are `Comment` trivia; blank lines, trailing spaces and carriage returns are whitespace. The grammar's issue nodes (a trailing lone `\`, an unterminated `[`) are unmapped, so such a line is a parse error and the file is refused. Chosen over a hand parser because the grammar follows the manual and the adapter, formatter shape and comment handling come with it.

Files: `.gitignore`, `.dockerignore`, `.npmignore`, `.prettierignore`, `.eslintignore`, `.sudsyignore` (by name) and `*.gitignore`, `*.dockerignore` (by extension; the fixtures and bench inputs use them). `.git/info/exclude` is not claimed: the file matchers cannot name a directory (see the roadmap).

## Formatting
- Trailing spaces are removed; a space escaped with `\` (`foo\ `) is part of the pattern and stays. Git strips spaces only, so blanks that contain a tab after a pattern are kept as written.
- Blank-line runs are capped at `max_blank_lines`; comments and patterns are kept as written (the order of patterns is meaning and is never changed).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between patterns and comments. |
<!-- /generated -->

## Presets
`gitignore/default` sets nothing.

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. Language rules (all `warn`):
- `no-duplicate-pattern`: the same pattern twice with no opposite pattern between.
- `redundant-pattern`: `foo/` or `/foo` after `foo` (also valid in `.dockerignore`; `**/foo` is left out because a bare name is root-only there).
- `negation-of-excluded-parent`: `!a/b` after a literal pattern that excludes `a` (git cannot re-include below an excluded directory; wildcard patterns are not judged; not reported in `.dockerignore`, where the later pattern wins).
- `inline-comment` (pattern rule): `*.log # note`, which is one pattern with a `#` in it.
- `backslash-separator` (pattern rule): `build\out`, where `\` escapes `o`.
Not done: Docker's differing semantics beyond the above (`*.log` matches only the root there); no other tool lints these files, so there is no parity entry.

## Conformance
`tests/conformance/fetch-gitignore-corpus.sh` fetches the `.gitignore` and `.dockerignore` files of moby, nvm, rustup, pyenv, asdf, bats-core and the dotnet install scripts at pinned commits (118 files); `cargo test --profile corpus -p lang-gitignore --test gitignore -- --ignored --nocapture conformance` checks that each parses, round-trips, formats with the same tokens and idempotently.

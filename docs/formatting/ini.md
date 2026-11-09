# INI (`lang-ini`)

## Parser
tree-sitter-ini 1.4 (MIT) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-ini/src/mapping.rs`, the tree shape `grammars/ini.grammar`. A `Document` holds the preamble `Setting`s and the `Section`s; a `Section` is a `SectionHeader` (`[name]`, the name one token, so a gitconfig `[remote "origin"]` is one name) and its `Setting`s (`key`, `=`, optional value, one token each). Comments are `#` or `;` lines. A line that is not a comment, a `[section]` or `key = value` (a key with no `=`, as in gitconfig booleans) is a parse error and the file is refused. That includes the indented continuation lines of a value (`install_requires =` then one requirement per line, as in `setup.cfg` and `tox.ini`): such files are left untouched; supporting them needs a grammar fork.

One language, no dialects: `.ini`, `.cfg`, `.gitconfig` and `.gitmodules` share the syntax and every rule is correct for all of them. `.properties` (`:`/space separators, `\` continuations, `!` comments, no sections) is not covered; it needs its own grammar and escape-aware formatter. `.git/config` is not matched, because file matchers see only the name `config`.

Files: extensions `.ini`, `.cfg`; names `.gitconfig`, `.gitmodules`.

## Formatting
- `key = value` (`key =` for an empty value), or `key=value` with `space_around_equals: false`; keys, names and values are kept as written, no trailing whitespace. An inline `; comment` after a value is part of the value.
- Settings start flush left (a gitconfig's tab indentation is removed).
- A blank line before every section (kept at most `max_blank_lines`); the comments in front of a section stay attached to it.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `space_around_equals` | bool | `true` | Write `key = value` rather than `key=value`. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between settings, sections and comments. |
<!-- /generated -->

## Presets
`ini/default` sets nothing.

## Rules
Shared: suppression comments, `no-empty-comment`, `comment-space`, `max-line-length`, and `no-duplicate-keys` (repeats in the preamble or one section; compared as written; gitconfig repeats keys on purpose for multi-valued settings such as `fetch` and `url`, so turn it off there).

Language rules:
- `no-duplicate-sections` (warn): the same section name twice in one file.
- `empty-section` (pattern rule, warn): a header with no settings under it.

No upstream tool lints INI content, so there is no parity entry.

## Conformance
`tests/conformance/fetch-ini-corpus.sh` fetches the `.cfg` and `.ini` files of ansible's tests and php-src's `php.ini-development` (70 KB) at pinned commits; `cargo test --profile corpus -p lang-ini --test ini -- --ignored --nocapture conformance` checks that each parses, round-trips and formats idempotently, and lists the files it cannot parse (one of ten, an ssh-config-like file).

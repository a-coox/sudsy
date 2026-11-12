# INI (`lang-ini`)

## Parser
tree-sitter-ini 1.4 (Apache-2.0), forked in `grammars/tree-sitter-ini` (continuation lines and keys without `=`; its FORK.md lists the changes), linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-ini/src/mapping.rs`, the tree shape `grammars/ini.grammar`. A `Document` holds the preamble `Setting`s and the `Section`s; a `Section` is a `SectionHeader` (`[name]`, the name one token) and its `Setting`s (`key`, optional `=`, optional value, then `Continuation` lines, one token each). Comments are `#` or `;` lines. The value token keeps the blanks around it; the formatter trims them and `IniFormatter::value` compares the trimmed values. A line that is not a comment, a `[section]`, a `key = value`, a bare `key` or a continuation line is a parse error and the file is refused.

A continuation line is an indented line right after a flush-left `key =` setting (`install_requires =` then indented lines, as in `tox.ini` and `setup.cfg`); an indented line after an indented setting is a setting of its own (`.gitconfig`, where every key is indented). The fork's external scanner (`src/scanner.c`) decides.

Files: extensions `.ini` and `.cfg`, names `.gitconfig` and `.gitmodules`. `.properties` is not matched: its `:` and blank separators, `\` continuations and `!` comments are not covered.

## Formatting
- `key = value` (`key =` for an empty value), or `key=value` with `space_around_equals: false`; keys, names and values are kept as written, no trailing whitespace. An inline `; comment` after a value is part of the value.
- Settings start flush left. A key without `=` is written as it is. Continuation lines keep their indentation as written (trailing blanks go); blank lines between them are part of the value and stay, at most `max_blank_lines`; a comment between them keeps its indentation.
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
No presets.

## Rules
Shared: suppression comments, `no-empty-comment`, `comment-space`, `max-line-length`, and `no-duplicate-keys` (repeats in the preamble or one section; compared as written; a `.gitconfig` that repeats a multi-value key such as `fetch` gets an error for each repeat).

Language rules:
- `no-duplicate-sections` (warn): the same section name twice in one file.
- `empty-section` (pattern rule, warn): a header with no settings under it.

No upstream tool lints INI content, so there is no parity entry.

## Conformance
`tests/conformance/fetch-ini-corpus.sh` fetches the `.ini` files of ansible's tests, php-src's `php.ini-development` (70 KB), the `tox.ini` and `setup.cfg` of pytest, setuptools and flake8 and a `.gitconfig` at pinned commits; `cargo test --profile corpus -p lang-ini --test ini -- --ignored --nocapture conformance` checks that each parses, round-trips and formats idempotently, and lists the files it cannot parse.

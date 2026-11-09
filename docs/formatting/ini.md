# INI (`lang-ini`)

## Parser
tree-sitter-ini 1.4 (MIT) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-ini/src/mapping.rs`, the tree shape `grammars/ini.grammar`. A `Document` holds the preamble `Setting`s and the `Section`s; a `Section` is a `SectionHeader` (`[name]`, the name one token) and its `Setting`s (`key`, `=`, optional value, one token each). Comments are `#` or `;` lines. The value token keeps the blanks around it; the formatter trims them and `extra_check` compares the trimmed values. A line that is not a comment, a `[section]` or `key = value` is a parse error and the file is refused.

Files: extension `.ini` only. The grammar has no continuation lines (`install_requires =` then indented lines, as in every `tox.ini` and `setup.cfg` of the pytest, setuptools and flake8 repositories) and no keys without `=` (gitconfig booleans, ansible inventories), so `.cfg`, `.gitconfig`, `.gitmodules` and `.properties` are not matched; they are on the roadmap. An `.ini` file that uses either construct is refused unchanged.

## Formatting
- `key = value` (`key =` for an empty value), or `key=value` with `space_around_equals: false`; keys, names and values are kept as written, no trailing whitespace. An inline `; comment` after a value is part of the value.
- Settings start flush left.
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
Shared: suppression comments, `no-empty-comment`, `comment-space`, `max-line-length`, and `no-duplicate-keys` (repeats in the preamble or one section; compared as written).

Language rules:
- `no-duplicate-sections` (warn): the same section name twice in one file.
- `empty-section` (pattern rule, warn): a header with no settings under it.

No upstream tool lints INI content, so there is no parity entry.

## Conformance
`tests/conformance/fetch-ini-corpus.sh` fetches the `.ini` files of ansible's tests and php-src's `php.ini-development` (70 KB) at pinned commits; `cargo test --profile corpus -p lang-ini --test ini -- --ignored --nocapture conformance` checks that each parses, round-trips and formats idempotently, and lists the files it cannot parse.

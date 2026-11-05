# Markdown (`lang-markdown`)

## Parser
tree-sitter-md 0.5 (CommonMark and GFM tables, task lists, strikethrough, front matter) through `core-treesitter` as a composite language ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#composite-languages)): the block grammar parses the file, its inline grammar is injected into every `inline` and `pipe_table_cell` node. Mapping `crates/lang-markdown/src/mapping.rs`, tree shape `grammars/markdown.grammar`.

Files: `*.md`, `*.markdown`.

- A paragraph is an `Inline` node directly under its container; a heading is `Heading > Inline`. Prose is `Text` tokens (one per line, blanks trimmed); escapes, entities and link punctuation stay inside them.
- Code spans (`Code`), inline and block HTML (`Html`) are nodes around their token, because pattern rules match nodes. Destinations, titles, labels and autolinks are tokens. Fenced code is `FencedCode` (`Fence`, `Info`, `CodeText`); indented code, link definitions, thematic breaks and front matter are tokens.
- Heading markers, setext underlines and quote continuation prefixes are trivia (`Marker`, `Prefix`), so they sit in the leading or trailing trivia of a neighbouring token, not inside their heading. A heading with no text, or a fenced block with no content, has no token to find its position by.
- HTML comments (`<!-- -->`, block or inline) are `HtmlComment` tokens tagged `#comment`: `<!-- sudsy-disable rule -->` and the other suppression markers work, and `no-empty-comment` applies.
- Conformance: the 652 CommonMark 0.31.2 examples (`tests/conformance/fetch-markdown-corpus.sh`, then `cargo test -p lang-markdown --test commonmark -- --ignored --nocapture`) all parse without errors and print back unchanged; 599 (91.9%) have the block and inline structure of pulldown-cmark. The rest are emphasis delimiter runs (12), reference links the parser cannot resolve against definitions (about 30), and a few hard breaks and raw HTML cases. Code spans with three or more backticks are read as text.

## Formatting
Line oriented, prose kept as written (Prettier's `proseWrap: preserve`, checked against Prettier 3.9 with `embeddedLanguageFormatting: off`). Blocks are separated by one blank line (none between the blocks of a tight list item, none between consecutive link definitions).
- ATX headings: `# Title`, no closing sequence. Setext headings are kept as written (as Prettier 3.9 does).
- Bullets are `-`; the next of two adjacent lists uses `*`. Ordered lists count up from the first number (`1. 1. 1.` and `0. 0. 0.` stay). A blank line after an item, or between the blocks of an item, makes that item loose. Task markers are `[ ]` and `[x]`; nested content is indented by the width of the marker (`indent_size` pads a short marker, as Prettier's `tabWidth`).
- Emphasis is `_text_` and strong `**text**`, only where this cannot change the parse: not next to a word character or another `*`/`_`, not around text that holds `*` or `_` or a blank at an edge, not in a link or image (reference labels are matched as written). Other delimiters stay as written. Escapes are the author's; nothing is added.
- Runs of blanks inside a line become one space (not next to a backtick); a hard break stays two trailing spaces.
- Fenced code uses backtick fences, longer than any backtick run inside (`~~~` stays when the info string has a backtick). Code, indented code and HTML blocks are copied with the prefix of their container replaced, trailing blanks included.
- Tables are aligned to the widest cell (display width), with `:---`, `:-:` and `---:` delimiter cells; the outer pipes are added.
- Thematic breaks are `---` (`***` first in a file or an item). Block quote lines all start with `> `. Link definitions have the blanks between their parts collapsed.
- Kept as written: a block containing a tab (its width depends on the column), a fence without a closing fence, a block quote or list containing indented code, front matter (not formatted as YAML), the next block after `<!-- sudsy-ignore -->`, and any block or file that fails the check below.
- Safety, in release builds too: the rendering of every top-level block is parsed again and must have the same signature as the block (structure, heading levels, list kinds, prose with blanks collapsed, raw text without container prefixes), and so must the whole output; otherwise the block (resp. the file) stays as written. Debug builds also run the generic check with `MarkdownFormatter::extra_check`. Both read the same parser, so for the cases where tree-sitter-md and the CommonMark algorithm differ the guard is a test: `cargo test --profile corpus -p lang-markdown --test markdown -- --ignored` (after the fetch script) requires every CommonMark example and the repo's docs to render to the same HTML with pulldown-cmark before and after formatting, and to be unchanged by a second run.
- Known differences from Prettier ([../../tests/parity/prettier/expected-diff.txt](../../tests/parity/prettier/expected-diff.txt)): code spans with three backticks, blocks with tabs, lists the restyle would parse differently, trailing blanks in code, wide characters in table cells (Prettier counts some differently), front matter.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

## Presets
`markdown/prettier-compat` sets the defaults (two-column list alignment, LF); `scripts/parity.sh format prettier` compares it with Prettier 3.9.10 on the repo's docs, a fixture and the dotnet/runtime coding-guidelines docs (56 of 64 files identical, the other 8 listed with their reasons).

## Rules
Shared: `max-line-length` and `no-empty-comment` (through the `HtmlComment` tag). Rust: `heading-increment`, `no-duplicate-heading`, `single-title`. Pattern files in `crates/lang-markdown/rules/`: `no-reversed-links`, `commands-show-output`, `no-missing-space-atx`, `no-trailing-punctuation`, `no-inline-html`, `no-bare-urls`, `no-emphasis-as-heading`, `no-space-in-code`, `no-space-in-links`, `fenced-code-language`, `no-empty-links`, `no-alt-text`, `descriptive-link-text`. All but the shared two are on (`warn`) by default, like markdownlint's. The catalog of all markdownlint rules (MD001-MD060, which are covered by a rule, by the formatter, partly, or deferred) is [../parity/markdownlint.yaml](../parity/markdownlint.yaml); `scripts/parity.sh markdownlint` compares the rules with markdownlint-cli2 on `tests/parity/markdownlint/`.

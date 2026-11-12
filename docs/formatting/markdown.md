# Markdown (`lang-markdown`)

## Parser
tree-sitter-md 0.5 (CommonMark and GFM tables, task lists, strikethrough, front matter) through `core-treesitter` as a composite language ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#composite-languages)): the block grammar parses the file, its inline grammar is injected into every `inline` and `pipe_table_cell` node. Mapping `crates/lang-markdown/src/mapping.rs`, tree shape `grammars/markdown.grammar`.

Files: `*.md`, `*.markdown`.

- A paragraph is an `Inline` node directly under its container; a heading is `Heading > Inline`. Prose is `Text` tokens (one per line, blanks trimmed); escapes, entities and link punctuation stay inside them.
- Code spans (`Code`), inline and block HTML (`Html`) are nodes around their token, because pattern rules match nodes. Destinations, titles, labels and autolinks are tokens. Fenced code is `FencedCode` (`Fence`, `Info`, `CodeText`); indented code, link definitions, thematic breaks and front matter are tokens.
- Heading markers, setext underlines and quote continuation prefixes are trivia (`Marker`, `Prefix`), so they sit in the leading or trailing trivia of a neighbouring token, not inside their heading. A heading with no text, or a fenced block with no content, has no token to find its position by.
- HTML comments (`<!-- -->`, block or inline) are `HtmlComment` tokens tagged `#comment`: `<!-- sudsy-disable rule -->` and the other suppression markers work, and `no-empty-comment` applies.
- Conformance: the 652 CommonMark 0.31.2 examples (`tests/conformance/fetch-markdown-corpus.sh`, then `cargo test --profile corpus -p lang-markdown --test markdown -- --ignored`) all parse without errors and print back unchanged; 599 (91.9%) have the block and inline structure of pulldown-cmark. The rest are emphasis delimiter runs (12), reference links the parser cannot resolve against definitions (about 30), and a few hard breaks and raw HTML cases. Code spans with three or more backticks are read as text.

## Formatting
Line oriented, prose kept as written (Prettier's `proseWrap: preserve`, checked against Prettier 3.9 with `embeddedLanguageFormatting: off`). Blocks are separated by one blank line (none between the blocks of a tight list item, none between consecutive link definitions).
- ATX headings: `# Title`, no closing sequence. Setext headings are kept as written (as Prettier 3.9 does).
- Bullets are `-`; the next of two adjacent lists uses `*`. Ordered lists count up from the first number (`1. 1. 1.` and `0. 0. 0.` stay). A blank line after an item, or between the blocks of an item, makes that item loose. Task markers are `[ ]` and `[x]`; nested content is indented by the width of the marker (`indent_size` pads a short marker, as Prettier's `tabWidth`).
- Emphasis is `_text_` and strong `**text**`, only where this cannot change the parse: not next to a word character or another `*`/`_`, not around text that holds `*` or `_` or a blank at an edge, not in a link or image (reference labels are matched as written). Other delimiters stay as written. Escapes are the author's; nothing is added.
- Runs of blanks inside a line become one space (not next to a backtick); a hard break stays two trailing spaces.
- Fenced code uses backtick fences, longer than any backtick run inside (`~~~` stays when the info string has a backtick). Code, indented code and HTML blocks are copied with the prefix of their container replaced, trailing blanks included.
- Tables are aligned to the widest cell (display width), with `:---`, `:-:` and `---:` delimiter cells; the outer pipes are added. A table with a row that has more cells than its header (an unescaped `|` in a code span splits a cell) is kept as written.
- Thematic breaks are `---` (`***` first in a file or an item). Block quote lines all start with `> `. Link definitions have the blanks between their parts collapsed.
- Embedded code: the first word of a fence's info string names a language (its name or a file extension: `json`, `jsonc`, `yml`, `sh`, `bash`, `ps1`, `toml`, `xml`, `dockerfile`, `editorconfig`, `css`, `scss`, `less`; `cs` is not formatted, the C# worker would start for a snippet). With `embedded_language_formatting: auto` the code is formatted with that language's formatter and its options (the `json:`, `yaml:` ... sections of the config, not the Markdown file's `.editorconfig`), indented as the fence is (lists, quotes). Code that does not parse, of a language that is not installed, or of a fence without a closing fence is kept as written, without an error. The formatting is a step after the Markdown render (`core-incremental`): the output is read again, so the fences it finds are the ones it formats, and it is left as rendered if they changed.
- Kept as written: a block containing a tab (its width depends on the column), a fence without a closing fence, a block quote or list containing indented code, a block whose indentation the renderer would change although it carries meaning (an indented paragraph line, a lazy `===` line in a quote, an indented opening fence, a list marker indented past its containers, wide marker spacing that keeps an indented block after the list out of it), front matter (not formatted as YAML), and the next block after `<!-- sudsy-ignore -->`.
- Safety: debug builds and the `corpus` profile run the generic check with `MarkdownFormatter::extra_check` (block and inline structure, heading levels, list kinds, prose with blanks collapsed, raw text without container prefixes); release builds skip it, like every language. The check reads the same parser, so for the cases where tree-sitter-md and the CommonMark algorithm differ the guard is a test: `cargo test --profile corpus -p lang-markdown --test markdown -- --ignored` (after the fetch script) requires every CommonMark example and the repo's docs to render to the same HTML with pulldown-cmark before and after formatting, and to be unchanged by a second run.
- Where Sudsy differs from Prettier by design (named in `tests/parity/prettier/differences.yaml`, with the GraphQL ones):

<!-- generated:differences:prettier -->

- `code-trailing-blanks`: trailing blanks inside a code block are code and stay; Prettier removes them
- `extend-directive`: `extend directive` is not in the GraphQL spec: the grammar rejects it
- `invalid-graphql`: the file is not valid GraphQL (Prettier's range markers, an empty selection set): Sudsy rejects it
- `tab-blocks`: a block with a tab is kept as written (the width of a tab depends on its column); Prettier re-indents the nested list
- `toc-indent`: a table of contents with nested lists indented by three columns is kept as written because the output would not parse to the same lists
<!-- /generated -->

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `embedded_language_formatting` | `auto`, `off` | `auto` | Format the code of a fenced block with the formatter of the language its info string names (`auto`, Prettier's `embeddedLanguageFormatting`), or keep it as written (`off`). Code that does not parse, or of a language that is not installed, is kept either way. |
<!-- /generated -->

## Presets
`markdown/prettier-compat` sets the defaults (two-column list alignment, LF); `scripts/parity.sh format prettier` compares it with Prettier 3.9.10 on the repo's docs, a fixture and the dotnet/runtime coding-guidelines docs (55 of 64 files identical, the other 9 listed with their reasons).

## Rules
The code of a fence is linted with the rules and config section of its language (`src/format/embedded.rs`; suppression comments inside the code and before the fence apply). Shared: `max-line-length` and `no-empty-comment` (through the `HtmlComment` tag). Rust: `heading-increment`, `no-duplicate-heading`, `single-title`. Pattern files in `crates/lang-markdown/rules/`: `no-reversed-links`, `commands-show-output`, `no-missing-space-atx`, `no-trailing-punctuation`, `no-inline-html`, `no-bare-urls`, `no-emphasis-as-heading`, `no-space-in-code`, `no-space-in-links`, `fenced-code-language`, `no-empty-links`, `no-alt-text`, `descriptive-link-text`. All but the shared two are on (`warn`) by default, like markdownlint's. The catalog of all markdownlint rules (MD001-MD060, which are covered by a rule, by the formatter, partly, or deferred) is [../parity/markdownlint.yaml](../parity/markdownlint.yaml); `scripts/parity.sh markdownlint` compares the rules with markdownlint-cli2 on `tests/parity/markdownlint/`.

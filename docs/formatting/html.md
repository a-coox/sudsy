# HTML (`lang-html`)

## Parser
tree-sitter-html 0.23.2, forked in `grammars/tree-sitter-html` ([FORK.md](../../grammars/tree-sitter-html/FORK.md): the doctype body is a node, CDATA sections parse, `textarea` and `title` hold text, not markup), linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)). The node mapping is the `mapping` of `crates/lang-html/lang.yaml`, the tree shape `grammars/html.grammar`.

- An `Element` is its start tag (`StartTag`, also for `<a/>` and `<br>`: its closing `>` or `/>` is one kind of token), its content and its `EndTag`. Void elements (`<br>`, `<img>`) and elements closed by an implied end tag (`<p>`, `<li>`) have no `EndTag`; the tree is the source as written, not the browser's repaired tree. `script`, `style`, `textarea` and `title` are `Element`s too.
- Text, entities (`&amp;`), CDATA sections and the content of `script`, `style`, `textarea` and `title` are one `Chunk` each, with a `Text`, `Entity`, `CData` or `RawText` token inside. Whitespace between nodes is trivia; comments are trivia.
- The attributes of a tag are `#key-value` entries of a `#mapping`: an `AttrName`, and when written an `=` and an `AttrValue` (quotes included).
- `Doctype` is `<!`, the keyword (`DoctypeKw`, any case), `DoctypeBody` and `>`. An end tag that closes no open element is a `StrayEndTag`.

Files: `.html`, `.htm`.

## Formatting
Prettier's printer-html is the reference (`format/model.rs` builds the nodes and the whitespace flags, `format/print.rs` lays them out); the defaults equal Prettier's.
- **Whitespace model:** whitespace between inline content is significant. Each node knows whether blanks before and after it are significant (from the CSS display of its neighbors and parent, Prettier's table of element defaults, `<!-- display: block -->` before an element overrides it) and whether there were blanks in the source. Where blanks are significant and absent, the closing `>` of the previous tag or the `<name` of the next one moves onto the neighboring line (`<a>x</a\n><b>y</b>`), so lines can break without adding rendered whitespace. `whitespace_sensitivity: ignore` makes every element a block, `strict` every element inline.
- **Elements:** children go on their own lines when they do not fit, block children always; `html`, `head`, `ul`, `ol`, `select` and table parts always break their children; one blank line between children is kept. Void elements and `<a/>` print as `<br />`; implied end tags are written (`<li>x<li>y` becomes two `<li>...</li>`); element names of known HTML elements and attribute names of known HTML attributes are lowercased.
- **Text:** words are filled to `line_width`. `pre`, `textarea` and `listing` keep their text exactly (a `pre` that holds elements is kept as written); `script` and `style` content is dedented and re-indented, not formatted.
- **Attributes:** values are double-quoted unless they hold more `"` than `'`; `class` values have their blanks collapsed; they fill the line or, when the tag is too wide, go one per line (`single_attribute_per_line`) with the `>` on its own line or at the end of the last attribute (`bracket_same_line`).
- **Comments:** stay where they are; a comment with blanks around it is on a line of its own. `<!-- prettier-ignore -->` and `<!-- sudsy-ignore -->` keep the next node as written.
- **Safety check:** the formatter rewrites the case of tag and attribute names and the doctype, the quotes of attribute values, the blanks of text and the form of the closing `>`; those tokens are `#restyle` and `HtmlFormatter::value` compares them without these changes. An end tag is `#transparent`: written or removed freely.
- **Embedded code** (`format/print/embed.rs`): the content of `<script>` (JS by `type` or `lang`; TS for `lang="ts"` or `application/x-typescript`; JSON for `*json`, `*importmap`, `speculationrules`; Markdown, HTML; other types are kept) and `<style>` (CSS, SCSS or Less by `lang`) is dedented, formatted by that language's formatter for the width left after the indentation, and indented again. Lines inside a template literal keep their indentation. Attribute values: `style` is a declaration list (`style="color: red; margin: 0"`, one declaration per line with a final `;` when it does not fit), event handlers (`on*`) an expression, else statements; `srcset` of `img` and `source` is a list of candidates (aligned when broken); `allow` of `iframe` a list of directives. Code that does not parse, or whose language is not installed, stays as written; `embedded_language_formatting: off` keeps all as written. The host reaches the other formatters through `FormatCtx::embed` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)). The safety check leaves these values to the formatter of their language.
- **Not done yet:** front matter, IE conditional comments as markup, and keeping text, an inline element and text written without blanks together on one line (see the parity gaps).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `embedded_language_formatting` | `auto`, `off` | `auto` | Format the code of `<script>` and `<style>` and of `style`, `srcset` and event-handler attributes with the formatter of its language (`auto`, Prettier's `embeddedLanguageFormatting`), or keep it as written (`off`). Code that does not parse, or of a language that is not installed, is kept either way. |
| `whitespace_sensitivity` | `css`, `strict`, `ignore` | `css` | Whether whitespace between inline elements and text is significant: `css` follows each element's CSS display, `strict` treats every element as inline, `ignore` as block. |
| `bracket_same_line` | bool | `false` | Put the `>` of a multi-line start tag at the end of the last attribute's line. |
| `single_attribute_per_line` | bool | `false` | Put every attribute of a tag that has several on its own line. |
<!-- /generated -->

## Conformance
`tests/conformance/fetch-html-corpus.sh` fetches the pinned corpus, and `cargo test --profile corpus -p lang-html --test html -- --ignored --nocapture conformance` checks that every file parses and formats with the shared checks (round trip, meaning, idempotency), or is reported as a syntax error. No file failed the checks; the files the grammar rejects:

| Group | Files | Syntax errors |
|---|---|---|
| Prettier 3.9.10 `tests/format/html` and `vue` (`.html`) | 159 | 12: deliberately broken markup (`_errors_`, `broken-html`, `unclosed`, bogus comments `<? ?>` and `<!- ->`), cursor markers, a `>` in text (`=>` in an interpolation example), `<plaintext>`, `pre.html` |
| Bootstrap 5.3.3 `js/tests` and `site` | 103 | 23: Hugo templates (`{{ ... }}` inside tags) and the `site/content/docs/5.3/examples` pages with front matter and `{{< >}}` shortcodes; the 13 JS test pages and 37 of the 48 layouts parse |
| mdn/learning-area (`.html`) | 154 | 0 |

## Lint
The rules are named and ordered after html-validate (`docs/parity/html-validate.yaml`; `docs/parity/htmlhint.yaml` and `html-eslint.yaml` map onto the same rules where they agree), run on the tree above: shared rules on the tags (`no-duplicate-keys` on attributes, compared in lowercase through `HtmlHooks`), pattern files in `crates/lang-html/rules/` for what a node, its text and its neighbors decide (case, quoting, spacing, self-closing and void tags, doctype, implicit end tags, `button` and `input` types, empty `title` and headings, ARIA roles, `valid-id`, `meta-refresh`, `require-sri`, `long-title`), and two Rust families:
- `meta` (`checks.rs`): checks against the element metadata `data/elements.txt`, generated by `data/generate.mjs` from html-validate's element table (names, void and deprecated elements, boolean, empty-allowed, deprecated and required attributes, the values an attribute is limited to): `no-unknown-elements`, `deprecated`, `no-deprecated-attr`, `element-required-attributes`, `attribute-boolean-style`, `attribute-empty-style`, `attribute-allowed-values` (values the metadata gives as a pattern, such as `id`, are not checked), `element-name`;
- document rules (`document.rs`): `no-duplicate-id`, `no-duplicate-class`, `no-multiple-main`, `heading-level`, `no-missing-references`, `no-redundant-for`, `no-conditional-comment`.

The parser already reports a stray end tag, a raw `&` or `<` in text and a self-closed `<script>` as `syntax-error`, which is the equivalent of html-validate's `close-order`, `no-raw-characters` and `script-element`. Presets: `html/html-validate-standard` and `html/html-validate-recommended` ([presets.md](presets.md)). All rules are off by default.

## Parity
`scripts/parity.sh format prettier-html` compares with Prettier 3.9.10 ([../development/parity.md](../development/parity.md)); the gaps are named in `tests/parity/prettier-html/differences.yaml`. Files identical to Prettier's output: `prettier-html` 118 of 144 that Prettier formats, `prettier-vue-html` 10 of 10, `real-world` 197 of 233. Lint parity: [Lint](#lint).

## Licenses
`bench/inputs/html`: the five `bootstrap-*` pages are Bootstrap 5.3.3's `js/tests/visual` (MIT, Copyright 2011-2024 The Bootstrap Authors); `mdn-accessibility-assessment.html` is from mdn/learning-area (CC0 1.0). tree-sitter-html is MIT ([LICENSE](../../grammars/tree-sitter-html/LICENSE)).

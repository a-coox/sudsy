# HTML (`lang-html`)

## Parser
tree-sitter-html 0.23.2, forked in `grammars/tree-sitter-html` ([FORK.md](../../grammars/tree-sitter-html/FORK.md): the doctype body is a node, CDATA sections parse, `textarea` and `title` hold text, not markup), linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)). The node mapping is the `mapping` of `crates/lang-html/lang.yaml`, the tree shape `grammars/html.grammar`.

- An `Element` is its start tag (`StartTag`, or `EmptyTag` for `<a/>`), its content and its `EndTag`. Void elements (`<br>`, `<img>`) and elements closed by an implied end tag (`<p>`, `<li>`) have no `EndTag`; the tree is the source as written, not the browser's repaired tree. `script`, `style`, `textarea` and `title` are `Element`s too.
- Text, entities (`&amp;`), CDATA sections and the content of `script`, `style`, `textarea` and `title` are one `Chunk` each, with a `Text`, `Entity`, `CData` or `RawText` token inside. Whitespace between nodes is trivia; comments are trivia.
- The attributes of a tag are `#key-value` entries of a `#mapping`: an `AttrName`, and when written an `=` and an `AttrValue` (quotes included).
- `Doctype` is `<!`, the keyword (`DoctypeKw`, any case), `DoctypeBody` and `>`. An end tag that closes no open element is a `StrayEndTag`.

Files: `.html`, `.htm`.

## Formatting
The formatter prints the document as written (final newline and line endings by the options).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

## Conformance
`tests/conformance/fetch-html-corpus.sh` fetches the pinned corpus, and `cargo test --profile corpus -p lang-html --test html -- --ignored --nocapture conformance` checks that every file parses and prints as written, or is reported as a syntax error. No file failed the checks; the files the grammar rejects:

| Group | Files | Syntax errors |
|---|---|---|
| Prettier 3.9.10 `tests/format/html` and `vue` (`.html`) | 159 | 12: deliberately broken markup (`_errors_`, `broken-html`, `unclosed`, bogus comments `<? ?>` and `<!- ->`), cursor markers (`<|>`), a `>` in text (`=>` in an interpolation example), `<plaintext>`, `pre.html` |
| Bootstrap 5.3.3 `js/tests` and `site` | 103 | 23: Hugo templates (`{{ ... }}` inside tags) and the `site/content/docs/5.3/examples` pages with front matter and `{{< >}}` shortcodes; the 13 JS test pages and 37 of the 48 layouts parse |
| mdn/learning-area (`.html`) | 154 | 0 |

## Licenses
`bench/inputs/html`: the five `bootstrap-*` pages are Bootstrap 5.3.3's `js/tests/visual` (MIT, Copyright 2011-2024 The Bootstrap Authors); `mdn-accessibility-assessment.html` is from mdn/learning-area (CC0 1.0). tree-sitter-html is MIT ([LICENSE](../../grammars/tree-sitter-html/LICENSE)).

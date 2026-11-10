# XML (`lang-xml`)

## Parser
tree-sitter-xml 0.7 (XML 1.0; the grammar of tree-sitter-grammars, MIT) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-xml/lang.yaml`, the tree shape `grammars/xml.grammar`. An `Element` holds its start tag (`STag`, or `EmptyTag` for `<a/>`), its content and its `ETag`. Text, references (`&amp;`, `&#65;`), CDATA sections, processing instructions, the XML declaration and the DOCTYPE are one `Chunk` each (a token inside); comments are trivia. Whitespace-only text between tags is trivia too. The attributes of a tag are `#key-value` entries of a `#mapping`. A DOCTYPE is kept as one token and its inside is not checked (the grammar cannot parse every DTD declaration).

Besides tree-sitter's syntax errors, `validate.rs` reports as parse errors what the grammar accepts: an end tag that does not match, a repeated attribute, `--` in a comment, `]]>` in text, an illegal character, a character reference to an illegal character, an entity other than the five predefined ones in a document without a DOCTYPE, a name that does not start with a letter, `_` or `:`, and an XML declaration that does not start the file.

Files: `.xml`, `.csproj`, `.vbproj`, `.fsproj`, `.props`, `.targets`, `.slnx`, `.config`, `.resx`, `.nuspec`, `.xaml`, `.axaml`, `.manifest`, `.ruleset`, `.proj`, `.projitems`, `.shproj`, `.vcxproj`, `.wixproj`, `.wxs`, `.runsettings`, `.vsixmanifest`, `.pubxml`, `.xsd`, `.xsl`, `.xslt`, `.svg`, `.plist`.

## Formatting
Whitespace in text is content, so the formatter lays out structure only.
- An element with only child elements and comments has each child on its own line, indented one level; blank lines between children are kept (at most `max_blank_lines`). `<a></a>` and `<a>` + blank text with a line break + `</a>` become `<a></a>`.
- An element with text, a reference, a CDATA section or a processing instruction, an element with `xml:space="preserve"`, and an element whose only content is blanks without a line break keep their content exactly as written.
- An empty element is `<b />` (one space before `/>`). Attributes are `name="value"`, single quotes become double quotes unless the value contains a double quote; values are never touched otherwise.
- A tag that does not fit within `line_width` has one attribute per line, indented, and its `>` (`/>`) on a line of its own; for an element whose text follows the tag, the `>` stays with the text (`>text</a>` on the line after the attributes). A single attribute never moves unless the element holds only text.
- The prolog (XML declaration, DOCTYPE, comments, processing instructions) and the root element are one per line; comments before and after the root are kept.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `100` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between elements, comments and declarations. |
<!-- /generated -->

## Presets
`xml/default` sets nothing. `xml/csharpier-compat` is CSharpier 1.x's XML defaults (100 columns, two spaces, line endings kept); `scripts/parity.sh format csharpier-xml` compares the two on `tests/fixtures/xml` and 372 project files of dotnet/aspnetcore v10.0.0 (`tests/conformance/fetch-xml-projects.sh`).

## Rules
The shared `no-empty-comment` and `max-line-length` work through the grammar's comment tags; `comment-space` does not apply (XML has no line comments). The attribute rules of the `#mapping` tag apply to attributes (`sorted-keys`, `key-name-casing`, `no-empty-keys`; a repeated attribute is a parse error, so `no-duplicate-keys` never fires). XML-specific: `no-duplicate-include` (a project item with the same name and `Include` as an earlier sibling) and `no-floating-package-version` (`PackageReference`/`PackageVersion` with `*` in `Version`, a pattern rule). Suppression comments are `<!-- sudsy-disable-next-line <rule> -->`.

## Conformance
W3C XML Conformance Test Suite 2013-09-23 (`tests/conformance/fetch-xml-corpus.sh`, then `cargo test --profile corpus -p lang-xml --test xml -- --ignored --nocapture conformance`): the UTF-8 tests of XML 1.0 fifth edition. Of the 824 well-formed documents, 785 parse and format with all checks (the 39 others use name characters or constructs the grammar lacks); of the 730 not-well-formed ones, 399 are rejected (tree-sitter alone rejects 305); of the 331 it accepts, 329 contain a DOCTYPE (DTD and entity rules are not checked) and 2 do not. `tests/fixtures/xml/` is checked on every test run.

# PowerShell (`lang-powershell`)

## Parser
woodruffw-trough-tree-sitter-powershell 0.1 (MIT, tree-sitter 0.27, PowerShell 7.5 reference parser, release 2026-10; the crate of the `woodruffw-trough/tree-sitter-powershell` repository) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-powershell/lang.yaml`, the tree shape `grammars/powershell.grammar`. The other grammar on crates.io (`tree-sitter-powershell` 0.26, airbus-cert, MIT) rejects 40 of the 210 corpus files, this one 1 (a C# source with a `.ps1` name). The cost is size: its `parser.c` is 51 MB (the airbus one 4.8 MB) and adds 11.7 MB to a release binary.

Files: `*.ps1`, `*.psm1`, `*.psd1`. The adapter runs in composite text mode (`Composite { text: "Word" }`): a leaf the mapping does not name (identifiers, variables, numbers, `.`, `::`) is a `Word`, and a backtick that ends a line is whitespace trivia.

The tree is flat: `Program` holds the tokens, with `Binary` (the operators of a binary expression, which tells `a - b` from `-1`), `Unary`, `Hash` (`@{ }`), `HashEntry` (`key = value`), `Command`, `Function`, `ScriptBlock` (a function's body), `ParamDecl`, `AssignStmt`, `CatchClause`, `Foreach` and `Block` (the `{ }` of a statement) as the only nodes; the formatter ignores all but `Binary` and `Hash`. A token belongs to the innermost of them, which is what a `token:` rule sees. Tokens: `Word`, `Parameter` (`-Name`), `Keyword`, `String` and `HereString` (whole, with the variables and `$(...)` inside), `Assign`, `Op`, `Comma`, `Semi`, `Pipe`, the brackets (`OpenBrace`, `HashOpen`, `OpenParen` for `(`, `$(` and `@(`, `OpenBracket` and the closers) and comments (`#` and `<# #>`). A rule that needs another node adds it with the rule.

## Formatting
Layout follows `Invoke-Formatter` of PSScriptAnalyzer with its default settings (the CodeFormatting preset, catalog `docs/parity/psscriptanalyzer.yaml`). Like Invoke-Formatter it adds blanks and line breaks and never removes the blanks the source has (`foo   -Name  bar` stays), so a script written with care keeps its alignment.
- Indentation: each open bracket (`{`, `@{`, `(`, `$(`, `@(`) indents the lines up to its closer by one level, a line that starts with a closer stands at the level of the line that opened it, and the lines after a `|` or a backtick at the end of a line are indented one more level (once, however many lines the pipeline has). A comment stands at the level of the code around it, also in front of `}`. Blank lines are kept up to `max_blank_lines`.
- Braces, for a block that spans lines: `{` joins the line of its head (`)`, `]`, a keyword, or the name after `function`) unless a comment is in the way; a statement written after `{` goes to its own line; `}` stands on a line of its own; `else`, `elseif`, `catch` and `finally` start a line after `}`. A block written on one line (`if ($a) { 1 } else { 2 }`) is not touched. A `{` on its own line after a command (`ForEach-Object` + line break + `{`) stays: joining it would change the meaning.
- Blanks added when the source has none: after `,` and `;`; around `=`, `+=`, ..., the binary operators (`+ - * / % -eq -and -f -join ...`, not `..`) and `|`; before `{` (not in `.ForEach{ }`, where a blank would make the block a separate statement); inside a one-line `{ }`; between `if`, `elseif`, `while`, `switch`, `foreach`, `for`, `until` and `(`.
- Hashtables that span lines: the `=` of the entries that start a line line up (one blank after the longest key), across blank lines and comments.
- Not changed: words, strings, here-strings, `<# #>` comment blocks (comment-based help keeps its inner indentation), splatting, `$(...)` inside strings, the line breaks of a multi-line expression, a backtick and its line break. Trailing blanks go.
- Not done: `PSUseCorrectCasing` (needs the command tables of a PowerShell), the Allman and Stroustrup brace styles, wrapping long lines.

`scripts/parity.sh format invoke-formatter` compares the output with Invoke-Formatter ([../development/parity.md](../development/parity.md)). Differences by design: Invoke-Formatter keeps trailing blanks, places a `{ one-line block }` that follows `else` on its own line at column 0 (Sudsy indents it), leaves `=3` unspaced in an aligned hashtable entry written `a=3` (Sudsy copies this), and joins every brace, including the one after a command.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `10` | Largest number of blank lines kept between statements and comments. |
<!-- /generated -->

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. The grammar has no `#mapping` or `#sequence` nodes, so the key rules do not apply. The language rules are a PSScriptAnalyzer subset, pattern files in `crates/lang-powershell/rules/` (`aliases`, `commands`, `declarations`, `backtick`), on by default at PSScriptAnalyzer's severities (Error is error, Warning is warn; Information rules are not built); the preset `powershell/psscriptanalyzer` lists them. Rules match the text of a `Command` (names are not case sensitive), a `ParamDecl`, an `AssignStmt`, a `Foreach` or a `Binary`, or a token of a node (`approved-verbs` reads the name token of a `Function`; `no-global-vars` and `no-misleading-backtick` read `Word` tokens, which sit in `Program` or in the innermost node around them). Which of PSScriptAnalyzer's 75 rules exist, and how they differ (checked against the module by `scripts/parity.sh psscriptanalyzer`), is in `docs/parity/psscriptanalyzer.yaml`. `[SuppressMessage('PSRule', ...)]` silences the mapped rule (through the parity catalog) for the function it is on or in, else for the file; its target and scope arguments are not read. The alias table and the approved verbs are fixed data in the rules; a rule that needs the command metadata of a PowerShell runtime is deferred.

## Parity and conformance
`scripts/parity.sh format invoke-formatter` (PSScriptAnalyzer 1.25.0, PowerShell 7.6) compares the output with Invoke-Formatter on `tests/parity/invoke-formatter/layout.ps1` and the corpus: 196 of 211 files are identical (trailing blanks ignored); the 15 others are listed with their reasons in `expected-diff.txt` (indentation after a multi-line pipeline that Invoke-Formatter derives from the syntax tree, enum alignment, a hashtable whose first entry is on the `@{` line).

`tests/conformance/fetch-powershell-corpus.sh` fetches scripts at pinned commits: dotnet-install.ps1, the modules and tools of PowerShell, the engine and tests of PSScriptAnalyzer, the sources of Pester. `cargo test --profile corpus -p lang-powershell -- --ignored --nocapture conformance` checks that each file the parser accepts (209 of 210; the other is C# in a `.ps1` file) round-trips, formats with the same tokens (the safety check) and formats idempotently.

# Shell (`lang-shell`)

## Parser
tree-sitter-bash 0.25.1 (MIT, tree-sitter organization, Bash grammar), forked in `grammars/tree-sitter-bash` (`FORK.md` lists the changes: `for x do`, a word ending in `$`, `a=1 b=2` + line break + a command, a quoted string in a case pattern, a here-string after another redirect; [../development/grammar-forks.md](../development/grammar-forks.md)), linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-shell/lang.yaml`, the tree shape `grammars/shell.grammar`. One grammar serves POSIX sh and Bash: a POSIX script is parsed as Bash. The adapter runs in composite text mode (`Composite { text: "Word", continuation: true, tolerant: [...] }`): a leaf the mapping does not name (`declare`, the text of a string, the inside of `(( ))`) is a `Word`, a `\` that ends a line is whitespace trivia, and a syntax error tree-sitter reports below an assignment, test, string, concatenation or substitution is not reported (tree-sitter-bash mis-parses a quoted string in a `=~` pattern; the text is valid and kept as written).

The tree is statements as nodes (`Command`, `Pipeline`, `List` (`&&`, `||`), `Redirected`, `Negated`, `Subshell`, `Group` (`{ }`), `If`/`Elif`/`Else`, `While`, `For`, `CFor`, `DoGroup`, `Case`/`CaseItem`, `Function`, `HeredocRedirect`) and the nodes of a word: `Assign` (`name=value`, also in a declaration), `Test` (`[ ... ]`, `[[ ... ]]`), `String` (`"..."`: text pieces, expansions and substitutions), `Concat` (adjacent parts of one word, `a$(b)"c"`), `Subst` (`$(...)` or backticks, holding the `Command` nodes inside; `<(...)` too), `Array` (`( b c )`, holding the element words) and `Redirect` (`> file`, `2>&1`, `<<< word`: operator and target), over flat tokens: `Keyword`, `Op` (`&`, `&&`, `||`, `|`, `|&`, `;;`), `Semi` (`;`, ignored by the safety check), `Word` (a word without quotes or substitutions, the text pieces of a string, `${...}` and `$var`), `Arith` (`$(( a + 1 ))`), `HeredocOp`/`HeredocBody`/`HeredocEnd`. Single-quoted strings and `$'...'` are one `Word`.

Files: `*.sh`, `*.bash`, `.bashrc`, `.bash_profile`, `.profile`. A file without an extension is not recognized by its `#!` line: file selection matches names and extensions only, and sniffing content for this one language is not worth a core feature.

## Formatting
Layout follows shfmt's defaults (tabs, no flags).
- One statement per line; `a; b` on one line becomes two lines; a `;` before a line break goes; `a &` keeps `&` after the statement. Blank lines between statements, after an opener and before a closer are kept up to `max_blank_lines`.
- `if cond; then`, `elif cond; then`, `while cond; do`, `for x in words; do`: `then` and `do` move to the head line (`; then`); bodies are indented one level; `fi`, `done`, `}`, `)` close at the head's column. A compound command written on one line with at most one statement per body (`if a; then b; fi`, `{ a; }`, `foo() { :; }`) stays on one line; any body with several statements, or a source with line breaks, is laid out on lines.
- `function`: `name() {` (the `{` joins the line of the name), `function name {` keeps the keyword.
- `case`: patterns at the column of `case`, alternatives as `a | b)`, the body indented, `;;` on its own line at the body's indentation. An arm written on one line (`a) b ;;`) stays on one line when it has at most one statement.
- Pipelines and lists: `a && b`; a line break after `&&`, `||` or `|` is kept and the rest indented one level; an operator behind a line continuation (`a \` + line break + `&& b`) moves to the end of the previous line, as shfmt does. A continuation between words is kept (`\` at the end of the line, the next line indented one level).
- Redirects: no blank after the operator (`2> f` is `2>f`) unless the word would run into it (`< <(cmd)`); a redirect keeps its place among the words.
- Heredocs: `<<EOF` / `<<-EOF` and the rest of the line are laid out; the body and the delimiter line are printed as written, at column 0, after the line.
- Comments: own-line comments stay on their lines at the indentation of what follows; a comment before `else`, `elif`, `fi`, `done`, `}` or `;;` goes into the body above it when it stands deeper than the keyword, else it stands at the keyword's column (shfmt's rule). A trailing comment keeps the blanks the source has before it (comments aligned in a column stay aligned; shfmt keeps the column instead, which differs when the code before the comment changed length).
- Substitutions and arrays: `$(...)` holds its commands on one line (`$(a | b)`, no blanks inside) unless it has several commands or the source breaks the line after `$(` or before `)`; then one command per line, indented one level from the line of the `$(`, `)` on its own line. The same inside strings. A `$(...)` with a heredoc or a continuation, and backticks, are kept as written. Array elements stay on the lines of the source; the lines after the first break are indented one level, a break after `(` or before `)` stays, no blanks inside `( )`.
- Not changed: words, `$(( ))`, `(( ))`, `[[ ]]`, `[ ]` and `<(...)` (but the blanks inside `(( ))`, `for (( ))` and `$(( ))` go: `((a))`; the operators inside are spaced like shfmt does: `((i = 0))`, `$((a + 1))`, `for ((i = 0; i < n; i++))`; a subscript and `i++ ))` are kept as written). The safety check allows only that and the blanks of redirects to differ from the input: every other token keeps its kind and text.

shfmt is the comparable tool; `scripts/parity.sh format shfmt` compares the output ([../development/parity.md](../development/parity.md)). Its flags map to nothing yet: `-i N` is `indent_style: space` + `indent_size: N`; `-bn`, `-ci`, `-sr`, `-fn` are deferred (catalog `docs/parity/shfmt.yaml`).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `4` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `tab` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between statements and comments. |
<!-- /generated -->

## Presets
`shell/shellcheck`: turns on the rules that have a shellcheck equivalent ([../parity/shellcheck.yaml](../parity/shellcheck.yaml)) at shellcheck's severities (error stays error; warning, info and style become `warn`). The formatter defaults are shfmt's.

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. The grammar has no `#mapping` or `#sequence` nodes, so the key rules do not apply. The language rules are a shellcheck subset, listed in [../rules/builtin.md](../rules/builtin.md); the catalog maps each SC code to its rule or says why it is deferred.
- Defaults: the quoting and command-misuse rules are on (`warn`, `quote-at-expansion` `error`); `declare-and-assign-separately` and the info and style rules are `off`. The preset turns on every rule at shellcheck's severities.
- Forms: pattern files in `crates/lang-shell/rules/` (`commands.yaml`, `quoting.yaml`); `quote-command-substitution`, `no-backticks` and `declare-and-assign-separately` match `Subst` and `Assign` nodes; `quote-variable`, `quote-at-expansion` and `quote-star-expansion` use `token:` (a regex on the word tokens and `Concat` nodes of a command or `[ ]`, [../rules/patterns.md](../rules/patterns.md#tokens)); `cd-without-exit` and `useless-echo` in Rust (`src/rules.rs`). The commands inside a `$(...)` are `Command` nodes, so every rule runs on them, and so do the embedded shell rules of a Dockerfile `RUN`.
- Syntax only: quoting is judged from the word's text, so `quote-variable` reads the arguments of a command and the words of `[ ]`, not the command's name, assignments, loop lists or `[[ ]]`, and cannot know a variable holds a number or one word (shellcheck follows literal assignments, so it stays silent on `x=3; [ $x -lt 4 ]`). A word with a quoted string that holds a nested `$(`/`${` ends the scan of that word, so such a word is not judged. Data flow (`SC2034`, `SC2154`), the target dialect (`SC3xxx`) and knowledge of other commands' options are deferred.

## Parity
`scripts/parity.sh shellcheck` compares the rules with shellcheck 0.11.0 (`-o all`, so the optional checks count) on the rule fixtures; see [../development/parity.md](../development/parity.md). Over the corpus below the rules agree with shellcheck on all 116 findings of covered codes; of the 125 Sudsy-only ones, 116 are `quote-variable` (95 in nvm.sh) on variables shellcheck knows hold a literal or a number and 9 are in places shellcheck does not report (`useless-echo` of a multi-line substitution, `no-which` and `quote-command-substitution` inside `[ ]`, `echo` and `$(...)` in a string, `cd-without-exit` in `$(cd d; pwd)`, `and-or-is-not-if-else` on `a && (b || c)`).

## Conformance
`tests/conformance/fetch-shell-corpus.sh` fetches shell scripts at pinned commits: the install scripts of dotnet, nvm, rustup, docker and Homebrew, the docker-library entrypoints of postgres and mysql, and the Bash sources of pyenv, bats-core and asdf. `cargo test --profile corpus -p lang-shell --test shell -- --ignored --nocapture conformance` checks that each file that tree-sitter parses (all 81) round-trips, formats with the same tokens (the safety check, comments compared as well) and formats idempotently. `tests/parity/shfmt/` compares with shfmt on the same files: 67 of 82 (the 81 corpus files and the `layout.sh` fixture) are identical, the 15 others are named in `tests/parity/shfmt/differences.yaml`. A file with a syntax error runs no rule.

Where Sudsy differs from shfmt by design:

<!-- generated:differences:shfmt -->

- `backticks`: backticks are kept (the formatter never changes tokens; the rule `no-backticks` fixes them); shfmt rewrites them to `$(...)`
- `case-last-arm`: a last `case` arm without `;;` is kept (the formatter never adds tokens); shfmt adds it
- `comment-column`: a trailing comment keeps the blanks of the source before it; shfmt keeps its column
- `empty-file`: a file of one empty line becomes an empty file; shfmt keeps the line
- `expr-kept`: `${v//x}` is kept (the formatter never adds tokens); shfmt writes `${v//x/}`
- `heredoc-tabs`: the tabs of a `<<-` heredoc body are kept (also inside `$(...)`); shfmt re-indents them
<!-- /generated -->

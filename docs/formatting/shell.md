# Shell (`lang-shell`)

## Parser
tree-sitter-bash 0.25 (MIT, tree-sitter organization, Bash grammar; release 2025-12) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-shell/src/mapping.rs`, the tree shape `grammars/shell.grammar`. One grammar serves POSIX sh and Bash: a POSIX script is parsed as Bash. The adapter runs in composite text mode (`Composite { text: "Word", continuation: true, tolerant: [...] }`): a leaf the mapping does not name (`declare`, the text of a string, the inside of `(( ))`) is a `Word`, a `\` that ends a line is whitespace trivia, and a syntax error tree-sitter reports below an assignment, test, string, concatenation or substitution is not reported (tree-sitter-bash mis-parses a quoted string in a `=~` pattern; the text is valid and kept as written).

The tree is statements as nodes (`Command`, `Pipeline`, `List` (`&&`, `||`), `Redirected`, `Negated`, `Subshell`, `Group` (`{ }`), `If`/`Elif`/`Else`, `While`, `For`, `CFor`, `DoGroup`, `Case`/`CaseItem`, `Function`, `HeredocRedirect`) and the nodes of a word: `Assign` (`name=value`, also in a declaration), `Test` (`[ ... ]`, `[[ ... ]]`), `String` (`"..."`: text pieces, expansions and substitutions), `Concat` (adjacent parts of one word, `a$(b)"c"`) and `Subst` (`$(...)` or backticks, holding the `Command` nodes inside), over flat tokens: `Keyword`, `Op` (`&`, `&&`, `||`, `|`, `|&`, `;;`), `Semi` (`;`, ignored by the safety check), `Word` (a word without quotes or substitutions, the text pieces of a string, `${...}` and `$var`), `Array` (`( b c )`), `Redirect` (`> file`, `2>&1`, `<<< word`), `HeredocOp`/`HeredocBody`/`HeredocEnd`. Single-quoted strings, `$'...'`, `<(...)` and `$(( ))` are one `Word`.

Files: `*.sh`, `*.bash`, `.bashrc`, `.bash_profile`, `.profile`. A file without an extension is not recognized by its `#!` line: file selection matches names and extensions only, and sniffing content for this one language is not worth a core feature.

Parser limits (both come from the grammar, not from the formatter):
- It rejects some valid Bash: `for x do` (no `in`), a word that ends in `$` before `;` (`grep -q 404$; then`), and whole files such as nvm's `nvm.sh`. The file then has a syntax error and is not formatted.
- It sometimes takes two statements for one: a line with two or more assignments (`a=1 b=2`) followed by a command on the next line, `z` + line break + `! cmd` after a pipeline, a pipeline that ends a line before a command with redirects. The formatter never changes a line break inside a `Command`, `Redirected` or `Negated` (where only blanks can stand) or after a pipeline/list element that is not an operator: such a node is kept as written. A mis-parse that keeps the meaning is formatted like any other tree.

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
- Not changed: words, strings, `$(...)` and backticks (also on several lines: their inner indentation stays; the nodes `String`, `Concat`, `Subst` and `Test` are printed as written), `$(( ))`, `(( ))`, `[[ ]]` and `[ ]`, array contents except the padding of `a=( b )` (`a=(b)`). The safety check allows only that and the blanks of redirects to differ from the input: every other token keeps its kind and text.

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
- Defaults follow shellcheck: its errors and warnings are on (`warn`/`error`), info and style are `off`.
- Forms: pattern files in `crates/lang-shell/rules/` (`commands.yaml`, `quoting.yaml`); `quote-command-substitution`, `no-backticks` and `declare-and-assign-separately` match `Subst` and `Assign` nodes; `quote-variable`, `quote-at-expansion` and `quote-star-expansion` use `token:` (a regex on the word tokens and `Concat` nodes of a command or `[ ]`, [../rules/patterns.md](../rules/patterns.md#tokens)); `cd-without-exit` and `useless-echo` in Rust (`src/rules.rs`). The commands inside a `$(...)` are `Command` nodes, so every rule runs on them, and so do the embedded shell rules of a Dockerfile `RUN`.
- Syntax only: quoting is judged from the word's text, so `quote-variable` reads the arguments of a command and the words of `[ ]`, not the command's name, assignments, loop lists or `[[ ]]`, and cannot know a variable holds a number or one word (shellcheck follows literal assignments, so it stays silent on `x=3; [ $x -lt 4 ]`). A word with a quoted string that holds a nested `$(`/`${` ends the scan of that word, so such a word is not judged. Data flow (`SC2034`, `SC2154`), the target dialect (`SC3xxx`) and knowledge of other commands' options are deferred.

## Parity
`scripts/parity.sh shellcheck` compares the rules with shellcheck 0.11.0 (`-o all`, so the optional checks count) on the rule fixtures; see [../development/parity.md](../development/parity.md). Over the corpus below the rules agree with shellcheck on 115 of the 116 findings of covered codes (one `quote-variable` in `unset a[$i]` is shellcheck-only); of the 25 Sudsy-only ones, 19 are `quote-variable` on variables shellcheck knows hold a literal or a number and 6 are in command substitutions that shellcheck does not report (`useless-echo` of a multi-line substitution, `no-which` and `quote-command-substitution` inside `[ ]` and `echo`, `cd-without-exit` in `$(cd d; pwd)`).

## Conformance
`tests/conformance/fetch-shell-corpus.sh` fetches shell scripts at pinned commits: the install scripts of dotnet, nvm, rustup, docker and Homebrew, the docker-library entrypoints of postgres and mysql, and the Bash sources of pyenv, bats-core and asdf. `cargo test --profile corpus -p lang-shell --test shell -- --ignored --nocapture conformance` checks that each file that tree-sitter parses (78 of 81) round-trips, formats with the same tokens (the safety check, comments compared as well) and formats idempotently. `tests/parity/shfmt/` compares with shfmt on the same files: 58 of 82 (the 81 corpus files and the `layout.sh` fixture) are identical, the 24 others are listed with their reasons in `expected-diff.txt`. A file with a syntax error runs no rule, and tree-sitter-bash rejects some valid shell (`for f do :; done`, `grep -q 404$; echo`): three corpus files (nvm.sh, rustup-init.sh, pyenv's conda.bash) get `syntax-error` and no findings.

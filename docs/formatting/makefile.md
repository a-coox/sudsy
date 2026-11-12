# Makefile (`lang-makefile`)

## Parser
tree-sitter-make 1.1.1 (GNU make, forked in `grammars/tree-sitter-make`) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-makefile/lang.yaml`, the tree shape `grammars/makefile.grammar`. The language is a composite with no injected grammar: text no mapped node covers (function names, the parentheses of `ifeq`) is a `Word` and a `\` ending a line is whitespace. A `Rule` holds `Targets`, a `Colon`, `Prereqs` and a `Recipe` of `RecipeLine`s; an `Assign` is `Word Op Value`; `include`, `export`, `ifeq` and the like are `Directive`s, `ifeq` to `endif` a `Conditional`, `define` to `endef` a `Define` (its body one `Raw` token). References (`$(CC)`, `$@`) and quoted strings are single tokens.

Files: `Makefile`, `GNUmakefile`, `makefile`, `*.mk` and `*.mak`.

The grammar is a fork ([../../grammars/tree-sitter-make/FORK.md](../../grammars/tree-sitter-make/FORK.md)); all 59 files of the corpus below parse. Known gap: `A+=b`, `A?=b` and `A!=b` written with no blank before the operator read as the name `A+` and `=`, so the formatter leaves such an assignment as written. `.RECIPEPREFIX` is parsed but recipes must still start with a tab.

## Formatting
- One statement per line group, blank lines kept up to `max_blank_lines` (2), trailing blanks removed.
- Assignments: one space around the operator (`A:=b` is `A := b`, `space_around_assignment`); the value is kept as written, and so are the lines after a continuation.
- Rules: `targets: prerequisites | order-only`, one space after the colon and none before it (`space_before_colon`), `&:` keeps its blank, an inline command stays after its semicolon. A continuation line of a rule or directive is indented one level (`indent_size`, default 2).
- Recipe commands start with exactly one tab and are kept as written, after their prefix (`@`, `-`, `+`); continued lines are kept as written. A command is Shell code (below); the Shell formatter does not run on it.
- Conditionals: the body is indented one level (not inside a recipe, where a body line is a command and starts with the tab); `else`, `else ifeq ...` and `endif` stand at the level of the `ifeq`. The condition is kept as written with its blanks collapsed.
- `define`: the body and its blanks are kept exactly, `endef` goes at the level of `define`.
- A value, directive or command that ends in `\` keeps the line it continues onto, so the next statement is not swallowed.
- Comments stay where they are. The safety check allows only blanks around values, keywords and recipe prefixes to change.

The options follow mbake's (`space_around_assignment`, `space_after_colon` always on, `max_consecutive_empty_lines`, tab recipes); checkmake only lints.

<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `space_around_assignment` | bool | `true` | Write `A := b` rather than `A:=b`. |
| `space_before_colon` | bool | `false` | Write `target : prerequisite` rather than `target: prerequisite`. |
| `max_blank_lines` | int 0-10 | `2` | Largest number of blank lines kept between statements and comments. |
<!-- /generated -->

## Presets
`makefile/checkmake`: turns on every rule that has a checkmake equivalent ([../parity/checkmake.yaml](../parity/checkmake.yaml)) at checkmake's defaults.

## Shell in recipes
Each command of a recipe is one shell invocation in make and one Shell parse here ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#embedded-languages), `src/embedded.rs`): from the command after its prefix characters to its last line. What the shell never sees is carved out and blanked: `$(...)`, `${...}` and `$x` references and calls, the first `$` of `$$`, and the tab that starts each continued line. The shell rules (`cd-without-exit`, the shellcheck ones, the `shell:` section of the config) run on it at their positions in the Makefile, and a `# sudsy-disable-next-line` comment of the Makefile silences them. Shell code in `$(shell ...)` and in `define` bodies is not parsed.

## Rules
Shared: suppression comments (`# sudsy-disable-next-line <rule>`), `no-empty-comment`, `comment-space`, `max-line-length`. The grammar has no `#mapping` or `#sequence` nodes, so the key rules do not apply. The language rules are listed in [../rules/builtin.md](../rules/builtin.md) and are checkmake's: `timestamp-expanded` is on by default; `phony-declared` (it names object files), `unique-targets` (a target may have several rules), `min-phony` (options `required`) and `max-body-length` (`max`) are off; the preset turns all five on. checkmake reads a rule with several targets (`a b:`) as one target named `a b`; Sudsy checks each name. A recipe ends where make's does (blank lines and comments inside it belong to it), so `max-body-length` counts more lines than checkmake in a recipe with a blank line or a column-0 comment.

## Parity
`scripts/parity.sh checkmake` ([../development/parity.md](../development/parity.md#checkmake)) runs checkmake 0.3.2 and `sudsy lint` on the rule fixtures (`tests/rules/makefile/*/*.mk`): 10 findings, none differs but the 4 named in `tests/parity/checkmake/differences.yaml`.

Where Sudsy differs from checkmake by design:

<!-- generated:differences:checkmake -->

- `double-colon-rule`: a repeated `::` rule is allowed, as make does; checkmake reports it
- `empty-recipe`: a rule with an explicit empty recipe (`$(OBJS): ;`) has a body; checkmake reads it as none
- `multi-target-rule`: each target of a rule like `c d:` is checked on its own; checkmake reads one target named `c d`
- `target-specific-variable`: a target-specific variable assignment is a variable; checkmake does not read it as one
<!-- /generated -->

## Conformance
`tests/conformance/fetch-makefile-corpus.sh` fetches the Makefiles of redis, git, lua, prometheus, kubernetes (`build/root`) and docker/cli at pinned commits; `cargo test --profile corpus -p lang-makefile --test integration -- --ignored --nocapture conformance` checks that each file parses (59 files), round-trips, formats with the same tokens (the safety check) and idempotently.

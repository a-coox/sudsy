# GitHub Actions (`lang-github-actions`)

Workflows (`.github/workflows/*.yml`, `*.yaml`) and action metadata (`action.yml`, `action.yaml`) are YAML with their own rules and embedded scripts.

## Selection and provider
A separate provider, `github-actions`, with the YAML grammar: it claims files by path (a `yml` or `yaml` file directly inside a folder named `workflows`, a file named `action.yml` or `action.yaml`; the folder name is not checked against `.github`, the matcher knows one directory name), which beat the extension matcher of `yaml`. Parsing, tree, formatter, hooks and option schema are `lang-yaml`'s, called through the provider (no copy of the grammar, no second parser). Its config base is `yaml` ([../config/sudsy-yaml.md](../config/sudsy-yaml.md#dialects)): the `yaml:` section's options and rule levels apply to workflows and actions, `github-actions:` on top, and every `yaml` rule (pattern rules included) runs besides the rules below. It has no presets of its own.

## Embedded scripts
The `run:` of every step of a workflow's jobs and of a composite action is code of another language ([../architecture/adding-a-language.md](../architecture/adding-a-language.md#embedded-languages)), found by `src/scripts.rs`:
- Which language: the step's `shell:`, else the job's or the workflow's `defaults.run.shell`, else PowerShell on a `runs-on` that names `windows`, else Shell. `bash` and `sh` (with options after the name) are Shell, `pwsh` and `powershell` PowerShell; anything else (`python`, `cmd`, a custom template) is not checked.
- Which text: a single-line plain scalar or a `|` block. A `>` block, a quoted scalar and a multi-line plain scalar fold or escape, so they are not code as written. The block's indentation and every `${{ }}` expression are carved out (blanked to spaces for the parse), so the Shell rules see `echo  ` for `echo ${{ github.sha }}` and report at workflow positions. A script with a here-document is left out when its terminator is indented (the parse would not find the terminator).
- Formatting never touches the script (`format: false`); a workflow's indentation is the YAML formatter's.

## Rules
| Rule | Does |
|---|---|
| `invalid-expression` | Syntax of `${{ }}`: unclosed, empty, brackets, strings, stray characters, unknown context or function. No grammar: operators and operands are not checked. |
| `script-injection` | An attacker-controlled context (issue and pull request titles and bodies, branch names, commit messages, ...) pasted into `run:` or the `script` of `actions/github-script`. |
| `deprecated-commands` | `::set-output`, `::save-state`, `::set-env`, `::add-path` in such a script. |
| `invalid-job-id` | Characters of job ids. |
| `duplicate-ids` | Job ids that differ only in case, repeated step ids of a job. |
| `unknown-needs`, `cyclic-needs` | `needs` of an unknown job; one report at the first job of each cycle. |
| `invalid-runs-on` | A label not matched by the `labels` option (regular expressions; default: the hosted images and `self-hosted`); skipped with `self-hosted`, an expression, or a `group`. |
| `missing-permissions` | Off by default: a job without `permissions` in a workflow without top-level `permissions`. |

Checked against actionlint by `scripts/parity.sh actionlint` ([../development/parity.md](../development/parity.md#actionlint)); the catalog [../parity/actionlint.yaml](../parity/actionlint.yaml) lists what is not done. Not done, offline: expression types and the contexts allowed per key, `if:` conditions that are always true, event and activity names and their filters, globs, matrix keys, permission scopes, runner label conflicts, the structure of the file (required keys, empty sections). Not done, needing the network or other files: the inputs and outputs of the actions a step uses, outdated runners of an action, local actions and reusable workflows.

Where Sudsy differs from actionlint by design:

<!-- generated:differences:actionlint -->

- `needs-location`: an unknown `needs` entry is reported at the entry; actionlint reports it at the job
- `script-line`: a script rule is reported at the line of the text of a block scalar; actionlint reports it at the line of the `|`
- `self-hosted-labels`: a job with `self-hosted` is not checked for custom runner labels; actionlint wants them in its config
<!-- /generated -->

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
| `bracket_spacing` | bool | `true` | Spaces inside the braces of a one-line mapping or object: `{ a: 1 }` or `{a: 1}`. |
| `array_bracket_spacing` | bool | `false` | Spaces inside the brackets of a one-line sequence or array: `[ 1, 2 ]` or `[1, 2]`. |
| `sequence_indent` | `preserve`, `indented`, `flush` | `preserve` | A block sequence that is the value of a mapping key: `indented` puts the dashes one level in, `flush` at the key's column, `preserve` keeps each sequence as written. |
| `quote_style` | `preserve`, `single`, `double`, `prefer_double` | `preserve` | Quote style of quoted scalars. With `single` and `double` a scalar keeps its quotes when the other style would change its value or need more escapes; `prefer_double` uses double quotes, except single quotes for text that contains a `"` (unless it has other escapes, which keep it as written). |
| `document_start` | `preserve`, `always`, `never` | `preserve` | The `---` marker: `always` starts every document with it, `never` leaves it out of the first document when that is possible, `preserve` keeps what is written. |
| `document_end` | `preserve`, `always`, `never` | `preserve` | The `...` marker: `always` ends every document with it, `never` leaves it out unless the next document has a directive or no `---`, or the document is empty, `preserve` keeps what is written. |
| `max_blank_lines` | int 0-10 | `1` | Largest number of blank lines kept between entries, comments and documents. |
| `blank_lines_after_key` | `preserve`, `remove` | `preserve` | Blank lines between a key and its block value on the next line: `preserve` keeps them (up to `max_blank_lines`); `remove` drops them. |
| `comment_spacing` | int 1-10 | `1` | Spaces between a value and a comment after it on the same line. |
<!-- /generated -->

## Conformance
`tests/conformance/fetch-github-actions-corpus.sh` (actionlint's test workflows at v1.7.12, `actions/starter-workflows`), `cargo test --profile corpus -p lang-github-actions --test integration -- --ignored conformance`: every file that parses formats with every check.

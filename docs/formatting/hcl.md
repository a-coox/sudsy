# HCL (`lang-hcl`)

## Parser
tree-sitter-hcl 1.1 (HCL2: Terraform, OpenTofu, Packer, Nomad, Consul, Terragrunt) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is the `mapping` of `crates/lang-hcl/lang.yaml`, the tree shape `grammars/hcl.grammar`. Expressions are flat: an `Expr` holds its operands, operators and brackets, and the constructs tree-sitter makes nodes of (call, conditional, `for`, tuple, object, template, attribute access, index) are nodes inside it. A string with no interpolation is one `String` token; a string with `${}` or `%{}` is a `QuotedTemplate` node, a heredoc a `Heredoc` node.

Files: `*.tf`, `*.tfvars`, `*.hcl` (also `*.pkr.hcl`, `*.tftest.hcl`), `*.nomad`. A `.tf.json` file is JSON. The shared comment rules know the `#` marker only (`//` and `/* */` comments are trivia like it). Tree-sitter reports syntax errors ("syntax error", "missing X"); a valid file never gets one, with one exception: `a . *` (blanks inside the legacy splat) does not parse. `.tfvars` in JSON syntax (a root object) parses as an `Object`.

## Formatting
The layout is `terraform fmt`'s (hashicorp/hcl `hclwrite.Format`, which `tofu fmt`, `packer fmt` and `nomad fmt` share), with no style options of its own. It works on the token stream, one source line at a time, and never joins or splits lines:
- Indentation: two spaces per level of open brackets (`{`, `[`, `(`, `${`, `%{`): a line that opens more than it closes indents the lines after it, and the line that closes them is dedented. Brackets opened on one line and closed on another count per line, so a continuation line inside `(` sits one level in even when it is not in a block.
- Spaces: one space between tokens, except none inside `( )` and `[ ]`, before `,` and `...`, around `.`, between a function name or a `[` and what comes before it, after `!` and a sign, inside `${ }` and `%{ }`; `{ a = 1 }` has spaces inside, `{}` has none. The text of strings and heredocs is kept (the code inside `${ }` of a string or heredoc is spaced like any code).
- Alignment: in a run of consecutive lines that each hold a whole `name = value` (a value that opens a bracket it does not close, as `a = {`, ends the run), the `=` lines up one space after the longest name; in a run of consecutive lines with a trailing `#` or `//` comment, the comments line up one space after the longest line.
- Blank lines (any number, also at the start and end of the file), comments and line breaks inside expressions are kept; a missing final newline is added. Blanks before a line break go.
- `sudsy-ignore` comments and layout directives are not honored: the layout is token-wise and has nothing to attach them to.
- Not done, because the formatter never changes a token: `terraform fmt` also rewrites `"${x}"` to `x` and quotes block labels written as bare names; the first is the rule `deprecated-interpolation` and its fix.

## Parity
`scripts/parity.sh format terraform` compares with `terraform fmt` 1.17.1 (downloaded on first use, `docs/parity/terraform.yaml`) on `tests/parity/terraform/` and a corpus of real files (`tests/conformance/fetch-hcl-corpus.sh`: the Terraform modules terraform-aws-vpc and terraform-aws-eks, Terraform's own fmt and config test data, Packer's examples and test templates; Nomad job files are in the corpus too and checked below). Sudsy runs `fix` there (with `tests/parity/terraform/sudsy.yaml`, which turns off the other rules), because `terraform fmt` unwraps `"${x}"` itself. Result (terraform 1.17.1): 312 of 318 files are identical; the other 6 are named in `tests/parity/terraform/differences.yaml`. The 243 Nomad job files of the corpus (HCL2 ones; the older HCL1 files do not parse in either tool) are not in the script, since terraform unwraps the `"${attr.kernel.name}"` Nomad fills in at run time; with the rule's fix on (the files renamed `.tf` in an ad hoc run) 225 are identical, and the other 18 differ in the cases above and in a space after a heredoc's closing marker, which terraform keeps and Sudsy removes.

Where Sudsy differs from terraform by design:

<!-- generated:differences:terraform -->

- `interpolation-in-list`: terraform fmt unwraps `"${x}"` only as a whole argument; the rule `deprecated-interpolation` also fixes it inside a list
- `semantic-error`: terraform rejects a file for a semantic error (a repeated plugin name); Sudsy formats it
- `token-rewrite`: terraform fmt rewrites tokens (it quotes a bare block label, `list` becomes `list(any)`); Sudsy keeps the tokens
<!-- /generated -->
`cargo test --profile corpus -p lang-hcl --test integration -- --ignored --nocapture conformance` checks every corpus file: parses without errors, round-trips, formats with the same tokens and idempotently.

## Lint
The rules are tflint's (tflint-ruleset-terraform) that need no provider and no other file; `preset:hcl/tflint-recommended` turns on the ones that are on in tflint's recommended preset. On by default: `deprecated-interpolation` (with a fix; not in Nomad job files, `.nomad` and `.nomad.hcl`, whose `"${attr.kernel.name}"` the agent fills in at run time), `deprecated-index` (with a fix), `empty-list-equality`, and the shared `no-duplicate-keys` (an argument repeated in a block, a key in an object). Off: `typed-variables`, `documented-variables`, `documented-outputs`, `required-version`, `required-providers`, `naming-convention` (option `format`), `comment-syntax`, `unused-declarations`. `required-version`, `required-providers` and `unused-declarations` read one file: use them on the file that holds the `terraform` block or the whole module. Most are pattern files in `crates/lang-hcl/rules/`; `src/rules.rs` has the ones that build a fix or read several nodes.
- Not covered, listed with their reasons in [../parity/tflint.yaml](../parity/tflint.yaml): the rules about module sources and versions, `terraform_deprecated_lookup`, `.tf.json` syntax, and those that need the other files of the module or the providers.

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

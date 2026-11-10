# GraphQL (`lang-graphql`)

## Parser
tree-sitter-graphql 0.3 (joowani's maintained fork of bkegley's grammar, MIT) linked in natively and converted by `core-treesitter` ([../architecture/adding-a-language.md](../architecture/adding-a-language.md)); the node mapping is `crates/lang-graphql/src/mapping.rs`, the tree shape `grammars/graphql.grammar`. A `Document` holds the definitions (operations, fragments, the schema, types, directive definitions, extensions); names, values, `$variables` and directive locations are one token each, a type reference (`[A!]!`) is a `TypeRef` of tokens, and commas are whitespace (GraphQL ignores them).

Files: `*.graphql`, `*.gql`. Tree-sitter reports syntax errors ("syntax error", "missing X"); a file with one is not formatted.

Grammar gaps (valid GraphQL the grammar rejects; such a file is refused like any syntax error): more than one directive on a field (`a @skip(if: $x) @include(if: $y)`), a backslash in a one-line string (`"a \" b"`, `"\n"`) or `\"""` in a block string, `extend schema @dir` without braces, a description on an operation or a variable, directives on a directive definition, fragment variables, empty selection sets, and an empty document (no definitions, or only comments). Fixing them needs a fork of the grammar with a regenerated `parser.c`.

## Formatting
Layout follows Prettier's GraphQL printer (`scripts/parity.sh format prettier` compares them): two-space indentation, one definition, field, enum value or argument definition per line, blank lines kept (at most one), comments kept; commas dropped.
- Argument lists, variable definitions, argument definitions and list and object values stay on one line (`f(a: 1, b: 2)`, `{ a: 1 }`, `[1, 2]`) when they fit `line_width`, else one item per line, no commas; `bracket_spacing` controls the spaces inside `{ }` of object values.
- Directives follow their subject on the same line when they fit, else one per line, indented (after an operation or fragment name the first stays on the line).
- `implements A & B` (leading `&` dropped); a union is `union U = A | B`, or when too long `union U =` with `| A` per line; directive locations on one line, `on A | B`.
- Block strings (`"""`) are re-indented to the nesting level (graphql-js's rule for the common indentation, leading and trailing blank lines dropped); a one-line value is trimmed, an all-blank one is `""" """`-free (`"""` and `"""` on their own lines). Other strings are kept as written (Prettier decodes `\u` escapes).
- A description goes on the line above its definition.

## Parity
`scripts/parity.sh format prettier` compares with Prettier 3.9.10 (`docs/parity/prettier.yaml`) on 132 GraphQL files with Sudsy's defaults (`tests/conformance/fetch-graphql-corpus.sh`: Prettier's own format tests, graphql-js, GraphiQL, Apollo Federation, graphql-eslint; the 368 KB GitHub schema among them): 105 are identical. The other 27 are listed with their reasons in `tests/parity/prettier/expected-diff.txt`: 24 hold syntax the grammar rejects (gaps above) or Prettier rejects too (empty selection sets, range markers), 2 use `# prettier-ignore`, 1 has comments between the interfaces and the body. 108 of the 132 parse without errors; every file that parses formats with the same tokens and idempotently (`cargo test --profile corpus -p lang-graphql --test graphql -- --ignored --nocapture conformance`).

## Rules
Rules of graphql-eslint that need no schema and no other file (`docs/parity/graphql-eslint.yaml` lists every rule and its status):
- `naming-convention` (options per kind of name), `no-anonymous-operations`, `no-typename-prefix`, `selection-set-depth` (`max_depth`), `unique-operation-name` and `unique-fragment-name` (within the file).
- Shared rules through the grammar tags: `no-duplicate-keys` (graphql-eslint `no-duplicate-fields`, `unique-argument-names`, `unique-input-field-names`, `unique-variable-names`, `unique-enum-value-names`: duplicate fields, arguments, object fields, variables, enum values and definitions in a selection set or body; a field with an alias counts under the alias), `sorted-keys` (`alphabetize`, which graphql-eslint configures per kind: here the shared rule's options apply to every list) and `key-name-casing` (all lists at once; prefer `naming-convention`).

## Options
<!-- generated:options -->
| Option | Values | Default | Description |
|---|---|---|---|
| `indent_size` | int 1-16 | `2` | Columns per indentation level (also the width of a tab). |
| `indent_style` | `space`, `tab` | `space` | Indent with spaces or tabs. |
| `line_width` | int 20-1000 | `80` | Preferred maximum line width in columns. |
| `bracket_spacing` | bool | `true` | Spaces inside the braces of a one-line mapping or object: `{ a: 1 }` or `{a: 1}`. |
| `final_newline` | bool | `true` | End the file with a newline. |
| `end_of_line` | `auto`, `lf`, `crlf` | `auto` | Line break style of the output: `lf`, `crlf`, or `auto` to keep the file's first line break. |
<!-- /generated -->

# Fork of tree-sitter-graphql

Upstream: crates.io `tree-sitter-graphql` 0.3.0 (joowani/tree-sitter-graphql, MIT, [LICENSE](LICENSE)). Regenerate `src/` after editing `grammar.js` with `scripts/grammar-regen.sh graphql`.

`bindings/rust` is trimmed to the language constant (no queries, scanner or doctest) so the workspace's fmt and clippy pass; `tree-sitter generate` does not touch it.

Changes to `grammar.js` (valid GraphQL the upstream grammar rejected):
- `field`: `directives` (any number) instead of one `directive`.
- `string_value`: `\` escapes in one-line strings (`"a \" b"`); `\"""` in block strings.
- `schema_extension`: `extend schema @dir` without braces; several root operations in the braces; `prec.right` prefers `{` as the extension's body.
- `operation_definition`, `fragment_definition`, `variable_definition`: optional `description`.
- `fragment_definition`: optional `variable_definitions`; `fragment_spread`: optional `arguments` (fragment arguments).
- `directive_definition`: optional `directives` after the arguments.
- `directive_extension`: `extend directive @a @b` (a `type_extension` alternative).
- `source_file`: optional `document` (an empty or comment-only file).

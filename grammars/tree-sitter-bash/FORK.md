# Fork of tree-sitter-bash

Upstream: crates.io `tree-sitter-bash` 0.25.1 (tree-sitter/tree-sitter-bash, MIT, [LICENSE](LICENSE)). Regenerate `src/parser.c` after editing `grammar.js` with `scripts/grammar-regen.sh bash`; `src/scanner.c` is edited by hand.

`bindings/rust` is trimmed to the language constant (no queries, node types or doctest) so the workspace's fmt and clippy pass; `tree-sitter generate` does not touch it.

Changes (valid Bash the upstream grammar rejects or mis-parses):
- `grammar.js` `for_statement`: the terminator before `do` is optional without `in` (`for x do`).
- `grammar.js` `concatenation`: a word may end in `$` after a single part (`grep -q 404$; then`).
- `grammar.js` `variable_assignments`: dynamic precedence 1, so `a=1 b=2` + line break + `cmd` is two statements, not one command.
- `grammar.js` `_extglob_blob` and `src/scanner.c` (extglob scan): a single-quoted string inside a case pattern (`--only=*' '*)`, `*\'*`).
- `grammar.js` `redirected_statement`: a here-string may follow other redirects (`cat 2>&1 <<< "x"`).

# Fork of tree-sitter-make

Upstream: crates.io `tree-sitter-make` 1.1.1 (tree-sitter-grammars/tree-sitter-make, MIT, [LICENSE](LICENSE)). Regenerate `src/` after editing `grammar.js` with `scripts/grammar-regen.sh make`.

`bindings/rust` is trimmed to the language constant (no queries or doctest) so the workspace's fmt and clippy pass; `tree-sitter generate` does not touch it.

Changes to `grammar.js` (valid makefiles the upstream grammar rejected):
- `function_call`: the blank after the function name belongs to the name token, so a variable named like a function (`$(dir)`, `$(info)`) is a reference.
- `arguments`: an argument may be empty (`$(if $(a),-b,)`).
- `text`: balanced parentheses (`_paren_group`) in a value, an argument or a `$(shell ...)` (`SOURCES = ( \ ... )`, `$(warning goal(s))`).
- `$$` no longer starts a reference or call (`variable_reference`, `substitution_reference`, `automatic_variable`, `function_call`, `shell_function`): it is the escape of a `$`, so `$$(cmd args)` in a recipe is shell text.
- `substitution_reference`: an automatic variable as the text (`$(@:%.o=%.c)`).

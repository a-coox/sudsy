# Fork of tree-sitter-ini

Upstream: crates.io `tree-sitter-ini` 1.4.0 (justinmk/tree-sitter-ini, Apache-2.0, [LICENSE](LICENSE)). Regenerate `src/parser.c` after editing `grammar.js` with `scripts/grammar-regen.sh ini`; `src/scanner.c` is edited by hand.

`bindings/rust` is trimmed to the language constant (no queries or doctest) so the workspace's fmt and clippy pass; `tree-sitter generate` does not touch it.

Changes (valid INI the upstream grammar rejects):
- `grammar.js` `setting`: a key without `=` (`.gitconfig` booleans, bare lines).
- `grammar.js` `setting`, `_continuation` and `src/scanner.c`: indented lines after a flush-left `key =` setting are `continuation` nodes (`tox.ini`, `setup.cfg`); an indented setting (`.gitconfig`) starts with a hidden `_indent` token and owns no continuation lines, which is how the two are told apart. The scanner emits both, since whitespace is an extra the lexer skips before any regex token could see it; an indented comment line is neither.
- `grammar.js` `comment` and `src/scanner.c`: the comment's line break is the external `_comment_end`, which also matches the end of the file, so a comment on the last line without a final newline parses.
- `grammar.js` `comment`: the line break is not part of the comment (the `_blank` extra takes it), so a comment on the last line without a final newline parses.

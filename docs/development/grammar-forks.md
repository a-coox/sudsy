# Grammar forks

A tree-sitter grammar that rejects valid input is forked into `grammars/tree-sitter-<lang>/`: the upstream crate's source (`grammar.js`, `src/`, `bindings/rust`, `tree-sitter.json`, `LICENSE`), a path-crate `Cargo.toml`, and `FORK.md` (upstream version, each change and why). The language's `lang.yaml` names it with `grammar-path: grammars/tree-sitter-<lang>` instead of `grammar-version`; `cargo xtask codegen` then writes a path dependency in the root `Cargo.toml`.

- Keep patches small, and list each one in `FORK.md`.
- `scripts/grammar-regen.sh <lang>` runs `tree-sitter generate` in the fork. It needs `tree-sitter` 0.25.10 (pinned in the script; it runs `cargo install tree-sitter-cli --locked --version 0.25.10` when the version differs). The CLI is for developers only: build, gate and CI compile the committed `src/parser.c`, so commit it with the `grammar.js` change.
- Then adjust `grammars/<lang>.grammar`, the mapping and the formatter for the changed node shapes.

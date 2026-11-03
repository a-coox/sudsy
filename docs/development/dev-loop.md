# Dev loop

## Commands
`cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `dotnet build workers/csharp`, `dotnet test workers/csharp`, `cargo xtask codegen [--check]` (alias in `.cargo/config.toml`). Rust tests that need the worker build it with `dotnet build` on first use; set `SUDSY_CSHARP_WORKER` to use a prebuilt (e.g. Release) worker.

## Scripts (`scripts/`)
| Script | Use |
|---|---|
| `gate.sh [--rust-only] [-p crate]` | The pre-commit gate: fmt, build, clippy, tests, codegen check, sample plugin fmt+tests, dotnet build+test, vscode typecheck/lint/test/build. `-p` limits to one crate. Must pass before committing. |
| `ship.sh` | Push the current branch, then watch the CI run for HEAD (`gh run watch`); refuses with staged changes |
| `dogfood.sh` | Build debug CLI + worker into `target/dogfood/` for the editor |
| `bench.sh`, `bench-time.sh`, `bench-measure.sh`, `bench-setup-wsl.sh` | [performance/tooling.md](../performance/tooling.md) |
| `parity.sh <tool>` | Differential test against an upstream tool on `tests/parity/<tool>/` fixtures: [parity.md](parity.md) |
| `check-version.sh [vX.Y.Z]` | Cargo workspace, `editors/vscode/package.json` + lock, tag and `CHANGELOG.md` agree |
| `package-cli.sh` | CLI archive: executable, `csharp-worker/`, LICENSE, CHANGELOG |

## Conventions
- Errors: `thiserror` in libraries, `anyhow` only in binaries (`cli`, `lsp`). `unsafe_code = "forbid"`.
- Snapshots use `insta`. After a decision, delete the losing alternatives; results live in these docs and git history.
- Docs describe the current state; update the relevant file in the same commit as the change.

## Windows / Git Bash / WSL
- Shell is Git Bash (`.vscode/settings.json` sets it); scripts are bash. Some tools mangle paths: use `cygpath` and `MSYS_NO_PATHCONV=1` when calling `wsl`.
- Instruction-count benchmarks need valgrind, so they run in WSL (`BENCH_WSL_DISTRO`, default Ubuntu); `bench.sh` converts paths. A VS Code window locks `target/debug` outputs, hence `target/dogfood/`.
- CSharpier comparisons copy the corpus outside the repo (`SUDSY_DIFF_DIR`) because CSharpier honors `.gitignore`.
- Environment variables: `SUDSY_CSHARP_WORKER`, `SUDSY_CSHARP_WORKERS`, `SUDSY_WASM_CACHE`, `SUDSY_DIFF_DIR`, `SUDSY_DIFF_FILTER`, `SUDSY_CORPUS_FILTER`, `SUDSY_CORPUS_FAILURES`, `SUDSY_SPEED_DIR`.

# Dev loop

## Commands
`cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `dotnet build workers/csharp`, `dotnet test workers/csharp`, `cargo xtask codegen [--check]` (alias in `.cargo/config.toml`; writes the generated files, including the rule and option tables of `docs/rules/builtin.md` and `docs/formatting/*.md`). Rust tests that need the worker build it with `dotnet build` on first use; set `SUDSY_CSHARP_WORKER` to use a prebuilt (e.g. Release) worker.

## Scripts (`scripts/`)
| Script | Use |
|---|---|
| `gate.sh [--rust-only] [-p crate]` | The pre-commit gate (prints each step's seconds; a step is killed after `GATE_TIMEOUT` seconds, default 1800): fmt, clippy, tests (`cargo nextest run`, which runs the test binaries in parallel; needs `cargo install cargo-nextest --locked`; then the doctests), codegen check, sample plugin fmt+tests, dotnet build+test, vscode typecheck/lint/test/build. `-p` limits to one crate. Must pass before committing. |
| `ship.sh` | Push the current branch, then watch the CI run for HEAD (`gh run watch`); refuses with staged changes; CI is manual for now (see ci-release.md), so it stops after the push |
| `dogfood.sh` | Build debug CLI + worker into `target/dogfood/` for the editor |
| `bench.sh`, `bench-time.sh`, `bench-measure.sh`, `bench-setup-wsl.sh` | [performance/tooling.md](../performance/tooling.md) |
| `parity.sh <tool>`, `parity.sh format <tool>` | Differential test against an upstream tool on `tests/parity/<tool>/` fixtures: [parity.md](parity.md) |
| `pack.sh <out> [language...]`, `pack-bench.sh` | Build language packs and their manifest for this machine; static vs pack timing and sizes ([../architecture/language-packs.md](../architecture/language-packs.md)) |
| `finish.sh [bench args]` | The check of a finished change: the full gate and `bench.sh` at the same time (about as long as the slower one); a gate failure stops the bench; ends in `FINISH OK` or `FINISH FAILED: <gate\|bench>` |
| `agent-stats.py [<subagents dir>] [N]` | Summary of the newest N (10) Claude Code subagent transcripts: model vs tool minutes, gate/bench launches, watchers, errors by kind, the commit whose CLAUDE.md each agent was given |
| `check-version.sh [vX.Y.Z]` | Cargo workspace, `editors/vscode/package.json` + lock, tag and `CHANGELOG.md` agree |
| `package-cli.sh` | CLI archive: executable, `csharp-worker/`, LICENSE, CHANGELOG |

## Parallel agents
Each parallel agent works in its own git worktree and sets `CARGO_TARGET_DIR=C:/Coding/.sudsy-targets/slot<N>` (outside the repo; one slot per running agent, assigned by whoever starts the agents). A slot outlives worktrees, so a new worktree in a slot starts warm (wasmtime and cranelift are not rebuilt). Every script honors `CARGO_TARGET_DIR`: the gate's cargo steps, `parity.sh` (binary and tool installs under `<slot>/parity/`), `bench.sh` (native work dir and base worktree under `<slot>/bench/`; the WSL callgrind cache `~/.cache/sudsy-bench/<slot name>` is keyed by the slot), `dogfood.sh`. The gate builds the sample plugin's own workspace in `<slot>/sample-host/` (two workspaces in one deps directory confuse rustc). Unset, everything uses `target/`.
- Iterate with `scripts/gate.sh -p <crate>`; run the full `scripts/gate.sh` once before pushing, and again only if the rebase brought conflicts.
- Run `scripts/bench.sh` once at the end of a batch, not mid-work.
- A slot serves one run at a time: `gate.sh` and `finish.sh` first stop what an earlier run left there, found by the `SUDSY_SLOT_RUN=<slot>` marker every process of a run inherits (`scripts/slot-run.sh`), because under Git Bash a cancelled background task leaves its shells, cargo and tests running and holding the slot's build lock. The first gate of a worktree prunes the slot alongside its steps.
- Build caches only grow: cargo never deletes incremental data, and every worktree path gets new file hashes, so each worktree that used a slot leaves a full set of test binaries and debug info behind (130 GB of 240 in one slot). `scripts/clean-slots.sh [--all] [slot dir...]` deletes the `incremental/` caches and all but the newest copy of each binary in `target/` and every slot (or the slots given), and the same in the slots' WSL bench caches (`--all`: whole `debug/` dirs); the gate warns when the build disk has under 30 GB free.

## Conventions
- Errors: `thiserror` in libraries, `anyhow` only in binaries (`cli`, `lsp`). `unsafe_code = "forbid"`.
- `.cargo/config.toml` points `TEMP`/`TMP`/`TMPDIR` at `.cargo/tmp` (inside the repo, so no `.gitignore` there: the project walk honors it), so `tempfile` dirs never sit below a stray `sudsy.yaml` of the system temp dir.
- Snapshots use `insta`. After a decision, delete the losing alternatives; results live in these docs and git history.
- Docs describe the current state; update the relevant file in the same commit as the change.

## Windows / Git Bash / WSL
- Shell is Git Bash (`.vscode/settings.json` sets it); scripts are bash. Some tools mangle paths: use `cygpath` and `MSYS_NO_PATHCONV=1` when calling `wsl`.
- Instruction-count benchmarks need valgrind, so they run in WSL (`BENCH_WSL_DISTRO`, default Ubuntu); `bench.sh` converts paths. A VS Code window locks `target/debug` outputs, hence `target/dogfood/`.
- CSharpier comparisons copy the corpus outside the repo (`SUDSY_DIFF_DIR`) because CSharpier honors `.gitignore`.
- Environment variables: `SUDSY_CACHE_DIR`, `SUDSY_PACK_MANIFEST` (build time), `SUDSY_CSHARP_WORKER`, `SUDSY_CSHARP_WORKERS`, `SUDSY_WASM_CACHE`, `SUDSY_DIFF_DIR`, `SUDSY_DIFF_FILTER`, `SUDSY_CORPUS_FILTER`, `SUDSY_CORPUS_FAILURES`, `SUDSY_SPEED_DIR`.

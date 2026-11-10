# Dev loop

## Commands
`cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all`, `dotnet build workers/csharp`, `dotnet test workers/csharp`, `cargo xtask codegen [--check]` (alias in `.cargo/config.toml`; writes the generated files, including the rule and option tables of `docs/rules/builtin.md` and `docs/formatting/*.md`). Rust tests that need the worker build it with `dotnet build` on first use; set `SUDSY_CSHARP_WORKER` to use a prebuilt (e.g. Release) worker.

## Scripts (`scripts/`)
| Script | Use |
|---|---|
| `gate.sh [--rust-only] [--affected [--base REV]] [-p crate]...` | The pre-commit gate (prints each step's seconds; a step is killed after `GATE_TIMEOUT` seconds, default 1800): fmt, clippy, tests (`cargo nextest run`, which runs the test binaries in parallel; needs `cargo install cargo-nextest --locked`; then the doctests), codegen check, sample plugin fmt+tests, each worker's `workers/<name>/check.sh` (C#: dotnet build+test), vscode typecheck/lint/test/build. `--affected`: only what the change selects (below). `-p` (repeatable) limits clippy and tests to those crates and skips the other steps. |
| `cargo xtask affected [--base REV] [--all]` | What a change needs checked, from its diff against REV (default the merge-base with origin/main; uncommitted and untracked files included): see [Affected checks](#affected-checks) |
| `ship.sh` | Push the current branch, then watch the CI run for HEAD (`gh run watch`); refuses with staged changes |
| `dogfood.sh` | Build debug CLI + worker into `target/dogfood/` for the editor |
| `bench.sh`, `bench-time.sh`, `bench-measure.sh`, `bench-setup-wsl.sh` | [performance/tooling.md](../performance/tooling.md) |
| `parity.sh <tool>`, `parity.sh format <tool>` | Differential test against an upstream tool on `tests/parity/<tool>/` fixtures: [parity.md](parity.md) |
| `pack.sh <out> [language...]`, `pack-bench.sh` | Build language packs and their manifest for this machine; static vs pack timing and sizes ([../architecture/language-packs.md](../architecture/language-packs.md)) |
| `finish.sh [bench args]` | The check of a finished change: `gate.sh --affected` and `bench.sh` (which measures the affected languages) at the same time (about as long as the slower one); `--all`: the full gate and every language; a gate failure stops the bench; ends in `FINISH OK` or `FINISH FAILED: <gate\|bench>` |
| `agent-stats.py [<subagents dir>] [N]` | Summary of the newest N (10) Claude Code subagent transcripts: model vs tool minutes, gate/bench launches, watchers, errors by kind, the commit whose CLAUDE.md each agent was given |
| `check-version.sh [vX.Y.Z]` | Cargo workspace, `editors/vscode/package.json` + lock, tag and `CHANGELOG.md` agree |
| `package-cli.sh` | CLI archive: executable, `csharp-worker/`, LICENSE, CHANGELOG |

## Parallel agents
Each parallel agent works in its own git worktree and sets `CARGO_TARGET_DIR=C:/Coding/.sudsy-targets/slot<N>` (outside the repo; one slot per running agent, assigned by whoever starts the agents). A slot outlives worktrees, so a new worktree in a slot starts warm (wasmtime and cranelift are not rebuilt). Every script honors `CARGO_TARGET_DIR`: the gate's cargo steps, `parity.sh` (binary and tool installs under `<slot>/parity/`), `bench.sh` (native work dir and base worktree under `<slot>/bench/`; the WSL callgrind cache `~/.cache/sudsy-bench/<slot name>` is keyed by the slot), `dogfood.sh`. The gate builds the sample plugin's own workspace in `<slot>/sample-host/` (two workspaces in one deps directory confuse rustc). Unset, everything uses `target/`.
- Iterate with `scripts/gate.sh -p <crate>`; run `scripts/finish.sh` once before pushing, and again only if the rebase brought conflicts.
- After a batch of merges, the lead runs `scripts/finish.sh --all` on main: a path the selection misses shows there.
- A slot serves one run at a time: `gate.sh` and `finish.sh` first stop what an earlier run left there, found by the `SUDSY_SLOT_RUN=<slot>` marker every process of a run inherits (`scripts/slot-run.sh`), because under Git Bash a cancelled background task leaves its shells, cargo and tests running and holding the slot's build lock.- `finish.sh` checks formatting before the seat and `cargo xtask codegen --check` right after it, before the gate and bench start: the misses that otherwise cost a seat wait or a whole test run.
- `finish.sh` takes a seat first: at most `SUDSY_FINISH_SEATS` (default 2) finish runs build at once across all slots; the others print who is ahead and wait. Seats are files in `locks/` next to the slots, freed when the run ends or its process is gone. `scripts/seat-check.sh` is the self-check.
- Build caches only grow: cargo never deletes outputs, and every dependency graph (each `Cargo.lock` change) and build kind leaves a full set of test binaries and debug info behind. Output names do not depend on the checkout path, so a slot's builds serve every worktree that uses it. At most once an hour, before anything builds, `gate.sh` and `finish.sh` cut the slot to `SUDSY_SLOT_MAX` (default 120GB) and its WSL bench builds to `SUDSY_SLOT_WSL_MAX` (30GB) with `cargo-sweep` (install: `cargo install cargo-sweep --locked`, in Windows and WSL), which removes the outputs builds read least recently, and delete the incremental caches of crates nothing compiled for 3 days (`sweep_slot` in `scripts/slot-run.sh`, log `<slot>/sweep.log`; about 40 s, 9 of it on the Windows side). `scripts/clean-slots.sh [--all] [slot dir...]` frees more by hand: every `incremental/` cache (`--all`: whole `debug/` dirs) in `target/` and the slots and their WSL bench caches; the gate warns when the build disk has under 30 GB free.

## Affected checks
`cargo xtask affected` (`xtask/full/src/affected.rs`) computes the selection on every run from the workspace itself; nothing is stored. It prints `crates <name>...` (or `crates all`, after a `full <reason>` line), `steps <step>...` and one `bench <folder> <crate|*>` line per `bench/inputs` folder to measure.
- A crate is affected when a changed file is in its folder; when it names a changed path outside its folder in a string literal (`"../../tests/fixtures"`, `"workers/csharp/Sudsy.Worker"`, `include_str!`, `bindgen!` paths: read at build or test time) or as a path dependency (the sample plugin); when it depends (normal or build) on an affected crate; and, one level only, when it dev-depends on one (its tests use it; its dependents do not).
- The root `Cargo.lock` selects the members whose locked dependencies changed; the root `Cargo.toml` selects the crates using a changed `[workspace.dependencies]` entry (members list changes select nothing of their own). So adding a language selects its crate and its users (`languages`, cli, lsp, perf-bench), not everything.
- Everything runs when the change touches the rest of the build set-up (other root `Cargo.toml` entries such as profiles and lints, `rust-toolchain.toml`, `.cargo/config.toml`, `xtask/`, the gate, finish and bench scripts) or a path no crate claims. `docs/`, `scripts/` (other than those), `site/`, `.github/`, `tests/parity/` (read by `parity.sh`) and the top-level Markdown files select nothing; `bench/history.jsonl` and `.cargo/tmp/` are ignored.
- Steps: `codegen` for any change; `sample-plugin` when `plugin-sdk` is affected or `tests/plugins/sample/` changed; `worker:<name>` when `workers/<name>/` changed (the gate runs its `check.sh`); `vscode` when `editors/vscode/` changed.
- Bench: a folder whose provider's crate (`LanguageProvider::crate_name`) is affected, or the crate of a language its inputs embed (found by parsing them with their host and resolving the regions it reports, as the engine does; a Markdown fence of shell selects `md` on a `lang-shell` change), or whose inputs changed; every folder when `tools/perf-bench` changed; `startup` when `languages` is affected (it builds every provider).

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

# Sudsy

Formatter + linter for C#, JSON and YAML. `docs/` describes how everything works now (index: `docs/README.md`); read the docs for the area you change before changing code.

## Layout
- Cargo workspace in `crates/` (see docs/architecture/overview.md). Language crates depend on core crates, never the reverse.
- .NET worker in `workers/csharp/` (net10.0, roll-forward). Grammars in `grammars/`, WIT in `wit/`, VS Code extension in `editors/vscode/`.

## Conventions
- Code is cost. Every line, test fixture and doc line costs agent tokens to write, to read on every later task, and to update on every change. Before adding, look for what to reuse or generalize: a shared rule over grammar tags, a pattern rule (YAML) or a data row of a rule family before a Rust rule, an existing helper before a new one. Fixtures are small and focused on the rule's own cases; never copy a shared input into many folders. When a second or third copy of something appears, generalize it instead.
- Small focused modules; extract helpers instead of growing entrypoints. No duplicated logic across crates: shared behavior lives in a core crate. Code of one language embedded in another (shell in a Dockerfile `RUN`, code in a Markdown fence) is parsed with that language's own grammar and checked by its own rules, never re-read by hand in the host language.
- Errors: `thiserror` in libraries, `anyhow` only in binaries (`cli`, `lsp`).
- Snapshot tests use `insta`; fixtures live under `tests/fixtures/<lang>/` or the crate's `tests/`.
- Every formatter test checks lossless round-trip, idempotency and meaning preservation.
- Clean up as you go: once a decision is made, delete the losing alternatives (benchmark candidates, prototypes, bootstrap code); once a temporary placement or shim is superseded (code living somewhere "until X exists"), move or remove it when X exists. Results stay in git history.
- Presets are pure config data. Formatter and rule code never branches on a preset or tool name (no "csharpier mode", no hidden preset-only values). Each style difference is either default behavior or a named, user-settable option that is documented in the schema, so any preset can be reproduced by writing its options in sudsy.yaml by hand.
- Measure performance of everything you add or change on a hot path (load, parse, format, lint, plugins, editor requests): record method, machine and numbers in `docs/performance/numbers.md` (latest values only), behind a script or `perf-bench` mode so it can be re-run. Unmeasured is unfinished.
- No speculative features: build what `docs/roadmap.md` does not defer and the task needs (style options needed to express a preset are not speculative). When a detail is open, pick the simplest option.
- Docs describe current state, not history: update the relevant `docs/` file in the same commit as the change (behavior, options, numbers, roadmap items). No milestone names or "was/used to" prose.

## Working as an agent
- Own worktree and build slot (`CARGO_TARGET_DIR`, see `docs/development/dev-loop.md`); write nothing outside them (not `%TEMP%`, not another slot). Don't kill processes you did not start.
- `scripts/gate.sh -p <crate>` while iterating, for every crate you changed (a registration change also touches `cli`), plus `cargo xtask codegen --check` when you changed lang.yaml, grammars or registration. finish.sh confirms a change that already passes those, it is not where you find compile, lint or test errors: one failed run costs 3-30 min. When the change is done: `scripts/finish.sh` once (gate and bench at the same time on the uncommitted tree, both on what the change affects; a gate failure stops the bench), and no edits while it runs. It passes: commit the change together with the line bench appended to `bench/history.jsonl`. The gate fails: `git checkout bench/history.jsonl`, fix, run it again. Check exit statuses (no piping into `tail`); never commit or push after a failure. A bench regression is a failure even when it is the change's intended work (a new language's startup, new checks): keep the bench line, stop and report the metrics and numbers; accepted, commit with that line (no second finish) and name the accepted metrics in the commit message; rejected, `git checkout bench/history.jsonl` and fix. A budget in the task prompt (for example "format metrics up to +3%") is accepted in advance: within it, commit and name the metrics without stopping. Rebase onto origin/main and fast-forward push. No tags or releases.
- Long steps (finish ~5 min, gate, bench, parity, corpus runs): one Bash call with `run_in_background`, holding only the command (`CARGO_TARGET_DIR=<slot> scripts/finish.sh`: no `export`, redirect or `echo exit`). Then keep working or end your turn: the completion notice wakes you with the exit status and the harness's output file, whose tail you read then (finish ends in `FINISH OK` or `FINISH FAILED: <gate|bench>`). No Monitor, until-loop or sleep, and never a second launch of a running step. To abandon a bench, `scripts/bench.sh --stop`: killing its task leaves its builds and WSL runs going.
- Shell commands: you already run in your worktree, so no `cd <worktree> &&`. One plain command per call: the worktree guard refuses loops, heredocs, `$(...)`, `$((...))` and variables around git, sed, wsl, pwsh or gh. Multi-step work goes in a script file in your slot, written with Write and run with `bash <file>`.
- Parity runs report real differences: never leave out or change an input to keep a run at 0; every known difference is a named `gap` (catalog or `differences.yaml`).
- Files are LF: edit with the Edit/Write tools; a script that writes files uses `open(p, 'w', newline='\n')` (Python on Windows writes CRLF otherwise, and git then warns "CRLF will be replaced by LF").
- Commit messages: a subject saying what changed, then 2-5 lines on why and what the diff does not show (behaviour change, trade-off, numbers); end with the Co-Authored-By trailer.
- Final report: what changed, measured results, `scripts/cost.sh` output, commits, open items.

## Commands
Run scripts in `scripts/` instead of retyping commands; reuse one before writing a new one.
On Windows run the scripts from Git Bash; `bash` in PowerShell resolves to WSL, which is not supported.
- Parallel agents: own worktree + `CARGO_TARGET_DIR` slot, see `docs/development/dev-loop.md`.
- `scripts/gate.sh [--rust-only] [--affected] [-p <crate>]...`: fmt check, clippy, test, `cargo xtask codegen --check`, worker build/test, extension typecheck/lint/test/build; `--affected`: only what `cargo xtask affected` selects for the change. Must pass before committing.
- `scripts/finish.sh [--all] [bench args]`: gate and bench on what the change affects, at the same time, bench stopped when the gate fails; the check before committing a finished change. `--all`: everything (the lead, on main after a batch of merges). `-p <crate>` / `--lang <bench folder>` (repeatable) replace the gate's / bench's selection when you know it is too wide or misses something; name the override in the commit message.
- `scripts/ship.sh`: push the branch, watch CI for HEAD (refuses with staged changes).
- `scripts/bench.sh [--base REV] [--all | --lang FOLDER...] [--time] | --stop`: instruction + allocation counts per phase of the affected languages (`--all`: every language) against the merge-base (exit 1 on regression; `--time` adds interleaved wall-clock A/B of the CLI; baseline counts are cached per language for all slots). Run it (through `finish.sh`) after every feature change; first time on Windows run `scripts/bench-setup-wsl.sh`. History: `bench/history.jsonl`.
- `cargo xtask new-rule <language> <id> [--rust]`: starter rule (pattern file, or Rust with `--rust`) + fixture; the flow is in docs/development/testing.md.
- `scripts/cost.sh [<rev-range>]`: added/removed lines per category (Rust src, Rust tests, rule fixtures, other test data, docs, generated, scripts/other); default: merge-base with origin/main to the working tree.
- `scripts/parity.sh <tool>`: differential test of `sudsy lint` against an upstream tool (yamllint) on `tests/parity/<tool>/`; catalog in `docs/parity/`.
- `scripts/dogfood.sh`: build debug CLI + worker into `target/dogfood/`, which `.vscode/settings.json` uses so VS Code never locks `target/debug`; restart the server after running it.

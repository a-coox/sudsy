# Sudsy

Formatter + linter for C#, JSON and YAML. `docs/` describes how everything works now (index: `docs/README.md`); read the docs for the area you change before changing code.

## Layout
- Cargo workspace in `crates/` (see docs/architecture/overview.md). Language crates depend on core crates, never the reverse.
- .NET worker in `workers/csharp/` (net10.0, roll-forward). Grammars in `grammars/`, WIT in `wit/`, VS Code extension in `editors/vscode/`.

## Conventions
- Small focused modules; extract helpers instead of growing entrypoints. No duplicated logic across crates: shared behavior lives in a core crate.
- Errors: `thiserror` in libraries, `anyhow` only in binaries (`cli`, `lsp`).
- Snapshot tests use `insta`; fixtures live under `tests/fixtures/<lang>/` or the crate's `tests/`.
- Every formatter test checks lossless round-trip, idempotency and meaning preservation.
- Clean up as you go: once a decision is made, delete the losing alternatives (benchmark candidates, prototypes, bootstrap code); once a temporary placement or shim is superseded (code living somewhere "until X exists"), move or remove it when X exists. Results stay in git history.
- Presets are pure config data. Formatter and rule code never branches on a preset or tool name (no "csharpier mode", no hidden preset-only values). Each style difference is either default behavior or a named, user-settable option that is documented in the schema, so any preset can be reproduced by writing its options in sudsy.yaml by hand.
- Measure performance of everything you add or change on a hot path (load, parse, format, lint, plugins, editor requests): record method, machine and numbers in `docs/performance/numbers.md` (latest values only), behind a script or `perf-bench` mode so it can be re-run. Unmeasured is unfinished.
- No speculative features: build what `docs/roadmap.md` does not defer and the task needs (style options needed to express a preset are not speculative). When a detail is open, pick the simplest option.
- Docs describe current state, not history: update the relevant `docs/` file in the same commit as the change (behavior, options, numbers, roadmap items). No milestone names or "was/used to" prose.

## Commands
Run scripts in `scripts/` instead of retyping commands; reuse one before writing a new one.
On Windows run the scripts from Git Bash; `bash` in PowerShell resolves to WSL, which is not supported.
- `scripts/gate.sh [--rust-only] [-p <crate>]`: fmt check, build, clippy, test, `cargo xtask codegen --check`, dotnet build/test, extension typecheck/lint/test/build. Must pass before committing.
- `scripts/ship.sh`: push the branch, watch CI for HEAD (refuses with staged changes).
- `scripts/bench.sh [--base REV] [--time]`: instruction + allocation counts per language and phase against the merge-base (exit 1 on regression; `--time` adds interleaved wall-clock A/B of the CLI). Run it after every feature change; first time on Windows run `scripts/bench-setup-wsl.sh`. History: `bench/history.jsonl`.
- `scripts/parity.sh <tool>`: differential test of `sudsy lint` against an upstream tool (yamllint) on `tests/parity/<tool>/`; catalog in `docs/parity/`.
- `scripts/dogfood.sh`: build debug CLI + worker into `target/dogfood/`, which `.vscode/settings.json` uses so VS Code never locks `target/debug`; restart the server after running it.

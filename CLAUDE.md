# Sudsy

Formatter + linter for C#, JSON and YAML. `plan.md` is the design source of truth; read the sections relevant to your task before changing code.

## Layout
- Cargo workspace in `crates/` (see plan.md "Repository layout"). Language crates depend on core crates, never the reverse.
- .NET worker in `workers/csharp/` (net10.0, roll-forward). Grammars in `grammars/`, WIT in `wit/`, VS Code extension in `editors/vscode/`.

## Conventions
- Small focused modules; extract helpers instead of growing entrypoints. No duplicated logic across crates: shared behavior lives in a core crate.
- Errors: `thiserror` in libraries, `anyhow` only in binaries (`cli`, `lsp`).
- Snapshot tests use `insta`; fixtures live under `tests/fixtures/<lang>/` or the crate's `tests/`.
- Every formatter test checks lossless round-trip, idempotency and meaning preservation.
- Clean up as you go: once a decision is made, delete the losing alternatives (benchmark candidates, prototypes, bootstrap code); once a temporary placement or shim is superseded (code living somewhere "until X exists"), move or remove it in the milestone that makes X exist. Results stay in docs/decisions.md and git history.
- Presets are pure config data. Formatter and rule code never branches on a preset or tool name (no "csharpier mode", no hidden preset-only values). Each style difference is either default behavior or a named, user-settable option that is documented in the schema, so any preset can be reproduced by writing its options in sudsy.yaml by hand.
- Measure performance of everything you add or change on a hot path (load, parse, format, lint, plugins, editor requests): record method, machine and numbers in `docs/decisions.md`, behind a script or `perf-bench` mode so it can be re-run. Unmeasured is unfinished.
- No speculative features beyond plan.md (style options needed to express a preset in the plan's preset table are not speculative). When the plan leaves a detail open, pick the simplest option and note it in `docs/decisions.md`.

## Commands
Run scripts in `scripts/` instead of retyping commands; reuse one before writing a new one.
On Windows run the scripts from Git Bash; `bash` in PowerShell resolves to WSL, which is not supported.
- `scripts/gate.sh [--rust-only] [-p <crate>]`: fmt check, build, clippy, test, `cargo xtask codegen --check`, dotnet build/test, extension typecheck/lint/test/build. Must pass before committing.
- `scripts/ship.sh`: push the branch, watch CI for HEAD (refuses with staged changes).
- `scripts/dogfood.sh`: build debug CLI + worker into `target/dogfood/`, which `.vscode/settings.json` uses so VS Code never locks `target/debug`; restart the server after running it.

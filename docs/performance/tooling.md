# Measuring

`scripts/bench-write.sh OLD NEW` times `format` rewriting many files with two binaries (the write path, which `bench.sh` does not cover).

Run `scripts/bench.sh` after every feature change; CI runs tier 1 on ubuntu-latest against the PR base. Record method, machine and numbers for any hot path you change in [numbers.md](numbers.md) (latest values only).

## `scripts/bench.sh [--base REV] [--time] [--rounds N] [--no-instr] [--record|--no-record]`
- Inputs: `bench/inputs/` (committed: 12 C# files from dotnet/runtime, 40 JSONTestSuite `y_*` files, 60 yaml-test-suite documents; 0.25 MB) plus generated JSON (4k objects) and YAML (6k items) from `perf-bench`. Workload: `perf-bench fixed LANG PHASE`, phases parse, format, format-query (the `format` query of the engine, layout rules included), lint (default rules), lint-all (JSON and YAML: every rule on at its defaults, opt-in ones included), patterns, plugins.
- Baseline: merge-base with origin/main (`--base REV`), built in a git worktree at `target/bench/base-src` with the candidate's `tools/perf-bench` copied in, so both sides run the same workload.
- Tier 1 (deterministic): allocations (native, `--features count`) and instructions (valgrind in WSL on Windows, own target dir under `~/.cache/sudsy-bench/<slot name>` (the `CARGO_TARGET_DIR` directory name, else the checkout name), so concurrent slots do not share one; set-up excluded by toggling collection on the measured function). Fails on instructions +1% or allocations +1% (and at least 10); `*.lint-all.*` metrics grow with every new rule, so they are printed with their change but never flagged. C# phases: allocations only. `*.work` lines (files, findings) flag a changed workload.
- Tier 2 (`--time`, `scripts/bench-time.sh`): interleaved A/B rounds of `--version`, `format --check` and `lint` on the CLI with the worker; median of per-round ratios with a bootstrap 95% CI; fails only when the CI lower bound exceeds 1.03. Five rounds gives about +-30%; use 11+.
- History: `bench/history.jsonl`, one line per run keyed by commit; counts compare across hosts, times only within the recorded `host`.
- Setup: `scripts/bench-setup-wsl.sh` once (apt needs sudo). Tier-1 instruction counts have not been run on the dev machine yet.

## `tools/perf-bench` (`cargo run --release -p perf-bench -- ...`)
- `json|yaml|json-flat|all [--phases-only]`: generated inputs (40k pretty JSON objects 11.1 MB; 60k YAML items 5.0 MB; one-line JSON 1.1 MB), per-phase timing, whole `format_file` at 1x/4x/16x. `--features count` adds allocations per phase, `--sites` samples allocation sites, `--features mimalloc` swaps the allocator. `--write DIR` writes the inputs.
- `csharp DIR [PASSES]`: warm worker pool, same files repeatedly with the safety check.
- `semantic DIR [LIMIT]`: cost per analysis depth, and the compiler-agreement run.
- `fixed LANG PHASE`: the `bench.sh` workload. `layout LANG [--inputs DIR]`: the `format` query with 0, 1, 10 and 100 pattern layout rules.
- `extensions-cli <sudsy>`: CLI processes with extensions. `--bin lsp-latency DIR WORKER.dll FILES RUNS`: server latency while typing.
- No sampling profiler is available on the dev machine (`samply` needs Administrator): work is found from phase timings, allocation counts and sampled allocation backtraces.

## Scripts under `tests/conformance/`
`speed.sh` (CSharpier vs `sudsy format --check` with `csharpier-compat`; run `csharpier-baseline.sh` first, it copies the corpus to `$SUDSY_DIFF_DIR`), `speed-json-yaml.sh` (Prettier, Biome, dprint; versions pinned in `speed-tools/`), `speed-extensions.sh [tree] [limit]` (pattern/plugin cost), `speed-lint.sh` (lint per depth, compare two checkouts). `workers/csharp/Sudsy.Bench` and `tools/wire-bench` measure the C# parse/mapping/serialize/decode path.

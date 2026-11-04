# Current numbers

Machine unless noted: AMD Ryzen 5 3600 (6 cores / 12 threads), 32 GB, Windows 11, release builds, mimalloc in the CLI, one run, noise 10-15%. Compare ratios across days, not absolutes. Method: [tooling.md](tooling.md). Latest value per metric only; `bench/history.jsonl` keeps the recorded series.

## Formatting (`perf-bench`, `format_file` = parse, layout, print, re-parse, tree compare, YAML model compare)
| Path | Number |
|---|---|
| JSON 11.1 MB phases: parse / layout / print / re-parse / compare | 304 / 1136 / 259 / 408 / 122 ms; whole 5.9 MB/s at 4x |
| JSON one line 1.1 MB | 386 ms (2.9 MB/s) |
| YAML 5.0 MB phases: parse / layout / print / re-parse / compare / data model | 522 / 1655 / 202 / 499 / 194 / 732 ms; whole 1.6 MB/s at 4x |
| TOML (tree-sitter) 5.5 MB generated (`perf-bench toml`) phases: parse / layout / print / re-parse / compare | 2337 / 558 / 142 / 2348 / 69 ms; whole 1.0 MB/s at 4x (parse 2.3 MB/s; 4.1 MB/s on 512 real `Cargo.toml` files in an earlier spike). Tree-sitter is two thirds of the parse, the adapter one third (spike). |
| Allocations per fixed workload (`scripts/bench.sh`) | json.format 851308, yaml.format 2114000, toml.format 807876, cs.format 140206 |
| Instructions per fixed workload (`scripts/bench.sh`, valgrind in WSL) | json.format 1.392 G, yaml.format 1.789 G, toml.format 3.189 G (toml.parse 1.215 G; the fixed TOML workload is 22 files, 3 MB generated, so not comparable with the other languages) |
| C# layout options of `csharp/stylecop` (blank-line rules, `)` placement, expanded empty bodies), `format --check` of the 12 files of `bench/inputs/cs` (0.1 MB), release CLI with worker, cold process, 7 runs x 2 | median 121 ms with `csharp/microsoft`, 122 ms with `csharp/stylecop` (process and worker start dominate; no measurable difference). Default-option allocations are unchanged (the rules are only built when an option asks for them). |

`format` writing 1500 small JSON files that all change (`scripts/bench-write.sh`, NTFS): 0.9 s with `fs::write`, 3.4 s with the temp file + fsync + rename of `format`/`fix` (about 1.7 ms per rewritten file, all of it the sync); files that do not change are not written, so check mode and clean trees are unaffected.

Time is linear in file size (guards: `core-format/tests/scaling.rs`, `tools/perf-bench/tests/scaling.rs`). Remaining cost is spread over phases: layout is ~7 allocations per node (rowan red nodes), parse is mostly rowan's node builder, the safety check re-parses and re-compares (about a third of the run). Allocator: mimalloc gives 1.3-1.4x over the system allocator.

## JSON / YAML against other formatters (`speed-json-yaml.sh`, check mode, caches off, new process per run, best of 3, seconds)
Prettier 3.9.10, Biome 2.6.4, dprint 0.64.3 (json 0.25.2, pretty_yaml 0.6.0), Node 24.22.0. Sudsy runs its always-on safety check. Biome YAML is not stable and is excluded.

| Input | Sudsy | Prettier | Biome | dprint |
|---|---|---|---|---|
| JSON, 95 small files | 0.10 | 0.67 | 0.36 | 0.37 |
| JSON, one 11.1 MB file | 2.03 | 9.19 | 1.45 | 1.91 |
| YAML, 308 small files | 0.11 | 1.13 | - | 0.35 |
| YAML, one 5.0 MB file | 2.39 | 20.19 | - | 3.06 |

## C# (12 cores, `speed.sh` and `perf-bench csharp`)
| | CSharpier 1.3.0 | Sudsy CLI cold | Sudsy warm worker |
|---|---|---|---|
| CoreLib (1679 files, 27.8 MB) | 17.3 - 17.8 s | 3.8 - 3.9 s | 2.4 - 2.6 s |
| full corpus (7659 files, 89.8 MB) | 42.3 - 43.3 s | 9.7 - 10.3 s | 8.0 - 8.1 s |

(Machine ran ~15% slower on that day; use the ratios: 4.3-4.5x cold, 5.3-7.3x warm.) Baseline run: CoreLib warm 12 threads 7.0 / 4.1 / 3.4 s over three passes; one thread 13.5 s, per file median 1.6 ms, p95 26.9 ms. Editor-style (one thread, one file at a time, warm): median 1.4-1.5 ms, p95 15-25 ms, p99 67-117 ms (slowest 1% are large generated files).

C# cost structure: on CoreLib Roslyn parse ~0.94 s and the mapping walk ~0.83 s (88% of parse, 8.4 M elements); the walk is bounded by Roslyn's public API creating red nodes on demand (visiting with no mapping costs 65% of parse), so a C# file costs about 2x its Roslyn parse to reach Rust. The safety re-parse costs as much as the first parse. Worker pool: 1 worker 6.1 s, 2 4.1 s, 4 3.4 s, 6 3.3 s (hence the cap of 4). Tree wire encoding overhead is 4.8% of parse.

## Lint (`perf-bench semantic`, one file at a time, single worker)
| Tree | files | fast | standard | deep | first file | standard median / p95 |
|---|---|---|---|---|---|---|
| CoreLib | 1679 | 5.1 s | 56.1 s | 57.1 s | 2.5 s | 16 / 110 ms |
| Roslyn | 2858 | 6.1 s | 77.3 s | 82.6 s | 3.2 s | 11 / 72 ms |
| aspnetcore | 3122 | 4.5 s | 74.1 s | 85.4 s | 3.6 s | 12 / 42 ms |

`standard` is 10-15x `fast` (Roslyn binding and flow of every body); `deep` adds 2-15%. 500 runtime files: `fast` 2.2 s (median 1.1 ms), `standard` 30.6 s (median 25.7 ms, p95 178 ms; first-file workspace build is included in the tree rows).

## Extensions
| Path | Number |
|---|---|
| Traversal with no rule, per C# file | 90-96 us |
| Default built-in C# rules, per file | 177 us |
| Pattern rules, 100 rules, token start / `$X` start | 504 / 3970 us per file (4.4 / 22.7 us per rule) |
| Built-in pattern rules (C#: `crates/lang-csharp/rules/*.yaml`, 4 files) | building the provider reads the files: 195 us once per process (no worker started, no pattern compiled); compiling all four on first use (kind and regex only, no snippet parse): 1.0 ms. Against the commit before (`bench.sh`): instructions json/yaml/toml lint, parse, format unchanged (within 0.01%), cs.lint allocations 9369 -> 9369, cs.parse +1 allocation; json/yaml.patterns +0.22% / +0.17% instructions (the rule struct holds its patterns lazily). `bench.sh` has no C# instruction or lint-all metric, so the per-rule lint-all cost of the four converted rules against their Rust versions was not measured. Re-run: `cargo test -p lang-csharp --release --test builtin_patterns -- --ignored --nocapture`. |
| Layout rules, `format` query on the bench inputs (json 1.1 MB / yaml 0.5 MB), 0 / 1 / 10 / 100 pattern layout rules | json 225 / 235 / 313 / 687 ms, yaml 322 / 337 / 417 / 837 ms (`perf-bench layout`: ~4.6 ms per rule per MB, one syntax-only traversal; C# 12 files within worker noise) |
| `format` query with no layout rule, against the commit before | instructions json +0.07%, yaml -0.01%; allocations +0.01% (`bench.sh`, tier 1) |
| Default lint after `no-undefined-alias` (YAML, on by default; a `memchr` scan for `&` and `*`, tree lookups only at those bytes) and `unused-variable` / `unreachable-code` off for C#, against the commit before | instructions yaml.lint +0.30%, json.parse +0.12%, others within 0.1%; allocations yaml.lint +0.14%, cs.lint -45% (`bench.sh`, tier 1) |
| Engine rule dispatch by node kind: a sorted `Vec` with binary search instead of a SipHash `HashMap` lookup per node, against the commit before (default lint, pattern-only run) | instructions json.lint -4.4%, yaml.lint -4.1%, json.patterns -10.1%, yaml.patterns -10.9%; allocations unchanged (`bench.sh`, tier 1; WSL2 Ubuntu, callgrind) |
| Opt-in YAML rules (`no-duplicate-anchors`, `no-unused-anchors`, `no-empty-values`, `float-style`, `no-octal-values`, `quoted-strings`): every YAML rule on at its defaults (`perf-bench fixed yaml lint-all`) | yaml 1000.6 M instructions, 1.884 M allocations, against 559.0 M and 1.181 M before; most of the added work is `quoted-strings` with `required: always` reporting a diagnostic for each of the fixture's strings (output 30346 vs 6156 diagnostics) and testing each plain scalar against the implicit-type pattern. The default lint set is unchanged: yaml.lint 187.45 M instructions (-0.54%), 383.2 k allocations (`bench.sh`, tier 1) |
| StyleCop syntax rules (`no-region-in-body`, `nullable-shorthand`, `use-lambda`, `constant-on-right`, `use-literal-suffix`, `separate-field-declarations`, `suppression-justification`, `single-class-per-file`, `single-namespace-per-file`, `file-name-matches-type`; off by default, on in `csharp/stylecop`), against the commit before | default lint, parse and format unchanged (all `cs.*` instructions and allocations within 0.02%, `bench.sh` tier 1); `sudsy lint .` over the 12 files of `bench/inputs/cs`, release CLI with worker, cold process, median of 7 on a loaded machine: 4082 ms with defaults, 2848 ms with `csharp/stylecop` (worker start and run-to-run noise dominate; no measurable cost from the rules) |
| StyleCop semantic rules (`symbol_findings` worker query behind `no-unneeded-base-prefix`, `no-default-value-type-constructor`, `qualify-namespace-usings`, `use-tuple-syntax`, `tuple-element-names-in-signatures`, `debug-assert-message`, `debug-fail-message`, `inheritdoc-on-inheriting-member`) and `preserve_single_line_*` options, against the commit before | `bench.sh` tier 1: all `cs.*` and json/yaml instructions and allocations within 0.03% (`cs.format-query.allocs` +0.03%); the worker walk runs in the .NET process, which valgrind does not follow, so its cost is not in these counts (one syntax walk with symbol lookups only at base, `new`, `using`, tuple and `Debug` nodes) |
| Key and sequence rules shared across languages (`no-duplicate-keys`, `no-empty-keys`, `key-name-casing`, `sorted-keys`, `sorted-array-values` in `core-rules` through the `#mapping`/`#sequence` tags and `RuleHooks`, tag subscription resolved per file from a table built with the grammar), against the same workload with the rules in the language crates (`bench.sh`, tier 1; WSL2 Ubuntu, callgrind; `perf-bench` runs the language's own and the shared rules, so both sides run the same set) | default lint (`no-duplicate-keys`): json.lint 140.3 M -> 89.3 M instructions (-36%), 213.0 k -> 145.0 k allocations; yaml.lint 189.8 M -> 110.1 M (-42%), 383.2 k -> 195.9 k; toml.lint 41.0 M -> 88.3 M, 91.4 k -> 147.0 k (TOML had no key rule and now runs `no-duplicate-keys` by default; 134.3 M and 235.4 k before the rule stopped finding key ranges it does not report, scanned mappings of up to 8 entries instead of hashing them and borrowed bare dotted TOML keys from the file text). Every rule on at its defaults (`perf-bench fixed LANG lint-all`; json 9 rules, yaml 17, toml 8, comparable only with the same set): json 859.2 M -> 810.7 M (-5.6%), 1.500 M -> 1.438 M allocations; yaml 1219.7 M -> 1358.6 M (+11.4%), 2.298 M -> 2.436 M, which now also runs `no-empty-keys`, `key-name-casing` and `sorted-array-values`, and `sorted-keys` on flow mappings; toml 324.9 M -> 920.8 M. Parse and format unchanged (within 0.2%). Already sorted input is decided in one pass without allocating a plan |
| Per-element brace placement (`brace_same_line`, `else_line`/`catch_line`/`finally_line`, `label_indent`, `Doc::Outdent`) against the commit before | json/yaml instructions within +0.09%, allocations unchanged; cs.format allocations +0.00% (140220 to 140221), cs.format-query +0.14% (141724 to 141917) (`bench.sh`, tier 1; the check per brace is a scan of an empty list) |
| StyleCop ordering rules (`member-order`, `accessor-order`, `using-order` options; off by default) and formatter `sort_usings` (off by default; the hook is one option read per file), against the commit before | default lint, parse and format unchanged (cs.* allocations within +0.02%, json/yaml instructions within 0.1%, `bench.sh` tier 1); every-rule-on json/yaml lint-all +2% to +4% is the shared tree helper (`entry_range`), informational; wall time 11 rounds: startup 0.968, format 0.999, lint 1.029 (CI includes 1.0) |
| Formatting inactive `#if` branches (`preprocessor_symbol_sets`, `derive_symbol_sets`; off by default, on in `csharp/csharpier-compat`) | Option off: two option reads per file, no directive walk (against the commit before, `bench.sh` tier 1: cs.format allocations +0.01% (140210 to 140223), cs.format-query +0.04%, json/yaml instructions within 0.01%; tier 2 wall time, 11 rounds: format ratio 1.024 with CI [0.946, 1.096], i.e. unchanged). Option on: files without disabled text pay nothing but that check (bench inputs, 12 files of 0.1 MB with 3 `#if`: median 3.3 vs 4.3 ms per file, single pass, within noise). Per extra parse that supplies a branch (parse + format + check reparse): about 5-7 ms for 6-25 KB files, warm worker, one thread: synthetic files of 20 `#if S1 / #elif S2 ... #else` chains with 1/2/4/8 `#elif` (5.7 / 8.5 / 14 / 25 KB), median per file 5.3 / 7.7 / 7.8 / 12.0 ms off and 10.4 / 18.0 / 28.1 / 67.0 ms on (1 / 2 / 4 / 8 extra sets). Release `perf-bench csharp DIR 6` with `PERF_BENCH_CSHARP_FORMAT="derive_symbol_sets: true"` against `false`; the 7659-file corpus formats twice (default and compat) in 178 s with 0 failures. |
| Pattern compile | 0.2-0.4 ms per rule after a first worker round trip of 11-12 ms |
| Plugin vs built in, `todo-marker` per C# file (14 KB) | 1170 vs 134 us (~0.07-0.1 ms per KB per rule: tree encode on host 23-30 us/KB, decode and walk in Wasm ~5x native) |
| Plugin load: cache off / empty / warm / same process | 178 / 183 / 6.8 ms / 0.6 ms |
| Empty-file plugin call | 7 us |
| `sudsy lint .`, 20 small JSON files: none / pattern / Wasm warm | 38 / 37 / 62 ms |
| Binary size with the Wasm host (Windows x64 release) | 28.3 MB (9.3 MB gz) vs 8.8 MB without; options tried: [Binary size](#binary-size) |

## Binary size
`sudsy.exe`, Windows x64, 12 threads, clean release build of `cli` + `perf-bench`; gz = `gzip -9`. Throughput: best of 5 `format --check` on the generated 11 MB JSON / 5 MB YAML and 100 small files, noise 10-15% (first runs of a new exe are slower from the virus scanner).

| Variant | Size | gz | Clean build | Format speed |
|---|---|---|---|---|
| Before (`wasmtime-wasi` default features) | 28.99 MB | 9.50 MB | 5.0 min | 1.75-1.85 s JSON, 2.3 s YAML |
| `wasmtime-wasi` with `p2` only (**kept**) | 28.31 MB | 9.28 MB | 5.2 min | same within noise |
| `lto = "thin"`, `codegen-units = 1`, `strip = true` | 26.92 MB | 9.17 MB | 11.6 min | same within noise |
| `lto = "fat"`, same | 25.63 MB | 9.01 MB | 14.5 min | same or slower (one run 2x on JSON), startup not better |

`wasmtime` itself already has default features off, with only what the host uses. The ~20 MB Wasm host is Cranelift, so LTO saves 7-12% for 2-3x the build time on every CI platform job: not taken. `strip` changes nothing with the MSVC toolchain (symbols are in the pdb). Reconsider fat LTO only if the size matters more than release build time.

TOML (tree-sitter-toml-ng through `core-treesitter`, `lang-toml`): `sudsy.exe` 29.71 MB without, 29.87 MB with (+0.15 MB, +0.5%), same machine and profile as above. Clean release build of `cli`: 261 s without, 239 s with; the difference is inside the noise of a shared machine (the C compile of the grammar and the runtime takes about a minute on an idle one). In the spike, touching `lang-toml` and rebuilding the CLI took 14.5 s against 15.0 s for `lang-json`.

## Editor (`lsp-latency`, 300 corpus files as workspace, 20 KB file open, six semantic rules)
| | Number |
|---|---|
| Formatting asked right after the first pull | 1.0-1.3 s (second worker starting) |
| First pull (first workspace build) | 3.2-3.7 s |
| Typing, 40 edits 30 ms apart: last pull answered after last edit | 0.10-0.25 s |
| Typing: pulls that did the full work | 1-2 of 40 |

## Dev loop / tooling (persistent `CARGO_TARGET_DIR` slot, `docs/development/dev-loop.md`)
Method: `CARGO_TARGET_DIR=C:/Coding/.sudsy-targets/slot1`, Git Bash, wall seconds, a fresh slot (cold) then the same command again (warm); Windows 11 desktop shared with two other agents building at the same time, so times are noisy. The warm test time is test execution, not compilation.

| Command | Cold | Warm |
|---|---|---|
| `scripts/gate.sh --rust-only` | 827 s (fmt 1, clippy 90, test 565, codegen 14+, sample plugin 40) | 171 s (clippy 1, test 159) |
| `scripts/bench.sh --no-record` (incl. WSL callgrind cache `~/.cache/sudsy-bench/slot1`) | 1363 s | 91 s |
| `scripts/parity.sh yamllint` (venv install + CLI build reused) | 86 s | |
| `scripts/gate.sh -p lang-json` from another checkout path with the warm slot | | 38 s (only workspace crates and a few small dependencies recompile; wasmtime and cranelift are reused) |

## Not measured repeatably
- LSP completion/hover/format request latency in `bench.sh`.
- Config load and discovery on a large repository.
- Cold CLI start of `format` on a C# project (worker spawn): only in `bench.sh --time` totals.
- Incremental re-lint after an edit (cache hit/miss path).
- `deep` depth in `bench.sh` (only `perf-bench semantic`).
- Instruction counts of the C# phases (the worker is a separate .NET process valgrind does not follow).

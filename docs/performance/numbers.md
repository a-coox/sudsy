# Current numbers

Machine unless noted: AMD Ryzen 5 3600 (6 cores / 12 threads), 32 GB, Windows 11, release builds, mimalloc in the CLI, one run, noise 10-15%. Compare ratios across days, not absolutes. Method: [tooling.md](tooling.md). Latest value per metric only; `bench/history.jsonl` keeps the recorded series.

## Formatting (`perf-bench`, `format_file` = parse, layout, print, re-parse, tree compare, YAML model compare)
| Path | Number |
|---|---|
| JSON 11.1 MB phases: parse / layout / print / re-parse / compare | 304 / 1136 / 259 / 408 / 122 ms; whole 5.9 MB/s at 4x |
| JSON one line 1.1 MB | 386 ms (2.9 MB/s) |
| YAML 5.0 MB phases: parse / layout / print / re-parse / compare / data model | 522 / 1655 / 202 / 499 / 194 / 732 ms; whole 1.6 MB/s at 4x |
| Allocations per fixed workload (`scripts/bench.sh`) | json.format 851308, yaml.format 2114000, cs.format 140206 |
| Instructions per fixed workload (`scripts/bench.sh`, valgrind in WSL) | json.format 1.392 G, yaml.format 1.789 G |
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
| Layout rules, `format` query on the bench inputs (json 1.1 MB / yaml 0.5 MB), 0 / 1 / 10 / 100 pattern layout rules | json 225 / 235 / 313 / 687 ms, yaml 322 / 337 / 417 / 837 ms (`perf-bench layout`: ~4.6 ms per rule per MB, one syntax-only traversal; C# 12 files within worker noise) |
| `format` query with no layout rule, against the commit before | instructions json +0.07%, yaml -0.01%; allocations +0.01% (`bench.sh`, tier 1) |
| Default lint after `no-undefined-alias` (YAML, on by default; a `memchr` scan for `&` and `*`, tree lookups only at those bytes) and `unused-variable` / `unreachable-code` off for C#, against the commit before | instructions yaml.lint +0.30%, json.parse +0.12%, others within 0.1%; allocations yaml.lint +0.14%, cs.lint -45% (`bench.sh`, tier 1) |
| Engine rule dispatch by node kind: a sorted `Vec` with binary search instead of a SipHash `HashMap` lookup per node, against the commit before (default lint, pattern-only run) | instructions json.lint -4.4%, yaml.lint -4.1%, json.patterns -10.1%, yaml.patterns -10.9%; allocations unchanged (`bench.sh`, tier 1; WSL2 Ubuntu, callgrind) |
| Opt-in YAML rules (`no-duplicate-anchors`, `no-unused-anchors`, `no-empty-values`, `float-style`, `no-octal-values`, `quoted-strings`): every YAML rule on at its defaults (`perf-bench fixed yaml lint-all`) | yaml 1000.6 M instructions, 1.884 M allocations, against 559.0 M and 1.181 M before; most of the added work is `quoted-strings` with `required: always` reporting a diagnostic for each of the fixture's strings (output 30346 vs 6156 diagnostics) and testing each plain scalar against the implicit-type pattern. The default lint set is unchanged: yaml.lint 187.45 M instructions (-0.54%), 383.2 k allocations (`bench.sh`, tier 1) |
| Opt-in JSON rules (`sorted-keys` options, `no-empty-keys`, `key-name-casing`, `unsafe-numbers`, `sorted-array-values`) and YAML `sorted-keys` options: every rule of the provider on at its defaults (`perf-bench fixed LANG lint-all`) | json 590.7 M instructions, 983.7 k allocations; yaml 558.9 M, 1.181 M. The commit before has fewer rules to switch on: json 337.6 M, 605.1 k; yaml 544.0 M, 1.187 M. Already sorted input is decided in one pass without allocating a plan; the default lint set is unchanged (`bench.sh`, tier 1) |
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

## Editor (`lsp-latency`, 300 corpus files as workspace, 20 KB file open, six semantic rules)
| | Number |
|---|---|
| Formatting asked right after the first pull | 1.0-1.3 s (second worker starting) |
| First pull (first workspace build) | 3.2-3.7 s |
| Typing, 40 edits 30 ms apart: last pull answered after last edit | 0.10-0.25 s |
| Typing: pulls that did the full work | 1-2 of 40 |

## Not measured repeatably
- LSP completion/hover/format request latency in `bench.sh`.
- Config load and discovery on a large repository.
- Cold CLI start of `format` on a C# project (worker spawn): only in `bench.sh --time` totals.
- Incremental re-lint after an edit (cache hit/miss path).
- `deep` depth in `bench.sh` (only `perf-bench semantic`).
- Instruction counts of the C# phases (the worker is a separate .NET process valgrind does not follow).

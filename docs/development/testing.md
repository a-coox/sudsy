# Testing

Every formatter test checks lossless round trip, idempotency, unchanged meaning (and comments kept). Parsers check the trivia attachment rule.

| Layer | What | Where |
|---|---|---|
| Parser/conformance | Round trip, attachment, shape check on large corpora (ignored tests, run by hand) | C#: `cargo test --release -p lang-csharp --test corpus -- --ignored` (7659 files, 89.8 MB, ~27 s); JSON: `lang-json/tests/conformance.rs`; YAML: `cargo test --release -p lang-yaml --test conformance -- --ignored` |
| Format corpus | Format twice (default and `csharpier-compat`): safety check and idempotency, 0 failures | `cargo test --release -p lang-csharp --test format_corpus -- --ignored` |
| Differential | `csharpier-compat` vs CSharpier 1.3.0 (pinned in `dotnet-tools.json`) | `tests/conformance/csharpier-baseline.sh`, then `--test differential -- --ignored`; writes `hunks.txt`, `summary.txt` |
| Snapshots | Fixtures per construct and option value (`insta`) | `crates/*/tests`, `tests/fixtures/csharp` (`error_*.cs` must report errors; also run with CRLF) |
| Presets | Sample input/expected output per preset; hand-written config equals the preset | `tests/fixtures/presets/`, `crates/cli/tests/presets.rs` |
| Rules | Inline-annotated fixtures (`// ^^^ rule-id`), fix output, fixes keep files parseable and idempotent | `provider_api::testing` |
| Compiler agreement | Flow rules vs CS0162/CS0168/CS0219 | `lang-csharp/tests/compiler_agreement.rs` |
| Worker contract | Protocol bytes, version mismatch, crash/restart | `provider-api/tests/proxy_contract.rs` (fake worker), `Sudsy.Worker.Tests` |
| Engine | Salsa re-execution events (`WillExecute`) | `core-incremental/tests/engine.rs` |
| LSP | In-process client | `crates/lsp/tests` |
| Plugins | Host vs built sample | `tests/plugins/sample` (needs `rustup target add wasm32-wasip2`) |
| Scaling guards | Work counts, not time | `core-format/tests/scaling.rs`, `tools/perf-bench/tests/scaling.rs` |
| Codegen | Generated files are fresh | `xtask/tests/codegen_is_fresh.rs` |

## Corpora (not fetched in CI)
- C#: `tests/conformance/fetch-corpus.sh` → pinned shallow sparse checkouts: dotnet/runtime v10.0.0 `System.Private.CoreLib/src` (1679 files), dotnet/roslyn tag Visual-Studio-2026-Version-18.7 (Compilers CSharp/Core Portable, Workspaces Core Portable; 2858), dotnet/aspnetcore v10.0.0 (Http, Mvc.Core, Middleware, Kestrel Core, Components, SignalR common; 3122). Every file parses with `TARGET_WINDOWS`, `TARGET_AMD64`, `TARGET_64BIT` (files named `*.Browser.*` with `TARGET_BROWSER`, `TARGET_WASM`, `TARGET_32BIT`) so `#else #error` branches do not fail.
- JSON: `fetch-json-corpus.sh` (JSONTestSuite `1ef36fa`). YAML: `fetch-yaml-corpus.sh` (yaml-test-suite `6ad3d2c`).

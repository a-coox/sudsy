# Testing

Every formatter test checks lossless round trip, idempotency, unchanged meaning (and comments kept). Parsers check the trivia attachment rule.

| Layer | What | Where |
|---|---|---|
| Parser/conformance | Round trip, attachment, shape check on large corpora (ignored tests, run by hand) | C#: `cargo test --release -p lang-csharp --test corpus -- --ignored` (7659 files, 89.8 MB, ~27 s); JSON: `lang-json/tests/conformance.rs`; YAML: `cargo test --release -p lang-yaml --test conformance -- --ignored` |
| Format corpus | Format twice (default and `csharpier-compat`): safety check and idempotency, 0 failures | `cargo test --release -p lang-csharp --test format_corpus -- --ignored` |
| Differential | `csharpier-compat` vs CSharpier 1.3.0 (pinned in `dotnet-tools.json`) | `tests/conformance/csharpier-baseline.sh`, then `--test differential -- --ignored`; writes `hunks.txt`, `summary.txt` |
| Snapshots | Fixtures per construct and option value (`insta`) | `crates/*/tests`, `tests/fixtures/csharp` (`error_*.cs` must report errors; also run with CRLF) |
| Presets | Sample input/expected output per preset; hand-written config equals the preset | `tests/fixtures/presets/`, `crates/cli/tests/presets.rs` |
| Rules | One fixture file per case under `tests/rules/<language>/<rule-id>/`: annotated diagnostics, fix output, fixes keep files parseable and idempotent | `provider_api::fixtures`, run by `lang-*/tests/rule_fixtures.rs`; helpers in `provider_api::testing` |
| Compiler agreement | Flow rules vs CS0162/CS0168/CS0219 | `lang-csharp/tests/compiler_agreement.rs` |
| Worker contract | Protocol bytes, version mismatch, crash/restart | `provider-api/tests/proxy_contract.rs` (fake worker), `Sudsy.Worker.Tests` |
| Engine | Salsa re-execution events (`WillExecute`) | `core-incremental/tests/engine.rs` |
| LSP | In-process client | `crates/lsp/tests` |
| Plugins | Host vs built sample | `tests/plugins/sample` (needs `rustup target add wasm32-wasip2`) |
| Scaling guards | Work counts, not time | `core-format/tests/scaling.rs`, `tools/perf-bench/tests/scaling.rs` |
| Parity catalog | `docs/parity/*.yaml` only name rules and options that exist; README counts are current | `languages/tests/parity.rs` |
| `init --from` | each tool fixture under `tests/fixtures/init/` migrates to a config that loads (snapshots), mappings agree with the catalog | `cli/tests/init.rs` |
| Codegen | Generated files and doc tables are fresh | `xtask/tests/codegen_is_fresh.rs` |

## Rule fixtures
Adding a rule needs no Rust test: add `tests/rules/<language>/<rule-id>/<name>.<ext>` files (C# fixtures run through the worker).
- `<name>.<ext>` is the input. Expected diagnostics are written under the line they point at, `// ^^^ rule-id` (`#` in YAML; carets give the span length, the marker may be left out when the span starts in the first columns). No annotation means no diagnostic. The id is checked, so a fixture also shows other rules that fire by default.
- `<name>.fixed.<ext>` is the text after the safe fixes, `<name>.unsafe.<ext>` after the unsafe ones too. A missing file means the fixes change nothing (resp. nothing more). Every fix run also checks that the file stays parseable and that a second run is idempotent.
- `rules.yaml` in the folder is the `lint.rules` mapping (default `<rule-id>: warn`); `<name>.rules.yaml` next to an input replaces it for that input alone, which is how a case sets options. Shared rules (`sorted-keys`, `no-duplicate-keys`, ...) have fixtures in each language's folder that tags the concept.
- `SUDSY_BLESS=1 cargo test -p lang-json --test rule_fixtures` (or `lang-yaml`, `lang-csharp`) rewrites the annotations and the `.fixed`/`.unsafe` files from what the rules do now; review the diff.
- One test per language lists every failing file with its message. Cases the runner cannot express stay in `crates/*/tests`: option validation errors, config-level on/off, depth and project scope.

### Adding a rule
`cargo xtask new-rule <language> <rule-id> [--rust]` does steps 1 and 2 with a starter that reports empty statements (the gate passes as generated; edit the rule and its fixture, then `SUDSY_BLESS=1 cargo test -p lang-<language> --test rule_fixtures`) and prints the parity-catalog lines for step 3. Without `--rust` it writes the pattern file, with `--rust` a `rule!` module registered in the language's `RULES`; only C# has scaffolding so far (the other languages' rules are written by hand).
1. A `rule!` declaration in the rule's module (id, category, description, default, fix, options with checks and descriptions; [../rules/engine.md](../rules/engine.md#rule-trait)) and register the rule.
   A syntax rule that fits a pattern is a file instead: `crates/lang-<language>/rules/<rule-id>.yaml` ([../rules/patterns.md](../rules/patterns.md#built-in-pattern-rules)), no Rust.
2. A fixture folder `tests/rules/<language>/<rule-id>/` as above.
3. The matching entry in `docs/parity/<tool>.yaml` (`sudsy: "rule: <id>"`) when another tool has an equivalent.
4. `cargo xtask codegen`: the rule table, option tables and `sudsy.schema.json` are generated; do not edit them by hand.

## Corpora (not fetched in CI)
- C#: `tests/conformance/fetch-corpus.sh` → pinned shallow sparse checkouts: dotnet/runtime v10.0.0 `System.Private.CoreLib/src` (1679 files), dotnet/roslyn tag Visual-Studio-2026-Version-18.7 (Compilers CSharp/Core Portable, Workspaces Core Portable; 2858), dotnet/aspnetcore v10.0.0 (Http, Mvc.Core, Middleware, Kestrel Core, Components, SignalR common; 3122). Every file parses with `TARGET_WINDOWS`, `TARGET_AMD64`, `TARGET_64BIT` (files named `*.Browser.*` with `TARGET_BROWSER`, `TARGET_WASM`, `TARGET_32BIT`) so `#else #error` branches do not fail.
- JSON: `fetch-json-corpus.sh` (JSONTestSuite `1ef36fa`). YAML: `fetch-yaml-corpus.sh` (yaml-test-suite `6ad3d2c`).

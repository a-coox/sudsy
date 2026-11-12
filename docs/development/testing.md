# Testing

Every formatter test checks lossless round trip, idempotency, unchanged meaning (and comments kept). Parsers check the trivia attachment rule.

A crate with several integration-test files builds them as one test binary, `integration` (`autotests = false` and a `[[test]]` in its `Cargo.toml`; for generated language crates the template in `xtask/src/lang_crate.rs`): `tests/main.rs` holds a `mod <file>;` line for every file in `tests/` (and `mod common;` for `tests/common/mod.rs`, which files reach as `crate::common`). A new test file needs its `mod` line. One binary per crate links once instead of once per file, which cuts the disk writes of a rebuild ([numbers](../performance/numbers.md)); tests run in one process per binary, so they share statics and the environment. Run one file's tests with `cargo test -p <crate> --test integration <file>::`. insta snapshots are named `integration__<file>__<name>.snap`.

| Layer | What | Where |
|---|---|---|
| Parser/conformance | Round trip, attachment, shape check on large corpora (ignored tests, run by hand) | C#: `cargo test --profile corpus -p lang-csharp --test integration -- --ignored corpus::`; JSON: `lang-json/tests/conformance.rs`; YAML: `cargo test --profile corpus -p lang-yaml --test integration -- --ignored conformance::`; CSS, SCSS, Less: `lang-css/tests/css.rs` (`fetch-css-corpus.sh`) |
| Format corpus | Format twice (default and `csharpier-compat`): safety check and idempotency, 0 failures | `cargo test --profile corpus -p lang-csharp --test integration -- --ignored format_corpus::` |
| Differential | `csharpier-compat` vs CSharpier 1.3.0 (pinned in `dotnet-tools.json`) | `tests/conformance/csharpier-baseline.sh`, then `--test integration -- --ignored differential::`; writes `hunks.txt`, `summary.txt` |
| Snapshots | Fixtures per construct and option value (`insta`) | `crates/*/tests`, `tests/fixtures/csharp` (`error_*.cs` must report errors; also run with CRLF) |
| Presets | Sample input/expected output per preset; hand-written config equals the preset | `tests/fixtures/presets/`, `crates/cli/tests/presets.rs` |
| Rules | One fixture file per case under `tests/rules/<language>/<folder>/` (a rule or a family of rules): annotated diagnostics, fix output, fixes keep files parseable and idempotent | `provider_api::fixtures`, run by `lang-*/tests/rule_fixtures.rs`; helpers in `provider_api::testing` |
| Compiler agreement | Flow rules vs CS0162/CS0168/CS0219 | `lang-csharp/tests/compiler_agreement.rs` |
| Worker contract | Protocol bytes, version mismatch, crash/restart | `provider-api/tests/proxy_contract.rs` (fake worker), `Sudsy.Worker.Tests` |
| Engine | Salsa re-execution events (`WillExecute`) | `core-incremental/tests/engine.rs` |
| LSP | In-process client | `crates/lsp/tests` |
| Plugins | Host vs built sample | `tests/plugins/sample` (needs `rustup target add wasm32-wasip2`) |
| Scaling guards | Work counts, not time | `core-format/tests/scaling.rs`, `tools/perf-bench/tests/scaling.rs` |
| Parity catalog | `docs/parity/*.yaml` only name rules and options that exist; README counts are current | `languages/tests/parity.rs` |
| `init --from` | each tool fixture under `tests/fixtures/init/` migrates to a config that loads (snapshots), mappings agree with the catalog | `cli/tests/init.rs` |
| Codegen | Generated files and doc tables are fresh | `xtask/full/tests/codegen_is_fresh.rs` |

Tree-sitter languages share `provider_api::format_testing` (see [adding-a-language.md](../architecture/adding-a-language.md)): their formatter and corpus tests are a few lines each.

## The C# worker in tests
Every test that needs the worker calls `lang_csharp::testing::built_worker()`: `SUDSY_CSHARP_WORKER` if set, else one Debug `dotnet build` per test process under a file lock (`workers/csharp/Sudsy.Worker/obj/test-build.lock`). Test binaries run as parallel processes (nextest, concurrent gates); unlocked, two of them compiling the same project collide on its `obj/` files (CS2012 "file is being used by another process"). Never build the worker in a test file by hand.

## Rule fixtures
Adding a rule needs no Rust test: add `tests/rules/<language>/<folder>/<name>.<ext>` files (C# fixtures run through the worker). A folder is named for a rule (`no-empty-catch`) or for a family of rules that share inputs (`doc-tags`, `type-layer`); inputs are small and cover the rules' own cases, and a shared input is never copied into several folders.
- `<name>.<ext>` is the input. Expected diagnostics are written under the line they point at, `// ^^^ rule-id` (`#` in YAML; carets give the span length, the marker may be left out when the span starts in the first columns). No annotation means no diagnostic. The id is checked, so a fixture also shows other rules that fire by default.
- `<name>.fixed.<ext>` is the text after the safe fixes, `<name>.unsafe.<ext>` after the unsafe ones too. A missing file means the fixes change nothing (resp. nothing more). Every fix run also checks that the file stays parseable and that a second run is idempotent.
- `rules.yaml` in the folder is the `lint.rules` mapping (default `<folder>: warn`; a family folder lists its rules); `<name>.rules.yaml` next to an input replaces it for that input alone, which is how a case sets options. Shared rules (`sorted-keys`, `no-duplicate-keys`, ...) have fixtures in each language's folder that tags the concept.
- `SUDSY_BLESS=1 cargo test -p lang-json --test integration rule_fixtures` (or `lang-yaml`, `lang-csharp`) rewrites the annotations and the `.fixed`/`.unsafe` files from what the rules do now; review the diff.
- Guards fail the language's test: every rule of the language is named by a folder with an input or by an annotation (a rule that never reports gets a folder named for it with a case that stays silent), no two inputs are the same text with the same rules, and no input is over 60 lines (annotations left out; split it or drop the repeated cases).
- One test per language lists every failing file with its message. Cases the runner cannot express stay in `crates/*/tests`: option validation errors, config-level on/off, depth and project scope.

### Adding a rule
`cargo xtask new-rule <language> <rule-id> [--rust]` does steps 1 and 2 with a starter that reports the node the language's `crates/lang-<language>/starter.<Kind>.<ext>` marks (a fixture whose last annotation `^^^ ID` marks a node of kind `<Kind>`; the gate passes as generated; edit the rule and its fixture, then `SUDSY_BLESS=1 cargo test -p lang-<language> --test integration rule_fixtures`) and prints the parity-catalog lines for step 3. Languages: those with a starter file. Without `--rust` it writes the pattern file (`crates/lang-<language>/rules/<id>.yaml`, every language); with `--rust` a `rule!` module under `src/rules/` listed in the language's registry (`src/rules/mod.rs` for C#, `src/rules.rs` for the others). The starters report an empty statement (C#), `{}` (the data languages) or `<b/>` (XML). Run `cargo fmt` after.
1. A `rule!` declaration in the rule's module (id, category, description, default, fix, options with checks and descriptions; [../rules/engine.md](../rules/engine.md#rule-trait)) and register the rule.
   A syntax rule that fits a pattern is a file instead: `crates/lang-<language>/rules/<rule-id>.yaml` ([../rules/patterns.md](../rules/patterns.md#built-in-pattern-rules)), no Rust.
2. A fixture folder `tests/rules/<language>/<rule-id>/` (or a case in the family folder) as above.
3. The matching entry in `docs/parity/<tool>.yaml` (`<upstream id>: rule <id>`) when another tool has an equivalent.
4. `cargo xtask codegen`: the rule table, option tables and `sudsy.schema.json` are generated; do not edit them by hand.

## Corpora (not fetched in CI)
- C#: `tests/conformance/fetch-corpus.sh` → pinned shallow sparse checkouts: dotnet/runtime v10.0.0 `System.Private.CoreLib/src`, dotnet/roslyn tag Visual-Studio-2026-Version-18.7 (Compilers CSharp/Core Portable, Workspaces Core Portable), dotnet/aspnetcore v10.0.0 (Http, Mvc.Core, Middleware, Kestrel Core, Components, SignalR common). Every file parses with `TARGET_WINDOWS`, `TARGET_AMD64`, `TARGET_64BIT` (files named `*.Browser.*` with `TARGET_BROWSER`, `TARGET_WASM`, `TARGET_32BIT`) so `#else #error` branches do not fail.
- Markdown: `fetch-markdown-corpus.sh` (CommonMark 0.31.2 `spec.json` by hash; the dotnet/runtime `docs/coding-guidelines` files at the pinned tag). `cargo test -p lang-markdown --test integration -- --ignored --nocapture commonmark::` counts the examples whose tree prints back and has the structure of pulldown-cmark; `cargo test --profile corpus -p lang-markdown --test integration -- --ignored markdown::` requires every example to render to the same HTML (pulldown-cmark) after formatting and to be unchanged by a second run.
- INI: `fetch-ini-corpus.sh` (pinned commits: the `.ini` files of ansible, php.ini-development of php-src). `cargo test --profile corpus -p lang-ini --test integration -- --ignored --nocapture conformance`.
- Corpora fetched as GitHub tarballs (shell, PowerShell, Dockerfile, gitignore: `fetch-gitignore-corpus.sh`) share `tarball-corpus.sh`.
- Shell: `fetch-shell-corpus.sh` (pinned commits: dotnet, nvm, rustup, docker and Homebrew installers, postgres and mysql entrypoints, pyenv, bats-core, asdf). `cargo test --profile corpus -p lang-shell --test integration -- --ignored --nocapture conformance`; format parity with shfmt: `scripts/parity.sh format shfmt`.
- PowerShell: `fetch-powershell-corpus.sh` (pinned commits: dotnet-install.ps1, the modules and tools of PowerShell, the engine and tests of PSScriptAnalyzer, the sources of Pester). `cargo test --profile corpus -p lang-powershell -- --ignored --nocapture conformance`; format parity with Invoke-Formatter: `scripts/parity.sh format invoke-formatter`.
- GitHub Actions: `fetch-github-actions-corpus.sh` (pinned: actionlint v1.7.12 test workflows, actions/starter-workflows). `cargo test --profile corpus -p lang-github-actions --test integration -- --ignored --nocapture conformance`; lint parity with actionlint: `scripts/parity.sh actionlint`.
- Protobuf: `fetch-proto-corpus.sh` (pinned commits: googleapis common and service protos, protobuf's own protos with the proto2, proto3 and editions test files, grpc's). `cargo test --profile corpus -p lang-protobuf --test integration -- --ignored --nocapture conformance`; format parity with buf: `scripts/parity.sh format buf`.
- GraphQL: `fetch-graphql-corpus.sh` (pinned commits: Prettier's GraphQL format tests, graphql-js's benchmark documents, GraphiQL's schemas and queries, Apollo Federation's supergraph schemas, graphql-eslint's mocks). `cargo test --profile corpus -p lang-graphql --test integration -- --ignored --nocapture conformance`; format parity with Prettier: `scripts/parity.sh format prettier`.
- Makefile: `fetch-makefile-corpus.sh` (pinned commits: the Makefiles of redis, git, lua, prometheus, kubernetes `build/root`, docker/cli). `cargo test --profile corpus -p lang-makefile --test integration -- --ignored --nocapture conformance`; lint parity with checkmake: `scripts/parity.sh checkmake`.
- HCL: `fetch-hcl-corpus.sh` (pinned commits: terraform-aws-vpc and -eks, Terraform's fmt and config test data, Nomad job files, Packer's examples and test templates; empty files removed). `cargo test --profile corpus -p lang-hcl --test integration -- --ignored --nocapture conformance`; format parity with `terraform fmt`: `scripts/parity.sh format terraform`.
- JSON: `fetch-json-corpus.sh` (JSONTestSuite `1ef36fa`). YAML: `fetch-yaml-corpus.sh` (yaml-test-suite `6ad3d2c`).

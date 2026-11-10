# Architecture overview

A Rust core owns everything language-neutral; each language contributes a provider. Only C# parsing and semantic queries run outside Rust, in a .NET worker. Every other language stays in Rust and never starts .NET.

## Crates (`crates/`)

| Crate | Role |
|---|---|
| `core-syntax` | Tree model, kinds, trivia rule, grammar loader and codegen, wire encoding |
| `core-incremental` | Salsa database: parse, options, format, lint, semantics as queries |
| `core-format` | Layout doc, printer, shapes, directives, comments, safety check |
| `core-rules` | Rule trait and scopes, diagnostics, fixes, suppressions, shared rules, extension set |
| `core-semantics` | Capability layers, per-file answers, symbol ids, behavior summaries |
| `core-flow` | Flow graph, worklist solver, reachability, liveness, reaching defs, definite assignment |
| `core-patterns` | Pattern matcher and YAML pattern rules |
| `core-plugins` | wasmtime host for Wasm rule plugins |
| `core-packs` | Language packs: embedded manifest, hash and ABI check, `dlopen`, installed set ([language-packs.md](language-packs.md)) |
| `pack` | The cdylib of one tree-sitter grammar (one cargo feature per language), built by `scripts/pack.sh` |
| `core-treesitter` | Adapter from a linked-in tree-sitter grammar to our lossless tree (mapping table, trivia from byte gaps, error regions) |
| `core-config` | `sudsy.yaml` model, chain, resolution, presets, `.editorconfig`, JSON Schema |
| `core-workspace` | File discovery (ignore files, hidden dirs, `.git` excluded) |
| `provider-api` | `LanguageProvider`, out-of-process proxy, protocol types, `format_file`, test harness |
| <!-- generated:crates -->`lang-csharp`, `lang-dockerfile`, `lang-editorconfig`, `lang-gitignore`, `lang-graphql`, `lang-ini`, `lang-json`, `lang-makefile`, `lang-markdown`, `lang-powershell`, `lang-protobuf`, `lang-shell`, `lang-toml`, `lang-xml`, `lang-github-actions`, `lang-yaml`<!-- /generated --> | Parser (JSON, YAML; TOML, XML and the rest through tree-sitter), formatter, rules, presets; C# adds the worker provider |
| `languages` | Built-in provider list, option schemas, `Project` (config found from a directory, validated) |
| `cli`, `lsp` | Front ends. `lsp` is a library; the server is `sudsy lsp` (one binary) |
| `plugin-sdk` | Public crate for rule authors |

Other: `xtask` (`cargo xtask codegen [--check]`), `tools/perf-bench`, `tools/wire-bench`, `workers/csharp`, `grammars/`, `wit/rules.wit`, `editors/vscode`, `schemas/sudsy.schema.json` (generated).

## Dependency rules
- Language crates depend on core crates, never the reverse. Shared behavior lives in a core crate.
- No parser types cross the provider boundary: the core sees only our tree, defined by our grammar. Roslyn kinds and trivia stay in the worker.
- Built-in C# rules are Rust (`lang-csharp`); the worker never runs rules, it parses and answers semantic queries.
- One diagnostics and fix model: built-in rules, pattern rules and Wasm plugins report into it.
- Nodes are addressed across boundaries by `(file, version, span, kind)`.
- Presets are pure config data; no code branches on a preset or tool name.

## Data flow
`discover files -> File input -> config -> selection -> options -> parse (provider; C# via worker) -> format | lint -> fix loop`. CLI and LSP set inputs on one database and read queries; see [incremental.md](incremental.md).

## Provider (`provider_api::LanguageProvider`)
A provider parses with a hand-written parser (JSON, YAML), the .NET worker (C#) or tree-sitter through `core-treesitter` (TOML, XML, Markdown, EditorConfig, INI, Gitignore, Dockerfile, Shell, PowerShell; [adding-a-language.md](adding-a-language.md)), so a language with a good tree-sitter grammar costs a mapping table and a formatter instead of a parser.

`id`, `language` (the name), `grammar` (parsed on first use: a built-in grammar is a `grammars::LazyGrammar` known by name and id, and `startup` costs the same for any number of languages), `file_matchers`, `capabilities`, `parse` (fallible: an out-of-process provider can be unavailable), formatter and option schema, presets, `rules`/`body_kinds`, `open_workspace`, `semantics(file, layers)`, `summaries`, `compiler_diagnostics` (test gate only). For C#, only parse, semantics and flow go to the worker.

## Versioned boundaries
Each is checked at handshake or load; a mismatch is refused. Why: workers and plugins are separate artifacts, and drift would mean reading kinds that mean something else.

| Boundary | Version |
|---|---|
| Grammar | SHA-256 over lexemes (comments/layout ignored, declaration order counts) |
| Tree wire encoding | `WIRE_VERSION = 1` |
| Worker protocol | `PROTOCOL_VERSION = 3` (+ grammar hash, flow-graph format version) |
| Layout directives | `DIRECTIVES_VERSION = 1` (text form shared by pattern files and the plugin WIT) |
| Plugin WIT | `sudsy:rules@0.2.0` / API `rules@0.2` (0.2 adds `layout` and `rule-info.layout`) |

# Sudsy docs

Current state only; history is in git. Update the doc for the area you change in the same commit.

| Doc | Covers |
|---|---|
| [architecture/overview.md](architecture/overview.md) | Crates, dependency rules, data flow, versioned boundaries |
| [architecture/syntax-tree.md](architecture/syntax-tree.md) | Lossless tree, trivia rule, grammars and codegen, wire encoding |
| [architecture/csharp-worker.md](architecture/csharp-worker.md) | .NET worker, protocol, workspace, lifecycle |
| [architecture/adding-a-language.md](architecture/adding-a-language.md) | Adding a tree-sitter language: steps and per-language cost |
| [architecture/language-packs.md](architecture/language-packs.md) | Languages as installable packs: format, manifest, cache, ABI check, missing language |
| [architecture/incremental.md](architecture/incremental.md) | Salsa query graph, backdating, parallelism |
| [formatting/engine.md](formatting/engine.md) | Layout doc, printer, shapes, directives, comments, safety check |
| [formatting/csharp.md](formatting/csharp.md) | C# formatter and options |
| [formatting/json.md](formatting/json.md) | JSON/JSONC parser, formatter, options |
| [formatting/yaml.md](formatting/yaml.md) | YAML parser, data model, formatter, options |
| [formatting/toml.md](formatting/toml.md) | TOML parser (tree-sitter), formatter, options, conformance |
| [formatting/xml.md](formatting/xml.md) | XML parser (tree-sitter), well-formedness checks, formatter, options, conformance |
| [formatting/editorconfig.md](formatting/editorconfig.md) | `.editorconfig` parser (tree-sitter), formatter, options, conformance |
| [formatting/ini.md](formatting/ini.md) | `.ini` parser (tree-sitter), formatter, options, rules, conformance |
| [formatting/dockerfile.md](formatting/dockerfile.md) | Dockerfile parser (tree-sitter), formatter, options, grammar gap, conformance |
| [formatting/shell.md](formatting/shell.md) | Shell (sh and Bash) parser (tree-sitter), formatter after shfmt, parser limits, conformance |
| [formatting/powershell.md](formatting/powershell.md) | PowerShell parser (tree-sitter), formatter after PSScriptAnalyzer's Invoke-Formatter, parity, conformance |
| [formatting/markdown.md](formatting/markdown.md) | Markdown parser (two tree-sitter grammars), formatter, safety checks, rules, conformance |
| [formatting/presets.md](formatting/presets.md) | Preset list and notes |
| [parity/README.md](parity/README.md) | Parity catalog: what yamllint, Prettier, Biome, eslint-plugin-jsonc, StyleCop, CSharpier do and where Sudsy stands |
| [rules/engine.md](rules/engine.md) | Rule trait, scopes, layers and depth, levels, fixes, suppressions |
| [rules/builtin.md](rules/builtin.md) | Built-in rules |
| [rules/semantics.md](rules/semantics.md) | Flow graphs, analyses, summaries, compiler agreement |
| [rules/patterns.md](rules/patterns.md) | Pattern rule files |
| [rules/plugins.md](rules/plugins.md) | Wasm plugins and `plugin-sdk` |
| [config/sudsy-yaml.md](config/sudsy-yaml.md) | Config model, resolution, extends, overrides, schema |
| [config/editorconfig.md](config/editorconfig.md) | `.editorconfig` mapping |
| [config/cli.md](config/cli.md) | Commands, exit codes, output formats |
| [editor/lsp.md](editor/lsp.md) | LSP server behaviour, analysis thread, cancellation |
| [editor/vscode.md](editor/vscode.md) | VS Code extension, packaging, dogfooding |
| [performance/numbers.md](performance/numbers.md) | Current numbers per hot path |
| [performance/tooling.md](performance/tooling.md) | How to measure: bench.sh, perf-bench, speed scripts |
| [development/dev-loop.md](development/dev-loop.md) | Build, gate, scripts, Windows/Git Bash/WSL notes |
| [development/parity.md](development/parity.md) | `parity.sh`: differential tests against upstream tools |
| [development/testing.md](development/testing.md) | Test layers, corpora, conformance |
| [development/site.md](development/site.md) | The public rule documentation site: build, data sources, deploy switch |
| [development/ci-release.md](development/ci-release.md) | CI jobs, release process, placeholders |
| [roadmap.md](roadmap.md) | Deliberately not built or deferred |

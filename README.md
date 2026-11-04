# Sudsy

A formatter and linter for C#, JSON (including JSONC), YAML and TOML, configured from one `sudsy.yaml`.

- **Formatting**: every layout decision is a named option; presets reproduce the styles of Microsoft, the .NET runtime, Google, CSharpier and Prettier. Each result is checked to have the same syntax tree (and for YAML the same data) as the input before it is written.
- **Linting**: built-in rules for all four languages, semantic C# rules (types, control and data flow), safe and unsafe fixes, suppression comments, [pattern rules](docs/rules/patterns.md) in YAML and [Wasm plugins](docs/rules/plugins.md) (unstable API).
- **Implementation**: a Rust core. JSON and YAML are parsed natively and TOML by an embedded tree-sitter grammar; C# is parsed and analysed by a Roslyn-backed .NET worker that the CLI starts when it needs it.

Measured speed against CSharpier, Prettier, Biome and dprint, with the method and machine: [docs/performance/numbers.md](docs/performance/numbers.md).

## Install

- **CLI**: download the archive for your platform (Linux, Windows or macOS; x64 or arm64) from the GitHub releases page and put the `sudsy` executable on your `PATH`. Keep the `csharp-worker/` directory beside it.
- **VS Code**: install the extension `Coox.sudsy`. It bundles the CLI and the worker.
- **C#** needs the [.NET 10 runtime](https://dotnet.microsoft.com/download/dotnet/10.0). JSON, YAML and TOML work without it.

## Quick start

```sh
sudsy init --preset csharp/microsoft   # writes a starter sudsy.yaml
sudsy format                           # format the current directory in place
sudsy lint                             # report rule violations
sudsy fix                              # apply fixes, then format
sudsy rules                            # list the rules; `sudsy rules <id>` explains one
```

In CI, `sudsy check` runs `format --check` and `lint` together and exits 1 when a file is unformatted or a rule reports an error:

```sh
sudsy check --format github   # ::error file=...:: annotations in the GitHub Actions log
sudsy check --format sarif > sudsy.sarif   # for code scanning upload
```

Output formats are `human`, `json`, `sarif` and `github`. Exit codes and all commands: [docs/config/cli.md](docs/config/cli.md).

## Presets

Presets are ordinary config files; writing their options by hand in `sudsy.yaml` gives the same output.

| Preset | Style |
|---|---|
| `csharp/microsoft` | The default |
| `csharp/dotnet-runtime` | Microsoft plus inline accessor bodies |
| `csharp/google` | 2 spaces, 100 columns, K&R braces |
| `csharp/csharpier-compat` | CSharpier 1.x layout |
| `csharp/stylecop` | StyleCop.Analyzers layout (blank lines, `)` placement, empty bodies) and its matching rules |
| `json/default`, `yaml/default`, `toml/default` | The defaults |
| `json/prettier-compat`, `yaml/prettier-compat` | Prettier-like (width 80) |
| `toml/taplo-compat` | taplo's defaults |

Without a `sudsy.yaml`, C# uses `csharp/microsoft`, existing `.editorconfig` keys apply, and JSON, YAML and TOML use their defaults. Details: [docs/formatting/presets.md](docs/formatting/presets.md).

## Configuration

```yaml
extends: [preset:csharp/google, preset:json/default, preset:yaml/default]
exclude: ["**/Generated/**"]
analysis: standard            # fast | standard | deep
csharp:
  format: { line_width: 120 }
  lint:
    rules:
      no-empty-catch: error
overrides:
  - files: ["tests/**"]
    csharp: { lint: { rules: { async-suffix: "off" } } }
```

`sudsy config print` shows the resolved config and where each value came from. A JSON Schema is in [schemas/sudsy.schema.json](schemas/sudsy.schema.json). Reference: [docs/config/sudsy-yaml.md](docs/config/sudsy-yaml.md), [.editorconfig mapping](docs/config/editorconfig.md).

## Editor setup

The VS Code extension runs `sudsy lsp` and gives diagnostics, quick fixes, fix all and formatting. Format on save is your choice, per language:

```json
{
  "[csharp]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "[json]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "[yaml]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }
}
```

Any editor with an LSP client can start `sudsy lsp` (stdio). See [docs/editor/vscode.md](docs/editor/vscode.md) and [docs/editor/lsp.md](docs/editor/lsp.md).

## Documentation

[docs/README.md](docs/README.md) indexes everything: architecture, per-language formatting options, the rule engine and built-in rules, pattern rules, plugins, performance, CI and release. Planned and deliberately skipped work: [docs/roadmap.md](docs/roadmap.md). Changes: [CHANGELOG.md](CHANGELOG.md).

## License and contributing

MIT, copyright Maia Coox ([LICENSE](LICENSE)); test and benchmark inputs from other projects are listed in [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md). Contributions need a DCO sign-off; see [CONTRIBUTING.md](CONTRIBUTING.md).

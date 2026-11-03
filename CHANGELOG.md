# Changelog

## [0.1.0] - unreleased

First release: one tool, as a CLI and a VS Code extension, that formats, lints and fixes C#, JSON
(including JSONC) and YAML from a single `sudsy.yaml`.

### Formatting
* Lossless shared syntax tree for all three languages; every layout decision is an option.
* C# through a Roslyn-backed .NET worker (framework-dependent, roll-forward); JSON and YAML are
  native Rust and never start .NET.
* Presets: `csharp/microsoft`, `csharp/dotnet-runtime`, `csharp/google`, `csharp/csharpier-compat`,
  `json/default`, `json/prettier-compat`, `yaml/default`, `yaml/prettier-compat`.
* Every format is checked for an unchanged tree (and unchanged data for YAML) before it is written.

### Rules and fixes
* Built-in rules for C#, JSON, YAML and every language, with safe, unsafe and suggestion-only
  fixes, suppression comments (`sudsy-disable-next-line`) and the fix loop.
* Semantic C# rules on Roslyn binding, types and values, and on control and data flow computed in
  the core from the worker's flow graphs: unreachable code, unused variables, assignments, usings
  and private members, null dereferences. Analysis depth `fast`, `standard` or `deep`.
* Pattern rules: ast-grep-style YAML rule files with metavariables, constraints and fixes
  (`plugins: patterns:`).
* Wasm plugins (API v0, **unstable**): Rust rules compiled to a component, using the same `Rule`
  trait, the flow graph and the pattern matcher (`plugin-sdk`, `wit/rules.wit`, `plugins: wasm:`).

### Tools
* CLI: `format`, `lint`, `fix`, `check`, `rules`, `config print`, `init`; human, JSON, SARIF and
  GitHub Actions output.
* LSP server and VS Code extension: diagnostics, quick fixes, fix all, format on save, a
  long-lived C# worker with an incrementally updated workspace.
* Packages for linux, macOS and Windows (x64 and arm64): a CLI archive and a VS Code extension
  each, with the C# worker.

### Known limits
* The semantic workspace is the `.cs` files under the opened roots, not an MSBuild project:
  package types do not resolve (`remove-unused-usings` stays silent for files with errors).
* Pattern rules have no `type_matches` predicate yet; `var-preference` and
  `prefer-collection-expression` are not built; range formatting is not available.
* The Wasm plugin API changes without notice until v1.

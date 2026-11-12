# Changelog

## [0.1.0-rc.1] - unreleased

First release candidate: one formatter and linter, as a CLI and a VS Code extension, configured by
one `sudsy.yaml`. Every layout decision is an option; presets are plain config.

### C# (needs the .NET 10 runtime)
* Formatter with options for brace style, wrapping, blank lines, usings and more; presets
  `csharp/google`, `csharp/csharpier-compat`, `csharp/stylecop`, `csharp/dotnet-runtime` and
  `csharp/unity`. Without a preset the layout is Microsoft's.
* Lint rules on Roslyn binding, types and control flow (unreachable code, unused variables and
  assignments, usings, private members, null dereferences) plus the StyleCop-style layout,
  naming and documentation rules. `analysis: fast|standard|deep` sets how much runs.
* Unity: `csharp/unity` counts `Awake`, `Update`, `OnTriggerEnter` and the other message methods
  as used; `.asmdef`, `.asmref`, `.inputactions` (JSON), `.uss` (CSS) and `.uxml` (XML) are
  formatted. Unity's serialized YAML (`.unity`, `.prefab`, `.asset`, `.meta`) is left alone.

### Data and config files
* JSON and JSONC, YAML, TOML, XML (`.csproj`, `.props`, `.resx`, `.xaml`, ...), INI, `.editorconfig`,
  `.gitignore`: formatting and lint rules; presets `json/prettier-compat`, `json/biome`,
  `yaml/prettier-compat`, `yaml/yamllint`, `toml/taplo-compat`, `xml/csharpier-compat`.
* GitHub Actions workflows and Dockerfiles, Protobuf, HCL, Makefile.

### Web, scripts and docs
* CSS, SCSS and Less, JavaScript and TypeScript (JSX/TSX), HTML with embedded script and style,
  GraphQL, Markdown (embedded code blocks are formatted by their languages).
* Shell and PowerShell lint.

### Tools
* CLI: `format`, `lint`, `fix`, `check`, `rules`, `config print`, `init` (including migration from
  yamllint, Prettier, Biome, ESLint, StyleCop and CSharpier configs); human, JSON, SARIF and GitHub
  Actions output.
* VS Code extension (language server): diagnostics, quick fixes, fix all, format on save, a
  long-lived C# worker with an incrementally updated workspace. The `.vsix` contains the server
  and the worker.
* Rules beyond the built-in ones: ast-grep-style pattern rules (`plugins: patterns:`) and Wasm
  plugins (API v0, **unstable**).
* Packages for linux, macOS and Windows (x64 and arm64).

### Known limits
* The C# workspace is the `.cs` files under the opened roots, not an MSBuild project: package types
  do not resolve, so `remove-unused-usings` stays silent for files with errors.
* `unused-private-member` does not see calls by name (`Invoke("Name")`, `StartCoroutine("Name")`).
* Range formatting is not available.
* The Wasm plugin API changes without notice until v1.

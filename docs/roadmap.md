# Roadmap: not built, or deferred

One line each; none is committed.

## Languages and analysis
- INI: `.cfg`, `.gitconfig`, `.gitmodules`, `.properties` (tox.ini and setup.cfg continuation lines, gitconfig keys without `=` and repeated multi-value keys, Java properties syntax); tree-sitter-ini has no continuation lines or bare keys, so they would be refused or get syntax errors. Needs a grammar fork (generated parser vendored) or a hand-written parser, and a dialect per family.
- TOML: TOML 1.1 syntax (the tree-sitter grammar targets 1.0; needs an upstream fix or a fork).
- XML: the inside of a DOCTYPE (the grammar cannot parse every DTD declaration) and DTD-dependent well-formedness (entities, attribute defaults); reflowing text (the formatter keeps mixed content as written); a `xmlWhitespaceSensitivity: ignore` mode; sorting `PackageReference` items.
- Gitignore syntax: `.dockerignore`-specific rules (root-only `*.log`).
- Dockerfile: rules (hadolint parity); shell code inside `RUN` as a Shell language injected through the composite adapter (the formatter keeps it as written); (low priority) vendor tree-sitter-containerfile's `scanner.c` with a fix for consecutive `[`-led continuation lines, which make 24 corpus files unparseable ([formatting/dockerfile.md](formatting/dockerfile.md)).
- Shell: rules (shellcheck subset); reformatting the inside of `$(...)`, `(( ))` and arrays like shfmt ([formatting/shell.md](formatting/shell.md)).
- PowerShell: rules (PSScriptAnalyzer subset) ([formatting/powershell.md](formatting/powershell.md)).
- Makefile: the files the grammar rejects (7 of 59 in the corpus, [formatting/makefile.md](formatting/makefile.md)) need a grammar fix or fork; shell code in `$(shell ...)` and `define` bodies; rules beyond checkmake's five.
- GitHub Actions: the actionlint checks not built ([parity/actionlint.yaml](parity/actionlint.yaml): types of expressions, events, globs, matrix, permission scopes, action metadata); a matcher for `.github/workflows` as a path, not a folder name ([formatting/github-actions.md](formatting/github-actions.md)).
- Protobuf: the cross-file buf lint rules ([parity/buf.yaml](parity/buf.yaml)); the remaining `buf format` differences ([formatting/protobuf.md](formatting/protobuf.md)).
- GraphQL: `# prettier-ignore`, the graphql-eslint rules that need no schema and are not built (`docs/parity/graphql-eslint.yaml`), schema-aware rules.
- HCL: the tflint rules that need the other files of the module or the providers, `terraform_deprecated_lookup` and the module source rules ([parity/tflint.yaml](parity/tflint.yaml)); the rewrites `terraform fmt` does besides layout (quoted block labels, quoted `variable` types), which would be rules ([formatting/hcl.md](formatting/hcl.md)); `sudsy-ignore` in HCL.
- Many more languages, each with a formatter and rules matching its de-facto tools, native rules only, a worker process per language where its valuable rules need types (like the C# worker). Ordered by coverage and cost, languages whose toolchain ships a formatter or linter last: INI / `.properties` / `.gitconfig`, ignore files, GraphQL; JavaScript / TypeScript / JSX / TSX (Prettier, ESLint, typescript-eslint), CSS / SCSS / Less (stylelint), HTML, Vue / Svelte, Razor; Python (ruff, black), SQL (sqlfluff), Java (google-java-format, Checkstyle), Kotlin (ktlint), PHP (PHP-CS-Fixer), Ruby (RuboCop), C / C++ (clang-format), Lua, R, Scala, Haskell; then the languages with official tools: Swift, Elixir, Dart, Go, Rust, Zig ([architecture/adding-a-language.md](architecture/adding-a-language.md)).
- Language packs, remaining: signing the packs (they are hash-checked against the embedded manifest, not signed).
- Plugin (Wasm) overhead: encode the tree once per file for all plugin rules, run a plugin's rules in one call, send only the node kinds it asks for.
- Type-aware rules per language use the language's reference checker, never our own, through a worker (the C# worker's protocol): Python with Pyrefly (preferred) or ty in-process if either can be used as a Rust library (spike first; pyright is too slow), TypeScript on the native compiler's API (7.x beta API now, the stable 7.1 API when it lands), Java (Eclipse JDT) and Kotlin (Analysis API) in one JVM host, then C / C++ (libclang). After the syntax-level sweep of the languages above; none for Ruby, Go, Rust, Swift for now.
- Replacing a tree-sitter grammar with a hand-written parser where speed or error messages matter.
- Native C# parser (Roslyn stays the only one; the conformance suite and addressing by `(file, version, span, kind)` keep the door open).
- `MSBuildWorkspace` / project- and NuGet-aware binding with syntax-only fallback (replaces `Workspace.cs` without a protocol change).
- Point queries (`symbol_of`, `type_of(span)`) and the pattern predicate `type_matches` that needs them.
- Taint and resource-state analyses on `core-flow`; call-graph rules; `must_use_result`, ownership and taint summaries; a consumer for `pure`.
- Embedded languages in Razor and in C# raw strings; Markdown fences of C# (the worker would start for a snippet; see [architecture/adding-a-language.md](architecture/adding-a-language.md#embedded-languages)).
- `preprocessor_symbols` come from config only, language version is not configurable.
- Rules: `var-preference`, `prefer-collection-expression`; a rename fix for `async-suffix`; rules using reaching definitions / definite assignment.
- Semantic requests in parallel (one workspace per core).
- Migration aid mapping StyleCop / .NET analyzer ids and `.editorconfig` severities to our rules. Hosting Roslyn analyzers is a non-goal.

## Formatting
- Range (selection) formatting; the formatter can already format a subtree in context.
- Other CSharpier differences listed in [formatting/csharp.md](formatting/csharp.md).
- Rule-produced layout directives for `#if` branches formatted from other parses (the layout rules run on the file's own parse only); solving `#if` conditions to find symbol sets (`derive_symbol_sets` tries subsets of the condition symbols, at most 16 sets).
- A rule source that attaches layout directives (pattern `layout:` key, plugin directive output); plugins returning layout documents is deliberately excluded.
- YAML: re-wrapping long scalars and comments; comments between split properties.
- Performance levers: green-tree cursor for the formatter instead of rowan red nodes; sharing anchored YAML nodes (`Rc`) for alias-heavy files; first-token index for pattern rules.

## Extensibility
- Plugin SDKs for languages other than Rust (the WIT is documented); freezing the plugin API at v1.
- Host import for parsing snippets so plugins can compile patterns; sharing one decoded tree across the rule calls of a plugin; options schema for plugin rules.
- Captures kept through `inside`/`has`.
- A separate plugin-host binary to shrink the 28.3 MB binary (LTO measured: 7-12% smaller for 2-3x build time, see performance/numbers.md).

## Tooling and editor
- Pull-diagnostic result ids; editor watching of closed files for deep diagnostics; pre-starting the second worker so formatting during the first build is instant.
- SARIF `fixes`; an output-file option.
- A result cache across CLI runs (as `ruff`, `eslint --cache`): per file, keyed by its bytes, the Sudsy version, its resolved config, plugin and pattern file hashes and pack versions; semantic results are additionally keyed by a hash of every file of that language in the project, so a change that touches no file of the language (a docs-only or YAML-only commit in a mixed repository) keeps them. Per-result file dependencies (re-checking only the files that use what changed) are the finer step, the same fix the LSP's stale cross-file diagnostics need. Built when a measured CLI run on a large repository (pre-commit, CI) is too slow.
- Editor extensions other than VS Code (other editors use `sudsy lsp`).
- Self-contained C# worker for machines without .NET.
- Repeatable benchmarks for the paths listed in [performance/numbers.md](performance/numbers.md).
- Publish the documentation site ([development/site.md](development/site.md)): enable the Pages deploy job when the repository is public; then switch `DOCS_BASE_URL` on and add `codeDescription` links to diagnostics.
- Release: first Marketplace publish ([development/ci-release.md](development/ci-release.md)).

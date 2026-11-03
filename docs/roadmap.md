# Roadmap: not built, or deferred

One line each; none is committed.

## Languages and analysis
- Native C# parser (Roslyn stays the only one; the conformance suite and addressing by `(file, version, span, kind)` keep the door open).
- `MSBuildWorkspace` / project- and NuGet-aware binding with syntax-only fallback (replaces `Workspace.cs` without a protocol change).
- Point queries (`symbol_of`, `type_of(span)`) and the pattern predicate `type_matches` that needs them.
- Taint and resource-state analyses on `core-flow`; call-graph rules; `must_use_result`, ownership and taint summaries; a consumer for `pure`.
- Embedded-language detection (tree, wire and plugin interface already support the node).
- `preprocessor_symbols` come from config only, language version is not configurable.
- Rules: `var-preference`, `prefer-collection-expression`; a rename fix for `async-suffix`; rules using reaching definitions / definite assignment.
- Semantic requests in parallel (one workspace per core).
- Migration aid mapping StyleCop / .NET analyzer ids and `.editorconfig` severities to our rules. Hosting Roslyn analyzers is a non-goal.

## Formatting
- Range (selection) formatting; the formatter can already format a subtree in context.
- Other CSharpier differences listed in [formatting/csharp.md](formatting/csharp.md).
- A rule source that attaches layout directives (pattern `layout:` key, plugin directive output); plugins returning layout documents is deliberately excluded.
- YAML: re-wrapping long scalars and comments; comments between split properties.
- Performance levers: green-tree cursor for the formatter instead of rowan red nodes; a token-level or hashed safety check; sharing anchored YAML nodes (`Rc`) for alias-heavy files; first-token index for pattern rules.

## Extensibility
- Plugin SDKs for languages other than Rust (the WIT is documented); freezing the plugin API at v1.
- Host import for parsing snippets so plugins can compile patterns; sharing one decoded tree across the rule calls of a plugin; options schema for plugin rules.
- A separate plugin-host binary to shrink the 28.3 MB binary (LTO measured: 7-12% smaller for 2-3x build time, see performance/numbers.md).
- Pattern rule options; captures kept through `inside`/`has`.

## Tooling and editor
- Pull-diagnostic result ids; editor watching of closed files for deep diagnostics; pre-starting the second worker so formatting during the first build is instant.
- SARIF `fixes`; an output-file option; a switch to skip the safety check (deliberately absent).
- Editor extensions other than VS Code (other editors use `sudsy lsp`).
- Self-contained C# worker for machines without .NET.
- Repeatable benchmarks for the paths listed in [performance/numbers.md](performance/numbers.md).
- Release: first Marketplace publish ([development/ci-release.md](development/ci-release.md)).

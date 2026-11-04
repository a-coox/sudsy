# C# worker (`workers/csharp`, net10.0, roll-forward)

Roslyn parse and mapping to our grammar, semantic queries, flow-graph export. Framework-dependent: needs the .NET 10+ runtime; a missing or too-old runtime is one typed condition (`ProxyError::RuntimeMissing` / `ProviderError::RuntimeMissing` / `Stage::Runtime`), detected when the program cannot be spawned (not found) or when the `dotnet` host exits before the handshake reporting a missing framework (exit code 0x80008096 or its "You must install ..." / "framework ... was not found" message on stderr, which is also forwarded to our stderr). The editor shows a "Download .NET" popup once; the CLI prints, per C# file, `error: <file>: Sudsy needs the .NET 10 runtime to format and lint C#. JSON and YAML keep working. Download it from https://dotnet.microsoft.com/download/dotnet/10.0 (<reason>)` and continues with other languages (exit code 2 as for any failed file). Other failures stay generic.

## Layout
`Sudsy.Worker` (`Mapping/` Roslyn to our kinds, `Protocol/`, `Semantics/`), `Sudsy.Worker.Tests`, `Sudsy.Bench` (production-path parse/mapping/serialize benchmark).

## Protocol (v3)
Length-prefixed frames on stdin/stdout, MessagePack payloads (enums as one-entry maps, structs as arrays in field order; C# uses integer `[Key]`s; a unit test pins the bytes). One request in flight, no ids.

| Message | Purpose |
|---|---|
| `Hello` | Protocol version, grammar hash, flow-graph format, wire version, capabilities. Either side refuses a mismatch; not retried. |
| `OpenWorkspace` | Roots for the semantic compilation (the proxy remembers and resends after a restart) |
| `Parse` | file, version, text, language version, `preprocessor_symbols` -> wire tree + errors |
| `UpdateText` | Sent with every semantic request; the worker ignores unchanged text |
| `Query` | Typed items: Binding `unused_imports`, Types `declarations` and `symbol_findings`, Values `maybe_null`, Project `references`, plus `diagnostic_ids`. File-wide enumerations, not point queries. |
| `FlowGraphs` | Bodies of a file -> graphs (plain MessagePack structs) |
| `Shutdown` | Clean exit |

- Text has no BOM (`SourceFile::from_bytes` strips it, records `had_bom`); Roslyn UTF-16 offsets become UTF-8 byte offsets in the worker.
- The worker thread has a 256 MB stack (deeply nested expressions recurse deep in Roslyn).
- `ConcurrentGarbageCollection=false` (`Directory.Build.props`): batch process, the background GC thread slowed it ~20%. The proxy sets `DOTNET_TC_CallCountingDelayMs=0` so Roslyn leaves tier-0 JIT quickly.

## Mapping (`TreeMapper`)
Single explicit-stack walk; trivia re-attached to our rule. Missing tokens are zero-width `ERROR` nodes; a skipped-token run is an `ERROR` node. A fast path (plain trivia) and a general routine (missing/skipped tokens) must agree; a test and `Sudsy.Bench --verify` check it.

## Workspace (not `MSBuildWorkspace`)
One `CSharpCompilation` from the `.cs` files under the opened roots (directory, project directory, or a solution's project directories; `bin`, `obj`, `node_modules`, `.git` skipped) plus sent text, against the runtime's reference assemblies. No project references, NuGet, target frameworks, generators.
- Why: MSBuild loading needs the SDK at run time and fails on many setups; the rules built need little of it.
- Degradation: package types do not resolve; `remove-unused-usings` stays silent for files with errors or `#if` regions. Flow graphs barely depend on resolution.
- Limits (the repo is untrusted): symlinks and junctions are not entered, a `.cs` or `.sln` file over 4 MiB is not read, and a solution's projects are used only when they lie under the solution's directory.
- CLI: workspace is the command-line paths (else cwd). LSP: the workspace folder. Files outside are analyzed alone.

## Lifecycle (`provider_api::proxy`)
- Started lazily on the first C# file that needs it. Discovery: `SUDSY_CSHARP_WORKER` (path to `Sudsy.Worker.dll`), else `csharp-worker/Sudsy.Worker.dll` beside the executable; run via `dotnet` with the temp directory as its working directory (never the checked repo).
- Pool: parsing uses up to `min(cores, 4)` workers (`SUDSY_CSHARP_WORKERS` overrides), a new one only when all started are busy. Semantic state lives in the first worker only, so semantic requests serialize.
- Failure: failures in a row count against `max_restarts = 1`; any reply resets the count. Two in a row: unavailable until restart. Hang = no reply within 120 s. Start failure or handshake refusal is final. Semantic unavailability is warned once (CLI: per distinct reason).

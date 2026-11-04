# Semantic analysis

## Capability layers (`core-semantics`)
Providers declare layers, rules require them; JSON and YAML declare none.

| Layer | Answered in C# by |
|---|---|
| Binding | Roslyn (`unused_imports`) |
| Types | Roslyn (`declarations`: async, task-returning, implements, override flags) |
| Values | Roslyn (`maybe_null`) |
| Flow | Worker exports Roslyn control-flow graphs; analyses run in the core |
| Project | Roslyn references; project phase in the core |

`symbol_findings` (Types) is one walk over the file for the checks that need a symbol or type: `base` calls that bind the same as `this`, default value-type constructors, partly qualified usings in namespaces, `ValueTuple` spelled out, unnamed tuple elements in signatures, `Debug.Assert`/`Fail` messages, `inheritdoc` on members that inherit nothing. Each finding carries its span and the replacement text the fix uses; the rules in `symbol_checks.rs` only turn findings into diagnostics. Only the `Debug` message rules also run without the answers (by the written name).

Answers are file-wide enumerations (`Answers`, `Declaration`, `Reference`), not point queries. Symbol ids are Roslyn documentation-comment ids (`M:Shop.Orders.OrderService.GetRecentAsync(System.Int32)`). `fast` never touches the worker's model; a `standard` run with no semantic rule enabled asks for nothing.

## Flow (`core-flow`)
- Export: blocks of simple operations (local read/write/read-write/declare; calls with callee symbol id and attribute flags; one `Other` mark per root operation), edges (always/true/false), constant branch conditions. Every block of a `try` has an edge to each handler start. Locals captured by lambdas/local functions or address-taken are flagged and excluded from dead-store analysis; parameters are not tracked.
- Unreachable blocks stay in the export; the core recomputes reachability from edges, constants and summaries.
- Generic worklist solver (forward/backward, any lattice, edge hook). Analyses: reachability, liveness (dead stores), reaching definitions, definite assignment (the last two have tests but no rule; the compiler already reports CS0165).
- `unreachable-code` reports the first statement of a run, only when none of its operations can run (`return c ? throw e : 0;` is not reported); never an unreachable `throw` or empty statement (compiler agreement). A body Roslyn cannot build a graph for (unresolved symbols) falls back to the syntactic check with the same exceptions.

## Behavior summaries (`BehaviorSummaries`)
- `does_not_return`: `[DoesNotReturn]`, built-in list (`Environment.Exit`, `Environment.FailFast`), and `csharp: semantics: does_not_return:` (qualified names, every overload).
- `pure`: `[Pure]` and `semantics: pure:`; `is_pure` exists but no fix consumes it.

## Editor
The workspace folder is the worker's workspace. With `analysis: deep` the first pull reads the project's other files from disk (include/exclude, once per session or config reload) and the project phase runs over all of them; a pull returns the project-scope findings for that file.

## Compiler agreement gate
`crates/lang-csharp/tests/compiler_agreement.rs`: a 36-method fixture (jumps, loops, switches, try/finally, labels, lambdas, local functions, iterators, patterns, `using`, deconstruction, out variables); the flow rules must report exactly the compiler's CS0162, CS0168, CS0219 positions (`non_returning_calls: false`). On the three corpora (warning-clean code, so mostly a false-positive check): 0 differences in aspnetcore and Roslyn; one CS0219 in CoreLib (`PortableThreadPool.GateThread.cs`) the compiler reports and we do not (a struct local assigned `default` whose only use is a property read). `perf-bench semantic DIR [LIMIT]` runs it.

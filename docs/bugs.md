# Known bugs

Open bugs only, one entry each: symptom, cause, suggested fix. The commit that fixes a bug deletes its entry.

## LSP: C# semantic diagnostics go stale when another file changes
- Symptom: editing `B.cs` (move a type to another namespace, change a nullable annotation or a base type) leaves `A.cs`'s semantic findings (`unused-import`, type-based rules, `maybe_null`, deep `references` facts) as they were until `A.cs` is edited, reopened or the config reloads.
- Cause: `semantics(file)` and `lint(file)` ([core-incremental/src/rules.rs](../crates/core-incremental/src/rules.rs)) read only the file's own inputs; `didChange` of B sets only B's input, so `lint(A)` stays memoized and a client re-pull returns it. `WorkspaceDiagnosticRefresh` is sent only on config reload ([lsp/src/server.rs](../crates/lsp/src/server.rs)). With `analysis: deep`, unopened files are read from disk once and never refreshed. No test edits B and re-pulls A.
- Suggested fix: a `workspace_revision` input bumped on open/change/close (and background load), read by `semantics` only when layers are non-empty, so format and syntax-only lint stay unaffected; plus a debounced diagnostics refresh after edits. Cost: one worker round trip per open C# file per edit.

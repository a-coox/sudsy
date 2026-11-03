# LSP server (`crates/lsp`, run as `sudsy lsp`)

Built on `lsp-server` + `lsp-types` (synchronous scaffold, no async runtime); `Connection::memory()` lets tests drive the real server in-process (`crates/lsp/tests`; `cli/tests/lsp.rs` checks the stdio wiring). Why not `tower-lsp`: one thread owns the one database.

## Behaviour
- Sync: incremental; positions in UTF-16 (no negotiation). Only `file:` URIs.
- Diagnostics: pull (`textDocument/diagnostic`, full reports, no result ids, no workspace diagnostics); a client without pull support gets none. Results are memoized per version; the client's own debounce is the idle trigger. Project-scope findings appear only with `analysis: deep` (see [semantics](../rules/semantics.md)).
- Code actions: one quick fix per fix of a diagnostic touching the range (unsafe titled "(unsafe)", never preferred); "Fix all `rule` problems in this file" (two or more fixable problems, one round of non-overlapping fixes, offered only if the result parses); `source.fixAll.sudsy` for `editor.codeActionsOnSave` (the `fix` loop, safe fixes only, offered only if it changes the text).
- Formatting: one edit between common prefix and suffix, so cursors and folds stay. A file that cannot be formatted (syntax errors, unavailable worker) gets no edits and a log message, not a protocol error. Range formatting is not offered.
- Config: one config for the workspace (first folder, else cwd). Reloaded when the client reports a change of `sudsy.yaml`/`.yml`, `.editorconfig` or a file in the chain (dynamic watchers on `**/*.yaml`, `**/*.yml`, `**/.editorconfig`, filtered); the client is asked to refresh diagnostics. Unsaved edits of a config file take effect on save. An invalid config puts the same error on every file and is shown once; a missing worker/.NET is shown once as a warning ("C# needs the .NET runtime (10 or newer)") and JSON/YAML keep working. Opened files are processed like explicit CLI arguments (`include`/`exclude` do not apply).
- Closed files are not watched: an on-disk edit of one is seen after the next config reload or restart.

## Analysis thread and cancellation (`analysis.rs`)
The client pulls diagnostics on every keystroke without debounce and cancels the one in flight. The first semantic pull builds the workspace compilation (seconds).
- One thread runs `textDocument/diagnostic` and `textDocument/codeAction` in order on a snapshot of `Workspace` (`Clone`: database handle, document list, `Arc` project). The message loop answers everything else (formatting, notifications), so it never waits on a semantic request. Code actions queue behind the pull of the same version (same memoized `lint`).
- Inputs without waiting: setting a salsa input blocks while another handle lives, so while the analysis thread holds one, `didChange` gives the document a new `File` input instead (a few unreferenced inputs per edit made during a request). A config reload still sets inputs and waits; the running request is answered `ServerCancelled`.
- Cancellation: `$/cancelRequest` for a queued or running request answers `RequestCancelled` (-32800). A `didChange`/`didClose` of the document a request is about answers it `ServerCancelled` (-32802, `retriggerRequest: true`). Queued requests are dropped before starting; a running one stops at its next checkpoint (after the lint query) and its result is not sent. A worker call already running is not interrupted, so a superseded first pull still costs the build once (memoized for the next).
- Formatting during the first build uses the pooled parse workers: it pays the start of a second worker (~1 s) instead of the end of the build.
- Tests: `crates/lsp/tests/responsiveness.rs` uses a rule that blocks until released (no sleeps).

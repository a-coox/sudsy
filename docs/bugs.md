# Known bugs

Open bugs only, one entry each: symptom, cause, suggested fix. The commit that fixes a bug deletes its entry.

- **`lsp` test `a_cancelled_pull_is_answered_with_request_cancelled` flakes under load** (`crates/lsp/tests/responsiveness.rs`, `#[ignore]` until fixed). Symptom: fails when other builds load the machine, passes alone (about 2,800 runs alone did not reproduce it). Cause: not found. The test assumes the server has read both `$/cancelRequest` notifications before it answers the `format` sent after them; under load that ordering may not hold. Suggested fix: confirm by logging the order the loop reads messages in, then make the test wait for an observable point (or make the loop drain pending notifications before answering a request), and remove the `#[ignore]`.

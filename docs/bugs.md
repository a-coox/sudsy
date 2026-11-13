# Known bugs

Open bugs only, one entry each: symptom, cause, suggested fix. The commit that fixes a bug deletes its entry.

- **`lsp` test `a_cancelled_pull_is_answered_with_request_cancelled` flakes under load** (`crates/lsp/tests/responsiveness.rs`, `#[ignore]` until fixed). Symptom: fails when other builds load the machine, passes alone (about 2,800 runs alone did not reproduce it). Cause: not found. The test assumes the server has read both `$/cancelRequest` notifications before it answers the `format` sent after them; under load that ordering may not hold. Suggested fix: confirm by logging the order the loop reads messages in, then make the test wait for an observable point (or make the loop drain pending notifications before answering a request), and remove the `#[ignore]`.
- **JS `no-undef` (off)**: it knows no globals (`expect`, `btoa`, `clearTimeout`), so it reports them. Fix: an `env`/`globals` default per file kind, then turn it on.
- **HTML `no-duplicate-keys` on conditional attributes**: `{{ if a }}id="x"{{ else }}id="y"{{ end }}` repeats `id` in the text and is reported. Template tags are otherwise skipped (`hooks::in_template`). Fix: treat attributes in different branches of a template conditional as distinct. `no-duplicate-class` is off by default (redundant, not a bug).
- **Dockerfile `copy-from-unknown-stage` / `copy-from-own-stage` after `ADD url#${VAR}`** (kept on): the grammar ends the instruction at `#`, so the next `FROM` is lost. Fix: patch the grammar fork.
- **JS `no-redeclare` (off) and `no-import-assign` (on)**: neither knows TypeScript declaration merging (namespace + const, function + interface, imported type + local value).
- **First active rule costs about 5M instructions per bench input in protobuf** (lint 532,677 with no rule on, 5,477,200 with `enum-first-value-zero`): engine setup (rule compilation, traversal dispatch tables), not the rule. Look at it when lint startup is tuned.

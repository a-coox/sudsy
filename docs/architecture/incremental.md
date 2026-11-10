# Incremental engine (`core-incremental`, salsa 0.28)

## Graph
- Inputs: `File { path, version, text, had_bom }`, singleton `ConfigFile { path, text }`, `ExtensionSet` (pattern rules and plugins). The provider list is fixed per database.
- Queries: `config` -> `selection` (provider + dialect from the path) -> `options` -> `language_version` (the only part of options parsing reads, so an `indent_size` edit keeps the parse) -> `parse` (green tree + errors) -> `format` (safety check inside in debug builds; `Ok(text)` or a failure; it reads `layout(file)`, the directives of the layout rules, which reads `parse`, the extension set and `lint_settings` and nothing when the language has no layout rule). Also `lint_settings(file)`, `semantics(file)` (asks only the layers active rules need and the depth allows; when that is any, it also reads the `WorkspaceRevision` input, which the LSP bumps after edits, since another file feeds the model; `format` and syntax-only `lint` never read it) and `lint(file)`.
- Fix loop: `fix_text` / `transient` (same file, other text) are shared by `sudsy fix` and the editor's fix-all.

## Behaviour
- Failures are values (`Failure { stage, message }`, `Clone + Eq`): memoized and backdated; a missing worker is `Err`, never a panic. Editors retry by setting the file again (always bumps the revision).
- `parse` stores the `GreenNode` (`Send + Sync`, structural `==`); the red root is rebuilt in O(1). Backdating pays off at `config`, `options` and `format` (a whitespace edit with equal output stops there).
- Parallelism: database handles are `Clone` and share memos; setting an input needs `&mut` and waits for other handles. Rule: set inputs on one handle, then fan out (`std::thread::scope`, atomic path counter; output ordered by path). Memos live until exit.
- Rule results depend on lint settings, not format options: a format-option edit re-runs no rule.
- Extension rules (pattern files, plugins) are leaked once per distinct text, so an unchanged reload keeps equal rule objects and invalidates nothing.

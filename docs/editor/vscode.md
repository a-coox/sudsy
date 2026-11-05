# VS Code extension (`editors/vscode`)

TypeScript, `vscode-languageclient` 9 (pull diagnostics), bundled with esbuild into `dist/extension.js`. It only starts `<server> lsp` and restarts it when `sudsy.serverPath` or `sudsy.csharpWorkerPath` change. No logic beyond that.

- Documents: <!-- generated:languages -->C#, EditorConfig, JSON, Markdown, TOML, XML and YAML<!-- /generated --> (JSON includes JSONC; `.cs` and `.toml` also by pattern, so a missing language extension does not matter). Commands: restart server, show output.
- Settings: `sudsy.serverPath` (default: bundled, else `sudsy` on PATH), `sudsy.csharpWorkerPath` (default: beside the executable); both have `machine` scope, so a workspace's `.vscode/settings.json` cannot set them (a malicious repo would otherwise run its own executable), `sudsy.trace.server`.
- Rule documentation: every Sudsy diagnostic gets the quick fix "Show documentation for <rule>", which opens the rule's page (Markdown preview of a virtual `sudsy-rule:` document) rendered by `src/ruleDocs.ts` from `schemas/rules.json`, a copy of `docs/rules/rules.json` made at build time and shipped in the package, so it works offline and matches the installed version. `DOCS_BASE_URL` in `ruleDocs.ts` is the one place for the site address (`https://a-coox.github.io/sudsy/rules/<id>`); diagnostics carry no `codeDescription` link until the site is live.
- Format on save is the user's choice: `editor.defaultFormatter: Coox.sudsy` per language, plus `"editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }`. The extension sets no defaults (see `editors/vscode/README.md`).
- Missing .NET runtime: the popup and its "Download .NET" button come from the server (`window/showMessageRequest`, `window/showDocument`), which `vscode-languageclient` handles by default; the extension has no code for it.
- Config schema: `yamlValidation` for `sudsy.yaml`/`.yml` with a copy of `schemas/sudsy.schema.json` made at build time (`npm run build`, not checked in). `redhat.vscode-yaml` reads it when installed; it is not a dependency. Why: C#/JSON users should not need a second YAML extension that also competes as YAML formatter.

## Packaging
A package is `server/sudsy[.exe]` plus `server/csharp-worker/` (framework-dependent `dotnet publish` output, English resources only), made by `scripts/stage-server.mjs`; the server finds the worker beside itself, so no setting is needed. `npm run package -- --target <platform>` makes the platform `.vsix` (~7 MB; the Wasm host makes the binary 28.3 MB against 8.8 MB before it). Targets: linux/win32/darwin x x64/arm64.

## Tests
Pure logic (which executable/worker to start) under `node --test` (`npm test`). Protocol behaviour is covered by `crates/lsp/tests`. No test starts a VS Code instance (it would download one per CI run); "used daily" is a manual gate.

## Dogfooding
`.vscode/settings.json` makes Sudsy the formatter for this repo. The path settings cannot live there (machine scope); put the two lines `scripts/dogfood.sh` prints (`target/dogfood/sudsy`, `target/dogfood/worker/Sudsy.Worker.dll`) in your user settings; `.vscode/launch.json` starts an extension host. `scripts/dogfood.sh` builds the debug CLI and worker and copies them to `target/dogfood/` so VS Code never locks the build outputs; restart the server with the extension's command afterwards.

## Workspace Trust
The manifest declares `capabilities.untrustedWorkspaces.supported: false` and `virtualWorkspaces: false`: in Restricted Mode the extension does not activate, because it runs an executable and reads repo-supplied config (patterns, Wasm plugins).

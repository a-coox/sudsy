# VS Code extension (`editors/vscode`)

TypeScript, `vscode-languageclient` 9 (pull diagnostics), bundled with esbuild into `dist/extension.js`. It only starts `<server> lsp` and restarts it when `sudsy.serverPath` or `sudsy.csharpWorkerPath` change. No logic beyond that.

- Documents: C#, JSON, JSONC, YAML (`.cs` also by pattern, so a missing C# extension does not matter). Commands: restart server, show output.
- Settings: `sudsy.serverPath` (default: bundled, else `sudsy` on PATH), `sudsy.csharpWorkerPath` (default: beside the executable), `sudsy.trace.server`.
- Format on save is the user's choice: `editor.defaultFormatter: sudsy.sudsy` per language, plus `"editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }`. The extension sets no defaults (see `editors/vscode/README.md`).
- Config schema: `yamlValidation` for `sudsy.yaml`/`.yml` with a copy of `schemas/sudsy.schema.json` made at build time (`npm run build`, not checked in). `redhat.vscode-yaml` reads it when installed; it is not a dependency. Why: C#/JSON users should not need a second YAML extension that also competes as YAML formatter.

## Packaging
A package is `server/sudsy[.exe]` plus `server/csharp-worker/` (framework-dependent `dotnet publish` output, English resources only), made by `scripts/stage-server.mjs`; the server finds the worker beside itself, so no setting is needed. `npm run package -- --target <platform>` makes the platform `.vsix` (~7 MB; the Wasm host makes the binary 28.9 MB against 8.8 MB before it). Targets: linux/win32/darwin x x64/arm64.

## Tests
Pure logic (which executable/worker to start) under `node --test` (`npm test`). Protocol behaviour is covered by `crates/lsp/tests`. No test starts a VS Code instance (it would download one per CI run); "used daily" is a manual gate.

## Dogfooding
`.vscode/settings.json` makes Sudsy the formatter for this repo and points the extension at `target/dogfood/sudsy` and `target/dogfood/worker/Sudsy.Worker.dll`; `.vscode/launch.json` starts an extension host. `scripts/dogfood.sh` builds the debug CLI and worker and copies them to `target/dogfood/` so VS Code never locks the build outputs; restart the server with the extension's command afterwards.

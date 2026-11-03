# Sudsy for VS Code

Formats and lints C#, JSON and YAML with Sudsy: diagnostics, quick fixes, fix all
and format on save, from the `sudsy.yaml` of your repository.

To format on save, choose Sudsy as the formatter of each language in your settings:

```json
{
  "[csharp]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "[json]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "[jsonc]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "[yaml]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }
}
```

`sudsy.yaml` itself gets completion and validation (through the Red Hat YAML extension, if you have it installed).

C# needs the .NET 10 runtime; JSON and YAML work without it.

## Settings

* `sudsy.serverPath`: the `sudsy` executable (default: the bundled one, else `sudsy` on PATH).
* `sudsy.csharpWorkerPath`: `Sudsy.Worker.dll` (default: the one beside the executable).
* `sudsy.trace.server`: `off`, `messages` or `verbose`.

## Building

`npm ci`, then `npm run lint`, `npm test` and `npm run build`. `npm run stage-server -- <release
sudsy> <published worker directory>` copies the native binary and the worker into `server/`, and
`npm run package -- --target <platform>` makes the `.vsix` of that platform (see
`.github/workflows/ci.yml`).

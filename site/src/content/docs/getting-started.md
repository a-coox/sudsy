---
title: Getting started
description: Install Sudsy, format and lint a project, and use it in VS Code.
---

## Install

- **CLI**: download the archive for your platform (Linux, Windows or macOS; x64 or arm64) from the GitHub releases page and put the `sudsy` executable on your `PATH`. Keep the `csharp-worker/` directory beside it.
- **VS Code**: install the extension `Coox.sudsy`. It bundles the CLI and the worker.
- **C#** needs the [.NET 10 runtime](https://dotnet.microsoft.com/download/dotnet/10.0). Every other language works without it.

## Format and lint

```sh
sudsy init --preset csharp/microsoft   # writes a starter sudsy.yaml
sudsy format                           # format the current directory in place
sudsy lint                             # report rule violations
sudsy fix                              # apply fixes, then format
sudsy rules <id>                       # explain one rule
```

In CI, `sudsy check` runs `format --check` and `lint` together and exits 1 when a file is unformatted or a rule reports an error. `--format github` and `--format sarif` write annotations and code scanning reports.

Rules and their levels are set in `sudsy.yaml`:

```yaml
extends: [preset:csharp/stylecop]
lint:
  rules:
    comment-space: warn
```

See the [rules](/sudsy/rules/), [presets](/sudsy/reference/presets/) and [format options](/sudsy/reference/format-options/).

## VS Code

The extension gives diagnostics, quick fixes, fix all and formatting. Format on save is your choice, per language:

```json
{
  "[csharp]": { "editor.defaultFormatter": "Coox.sudsy", "editor.formatOnSave": true },
  "editor.codeActionsOnSave": { "source.fixAll.sudsy": "explicit" }
}
```

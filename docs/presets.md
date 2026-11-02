# Presets

`extends: [preset:<language>/<name>]`. A preset is an ordinary `sudsy.yaml`
(`crates/lang-*/presets/<name>.yaml`); copy it and you get the same output. Presets have no
versions: `preset:csharp/google@1` is a config error.

| Preset | Notes |
|---|---|
| `csharp/microsoft` | The defaults. Width 120, not the docs' 65 columns (which only fit documentation snippets). |
| `csharp/dotnet-runtime` | Microsoft plus `inline_accessor_bodies`. |
| `csharp/google` | 2 spaces, 100 columns, K&R braces. No alignment of the first argument: wrapped lines continue with 4 spaces. Trailing comments are not aligned. Arguments go one per line when they do not fit. |
| `csharp/csharpier-compat` | CSharpier 1.x layout through the named options of the C# provider. |
| `json/default` | The defaults. |
| `json/prettier-compat` | Width 80, objects stay expanded after an opening line break. Number arrays are not filled and numbers are not normalized. |
| `yaml/default` | The defaults (styles preserved). |
| `yaml/prettier-compat` | Width 80, `quote_style: prefer_double`, `blank_lines_after_key: remove`. |

Sample input and expected output for each preset: `tests/fixtures/presets/`.

# Presets

`extends: [preset:<language>/<name>@<version>]`. A preset is only option values
(`crates/lang-*/presets.yaml`); write the same options in `sudsy.yaml` and you get the same output.
Changing a preset means adding `@2`.

| Preset | Notes |
|---|---|
| `csharp/microsoft@1` | The defaults. Width 120, not the docs' 65 columns (which only fit documentation snippets). |
| `csharp/dotnet-runtime@1` | Microsoft plus `inline_accessor_bodies`. |
| `csharp/google@1` | 2 spaces, 100 columns, K&R braces. No alignment of the first argument: wrapped lines continue with 4 spaces. Trailing comments are not aligned. Arguments go one per line when they do not fit. |
| `csharp/csharpier-compat@1` | CSharpier 1.x layout through the named options of the C# provider. |
| `json/default@1` | The defaults. |
| `json/prettier-compat@1` | Width 80, objects stay expanded after an opening line break. Number arrays are not filled and numbers are not normalized. |
| `yaml/default@1` | The defaults (styles preserved). |
| `yaml/prettier-compat@1` | Width 80, `quote_style: prefer_double`, `blank_lines_after_key: remove`. |

Sample input and expected output for each preset: `tests/fixtures/presets/`.

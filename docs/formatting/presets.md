# Presets

`extends: [preset:<language>/<name>]`. A preset is an ordinary `sudsy.yaml` (`crates/lang-*/presets/<name>.yaml`, embedded with `include_str!`); writing its options by hand gives the same output. Sample input and expected output per preset: `tests/fixtures/presets/` (each has a `SOURCE.md` noting deviations from the published guide).

Why plain config and unversioned: no hidden values or code paths per tool, and an unversioned name means `preset:csharp/google@1` is a config error. Presets that change would reformat code, so they change deliberately, with the sample fixtures updated.

| Preset | Notes |
|---|---|
| `csharp/microsoft` | The defaults. Width 120, not the docs' 65 columns (which only fit documentation snippets). |
| `csharp/dotnet-runtime` | Microsoft plus `inline_accessor_bodies`. |
| `csharp/google` | 2 spaces, 100 columns, K&R braces, `{}` for empty bodies. No alignment of the first argument: wrapped lines continue with 4 spaces. Trailing comments not aligned. Arguments go one per line when they do not fit. |
| `csharp/csharpier-compat` | CSharpier 1.x layout through the named C# options. |
| `csharp/stylecop` | StyleCop.Analyzers 1.2.0-beta.556 default layout: `closing_paren_line: same_line`, `empty_braces: expanded`, `blank_lines_between_members: always`, `blank_lines_after_block: require`, `blank_line_before_comment: require`, `blank_line_after_comment: remove`, `space_after_delegate: false`, `trailing_commas: multiline`; and the lint rules that match StyleCop's style rules (`comment-space`, `doc-comment-space`, `directive-keyword-space`, `no-regions`, ...) at `warn`. Checked against StyleCop itself by `scripts/parity.sh stylecop`. |
| `json/default` | The defaults. |
| `json/prettier-compat` | Width 80, LF, objects stay expanded after an opening line break (`collapse: opening_break`); `package.json`, `package-lock.json` and `composer.json` fully expanded (`overrides`, `collapse: never`). Number arrays not filled, numbers not normalized. |
| `json/biome` | Biome's defaults: tab indent, width 80, LF, `collapse: opening_break`; the same `package.json` override. |
| `yaml/default` | The defaults (styles preserved). |
| `yaml/yamllint` | yamllint's default config: lint levels (line length 80 at error, `no-undefined-alias` at error, `ambiguous-booleans`, `document-start` and `comment-space` at warn; yamllint's other rules are off by default and so stay off) and output that satisfies it: `comment_spacing: 2`, `max_blank_lines: 2`, `sequence_indent: indented`, width 80. `bracket_spacing: false` (`{a: 1}`) and `end_of_line: lf`. |
| `yaml/prettier-compat` | Width 80, LF, `quote_style: prefer_double`, `blank_lines_after_key: remove`. |

## Mechanics
- A preset goes through the same merge as an `extends` file, below all files in the chain (not interleaved at the position of the entry). `config print` shows `preset <name>` as the source.
- A preset may extend other presets only (no relative files) and may not set `include`, `exclude`, `analysis` or `plugins`. Its override globs are relative to the root config's directory. Cycles are caught.
- Two tests keep presets pure: a hand-written config with a preset's values formats every sample like `extends`, and no formatter source mentions "csharpier".
- Zero-config (no `sudsy.yaml`): C# uses `csharp/microsoft`; `.editorconfig` keys are honored; JSON/YAML use their defaults (2-space indent, YAML quotes kept, keys never reordered).

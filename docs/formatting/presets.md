# Presets

`extends: [preset:<language>/<name>]`. A preset is an ordinary `sudsy.yaml` (`crates/lang-*/presets/<name>.yaml`, embedded with `include_str!`); writing its options by hand gives the same output. Sample input and expected output per preset: `tests/fixtures/presets/` (each has a `SOURCE.md` noting deviations from the published guide).

Why plain config and unversioned: no hidden values or code paths per tool, and an unversioned name means `preset:csharp/google@1` is a config error. Presets that change would reformat code, so they change deliberately, with the sample fixtures updated.

| Preset | Notes |
|---|---|
| `csharp/microsoft` | The defaults. Width 120, not the docs' 65 columns (which only fit documentation snippets). |
| `csharp/dotnet-runtime` | Microsoft plus `inline_accessor_bodies`. |
| `csharp/google` | 2 spaces, 100 columns, K&R braces, `{}` for empty bodies. No alignment of the first argument: wrapped lines continue with 4 spaces. Trailing comments not aligned. Arguments go one per line when they do not fit. |
| `csharp/csharpier-compat` | CSharpier 1.x layout through the named C# options, including `sort_usings: true`. |
| `csharp/stylecop` | StyleCop.Analyzers 1.2.0-beta.556 default layout: `closing_paren_line: same_line`, `empty_braces: expanded`, `blank_lines_between_members: always`, `blank_lines_after_block: require`, `blank_line_before_comment: require`, `blank_line_after_comment: remove`, `space_after_delegate: false`, `space_between_empty_for_clauses: true`, `trailing_commas: multiline`; and the lint rules that match StyleCop's style rules (`comment-space`, `doc-comment-space`, `directive-keyword-space`, `no-regions`, `member-order`, `accessor-order`, `using-order` with `compare: culture`, `static_position: after` and `alias_position: after`, ...) and the 24 documentation comment rules ([rules/builtin.md](../rules/builtin.md#c-documentation-comment-rules)) at `warn`. Checked against StyleCop itself by `scripts/parity.sh stylecop`. |
| `json/default` | The defaults. |
| `json/prettier-compat` | Width 80, LF, objects stay expanded after an opening line break (`collapse: opening_break`); `package.json`, `package-lock.json` and `composer.json` fully expanded (`overrides`, `collapse: never`). Number arrays not filled, numbers not normalized. |
| `json/biome` | Biome's defaults: tab indent, width 80, LF, `collapse: opening_break`; the same `package.json` override. |
| `yaml/default` | The defaults (styles preserved). |
| `toml/default` | The defaults. |
| `toml/taplo-compat` | taplo 0.10 defaults: width 80, two spaces, LF, `collapse: always` (an array that fits goes on one line), up to two blank lines (`max_blank_lines: 2`) and `align_comments: true`. Checked against taplo by `scripts/parity.sh format taplo`. |
| `markdown/prettier-compat` | Prettier 3.9 with `proseWrap: preserve`: two-column list alignment, LF. Checked against Prettier by `scripts/parity.sh format prettier`. |
| `protobuf/buf-standard` | buf's STANDARD lint category: the rules Sudsy has at `warn` (the formatter has no style options). Checked against buf by `scripts/parity.sh buf`. |
| `hcl/tflint-recommended` | The rules of tflint-ruleset-terraform's recommended preset that Sudsy has, at `warn` (the formatter has no style options). |
| `xml/default` | The defaults. |
| `xml/csharpier-compat` | CSharpier 1.x XML defaults: width 100, two spaces, line endings kept. |
| `css/stylelint-recommended` | The rules of stylelint-config-recommended that Sudsy has, at `error` (stylelint's severity), the lint rules for `.css`, `.scss` and `.less`; `declaration-block-no-duplicate-properties` with `ignore_consecutive_different_values`. The formatter has no preset options. Checked against stylelint by `scripts/parity.sh stylelint`. |
| `css/stylelint-standard` | The recommended preset and the rules of stylelint-config-standard that Sudsy has (notation, naming patterns with stylelint's kebab-case regex, `number-max-precision: 4`, `length-zero-no-unit`, ...) at `error`. The empty-line, vendor-prefix, color-function and keyword-case rules of the standard set are not built yet. |
| `yaml/yamllint` | yamllint's default config: lint levels (line length 80 at error, `no-undefined-alias` at error, `ambiguous-booleans`, `document-start` and `comment-space` at warn; yamllint's other rules are off by default and so stay off) and output that satisfies it: `comment_spacing: 2`, `max_blank_lines: 2`, `sequence_indent: indented`, width 80. `bracket_spacing: false` (`{a: 1}`) and `end_of_line: lf`. |
| `yaml/prettier-compat` | Width 80, LF, `quote_style: prefer_double`, `blank_lines_after_key: remove`. |

## Mechanics
- A preset goes through the same merge as an `extends` file, below all files in the chain (not interleaved at the position of the entry). `config print` shows `preset <name>` as the source.
- A preset may extend other presets only (no relative files) and may not set `include`, `exclude`, `analysis` or `plugins`. Its override globs are relative to the root config's directory. Cycles are caught.
- Two tests keep presets pure: a hand-written config with a preset's values formats every sample like `extends`, and no formatter source mentions "csharpier".
- Zero-config (no `sudsy.yaml`): C# uses `csharp/microsoft`; `.editorconfig` keys are honored; JSON/YAML/TOML/XML/Markdown use their defaults (2-space indent, YAML quotes kept, keys never reordered).

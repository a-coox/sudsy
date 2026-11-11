# Parity catalog

What the tools Sudsy aims to replace do, and where Sudsy stands. One file per tool (`docs/parity/<tool>.yaml`); `tool`, `version` (upstream; the parity runner installs it), `languages`, `sources` and `scope` head each file. A test (`crates/languages/tests/parity.rs`) fails when a `rule` or `option` status names something that does not exist; the counts below are generated (`cargo xtask codegen`). The differential runner [`scripts/parity.sh`](../development/parity.md) compares a tool's output with `sudsy lint` on fixtures.

## Format
One line per upstream rule or option under `entries:`, `<id>: <status>` or `<id>: <status> | <gap>`; the id of an option is `line-length.max`, so tools with many sub-options weigh more in the counts. `<gap>` says what is missing and makes a `rule`, `option`, `formatter` or `config` entry `partial`; for `planned` and `deferred` it is the reason. Ids that are only deferred are grouped by reason under a top-level `deferred:` (`- gap: "<reason>"`, `ids: [DL3049, DL3050]`). `languages::catalog::parse` expands both forms; coverage follows from the status, so it is not written.

| Status | Meaning | Coverage |
|---|---|---|
| `rule <id>` | A Sudsy rule does it (`syntax-error` is the parser's diagnostic); the rule's `sudsy rules` page and the table in [rules/builtin.md](../rules/builtin.md) list the entry as its equivalent. | covered, partial with a gap |
| `option <language>.format.<name>` | A formatter option does it. | covered, partial with a gap |
| `formatter` | Fixed formatter behavior, no option. | covered, partial with a gap |
| `config <text>` | Configuration outside rules and options (overrides, ignore files, presets, suppression comments). | covered, partial with a gap |
| `sdk <id>` | A .NET SDK diagnostic (IDE, CA, CS) does it; Sudsy never reimplements these. | sdk |
| `planned` | Accepted for implementation. | missing |
| `deferred` | Wanted eventually, deliberately not scheduled. | deferred |
| `wontfix <reason>` | Out of scope. | wontfix |

## Counts
<!-- generated:counts -->
| Tool | Entries | Covered | Partial | Missing | SDK | Wontfix | Deferred | Planned |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| actionlint | 26 | 5 | 4 | 0 | 0 | 0 | 17 | 0 |
| biome | 34 | 13 | 6 | 2 | 0 | 13 | 0 | 2 |
| buf | 47 | 20 | 3 | 0 | 0 | 0 | 24 | 0 |
| checkmake | 5 | 4 | 1 | 0 | 0 | 0 | 0 | 0 |
| csharpier | 16 | 11 | 5 | 0 | 0 | 0 | 0 | 0 |
| dotnet-format-editorconfig | 56 | 50 | 3 | 0 | 1 | 2 | 0 | 0 |
| eslint | 292 | 82 | 11 | 136 | 0 | 32 | 31 | 136 |
| eslint-plugin-jsonc | 44 | 36 | 6 | 0 | 0 | 2 | 0 | 0 |
| graphql-eslint | 35 | 4 | 6 | 0 | 0 | 0 | 25 | 0 |
| hadolint | 71 | 38 | 10 | 0 | 0 | 1 | 22 | 0 |
| markdownlint | 53 | 25 | 16 | 0 | 0 | 0 | 12 | 0 |
| prettier | 57 | 18 | 5 | 23 | 0 | 10 | 1 | 23 |
| psscriptanalyzer | 75 | 12 | 8 | 0 | 0 | 1 | 54 | 0 |
| roslynator | 67 | 23 | 7 | 16 | 18 | 1 | 2 | 16 |
| shellcheck | 520 | 17 | 8 | 0 | 0 | 0 | 495 | 0 |
| shfmt | 9 | 1 | 1 | 0 | 0 | 3 | 4 | 0 |
| stylecop | 197 | 120 | 10 | 1 | 45 | 21 | 0 | 1 |
| stylelint | 150 | 84 | 8 | 54 | 0 | 0 | 4 | 54 |
| stylelint-less | 6 | 0 | 0 | 5 | 0 | 0 | 1 | 5 |
| stylelint-scss | 71 | 1 | 2 | 67 | 0 | 0 | 1 | 67 |
| taplo | 23 | 11 | 7 | 0 | 0 | 0 | 5 | 0 |
| terraform | 2 | 1 | 1 | 0 | 0 | 0 | 0 | 0 |
| tflint | 20 | 8 | 4 | 0 | 0 | 0 | 8 | 0 |
| yamllint | 65 | 62 | 3 | 0 | 0 | 0 | 0 | 0 |
<!-- /generated -->

## Migration
`sudsy init --from <tool>` ([config/cli.md](../config/cli.md#init---from)) turns a tool's config into a `sudsy.yaml` with this catalog as the source of truth: the catalog files are embedded in the binary, the reason printed for a setting that cannot be migrated is the entry's `gap` (or its status), and the C# `.editorconfig` entries that switch off covered SA rules come from the `covered` StyleCop entries. The option and rule values are mapped in code (`crates/cli/src/init/`); each mapping names its catalog entry and debug builds fail when the entry points elsewhere. A new `rule:` or `option:` status therefore changes what `init` writes for the settings that use it; the fixtures under `tests/fixtures/init/` show the result.

## Equivalent ids of rules
`sudsy rules <id>` and the table in [rules/builtin.md](../rules/builtin.md) list as `Equivalent` every `covered` or `partial` entry whose status is `rule <id>` in a catalog for one of the rule's languages (`languages::catalog::equivalents`). Rule descriptions do not name other tools; add or fix the entry here instead.

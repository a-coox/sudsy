# Parity catalog

What the tools Sudsy aims to replace do, and where Sudsy stands. One file per tool: `yamllint.yaml`, `prettier.yaml` (YAML and JSON), `biome.yaml` (JSON), `eslint-plugin-jsonc.yaml`, `stylecop.yaml` (StyleCop.Analyzers 1.2.0-beta.556), `csharpier.yaml`, `dotnet-format-editorconfig.yaml` (the IDE0055 formatting options and `.editorconfig` keys), `roslynator.yaml` (overlaps with the others only). Upstream versions are in each file's `version`. Sudsy facts were observed on a debug build; entries without a "Verified" note follow from the documentation of the tool.

The differential runner [`scripts/parity.sh`](../development/parity.md) compares a tool's output with `sudsy lint` on fixtures. A test (`crates/languages/tests/parity.rs`) fails when a `rule:` or `option:` status names something that does not exist, or when the counts below are stale.

## Entry fields
`id`, `name`, `default` (upstream default), `kind` (`lint` or `format-option`), `sudsy` (status), `coverage`, `notes`, `gap` (what is missing), `priority` (`p1` on by default upstream, `p2` popular opt-in, `p3` rare; judgment, not measured), `usage` (judgment), `batch` (planning group, see below). Option-level entries have ids like `line-length.max`, so tools with many sub-options weigh more in the counts.

## Status (`sudsy`)
| Status | Meaning |
|---|---|
| `rule: <id>` | A Sudsy rule does it (or most of it, see `coverage`). `syntax-error` is the parser's diagnostic. |
| `option: <language>.format.<name>` | A formatter option does it. |
| `formatter` | Fixed formatter behavior, no option. |
| `sdk: <id>` | A .NET SDK diagnostic (IDE, CA, CS) does it. Sudsy never reimplements these; the rules that overlap one say so in `sudsy rules <id>`. |
| `config: <text>` | Configuration outside rules and formatter options (overrides, ignore files, presets, suppression comments). |
| `planned` | Accepted for implementation; `coverage` shows today's state. |
| `deferred` | Wanted eventually, deliberately not scheduled (the StyleCop SA16xx documentation rules need an XML doc-comment parser first). |
| `wontfix: <reason>` | Out of scope. |

## Coverage
`covered` (at the upstream default, or via a non-default option, as the notes say), `partial` (the primary mapping exists, `gap` says what is missing), `missing`, `sdk`, `wontfix`, `deferred`.

## Counts
| Tool | Entries | Covered | Partial | Missing | SDK | Wontfix | Deferred | Planned |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| biome | 34 | 13 | 6 | 2 | 0 | 13 | 0 | 2 |
| csharpier | 16 | 9 | 6 | 0 | 0 | 1 | 0 | 2 |
| dotnet-format-editorconfig | 56 | 39 | 11 | 1 | 1 | 4 | 0 | 1 |
| eslint-plugin-jsonc | 44 | 36 | 6 | 0 | 0 | 2 | 0 | 0 |
| prettier | 28 | 15 | 4 | 1 | 0 | 8 | 0 | 1 |
| roslynator | 67 | 23 | 7 | 16 | 18 | 1 | 2 | 16 |
| stylecop | 197 | 73 | 4 | 28 | 45 | 21 | 26 | 28 |
| yamllint | 65 | 40 | 10 | 15 | 0 | 0 | 0 | 15 |

## Batches
Planned work is grouped so related entries land together.

| Batch | Scope |
|---|---|
| B1 yaml-defaults | yamllint defaults: comment spacing, `truthy` options, `line-length` for unbreakable words, a yamllint-compatible preset |
| B2 shared-format-options | `end_of_line`, `bracket_spacing`, JSON/YAML options several tools share |
| B3 yaml-optin-lint | yamllint opt-in rules: anchors, empty values, float and octal values, quoted strings, document end |
| B4 json-optin-lint, sorted-keys-options | JSON opt-in rules and `sorted-keys` parity |
| B5 stylecop-lexical | comment and preprocessor spacing, empty statements, token checks |
| B6 stylecop-syntax | syntax-structure checks, one type per file, file name matches type |
| B7 stylecop-ordering | element and using order |
| B8 stylecop-layout-formatter | blank-line and closing-parenthesis layout as formatter options |
| B9 stylecop-semantic | rules that need the type layer |
| B10 stylecop-docs | documentation-comment rules (deferred) |
| B11 csharpier-gaps | using sort, inactive `#if` branches, layout differences |
| B12 editorconfig-mapping | the `.editorconfig` keys not mapped yet |
| B13 existing-rule-options | options of rules that exist |

# Parity catalog

What the tools Sudsy aims to replace do, and where Sudsy stands. One file per tool: `yamllint.yaml`, `prettier.yaml` (YAML, JSON and Markdown), `markdownlint.yaml` (markdownlint 0.42, rules MD001-MD060), `biome.yaml` (JSON), `eslint-plugin-jsonc.yaml`, `hadolint.yaml` (hadolint 2.15.1, rules DL1001-DL4006; the ShellCheck rules it runs on RUN are out of scope), `stylecop.yaml` (StyleCop.Analyzers 1.2.0-beta.556), `taplo.yaml` (TOML), `shfmt.yaml` (Shell format), `psscriptanalyzer.yaml` (PowerShell format), `shellcheck.yaml` (shellcheck 0.11.0, checks SC1000-SC3062), `csharpier.yaml`, `dotnet-format-editorconfig.yaml` (the IDE0055 formatting options and `.editorconfig` keys), `roslynator.yaml` (overlaps with the others only). Upstream versions are in each file's `version`. Sudsy facts were observed on a debug build; entries without a "Verified" note follow from the documentation of the tool.

The differential runner [`scripts/parity.sh`](../development/parity.md) compares a tool's output with `sudsy lint` on fixtures. A test (`crates/languages/tests/parity.rs`) fails when a `rule:` or `option:` status names something that does not exist, or when the counts below are stale.

## Entry fields
`id`, `name`, `default` (upstream default), `kind` (`lint` or `format-option`), `sudsy` (status), `coverage`, `notes`, `gap` (what is missing), `priority` (`p1` on by default upstream, `p2` popular opt-in, `p3` rare; judgment, not measured), `usage` (judgment), `batch` (planning group, see below). Option-level entries have ids like `line-length.max`, so tools with many sub-options weigh more in the counts.

### Compact deferred entries
Entries that are only deferred (no notes) may instead be listed under a top-level `deferred:` of a catalog, grouped by their gap; each id becomes an entry with status and coverage `deferred` and that `gap` (no name, default, priority, usage or batch), counted like any other:

```yaml
deferred:
  - gap: "Needs the configured label schema."
    ids: [DL3049, DL3050]
```
The readers (`languages::catalog::parse`, used by the counts test, `init` and the rule tables) expand the list; scripts that read `entries` only need covered and partial ones. Move an id back to `entries` to give it notes or a status other than `deferred`.

## Status (`sudsy`)
| Status | Meaning |
|---|---|
| `rule: <id>` | A Sudsy rule does it (or most of it, see `coverage`). `syntax-error` is the parser's diagnostic. |
| `option: <language>.format.<name>` | A formatter option does it. |
| `formatter` | Fixed formatter behavior, no option. |
| `sdk: <id>` | A .NET SDK diagnostic (IDE, CA, CS) does it. Sudsy never reimplements these; the rules that overlap one say so in `sudsy rules <id>`. |
| `config: <text>` | Configuration outside rules and formatter options (overrides, ignore files, presets, suppression comments). |
| `planned` | Accepted for implementation; `coverage` shows today's state. |
| `deferred` | Wanted eventually, deliberately not scheduled. |
| `wontfix: <reason>` | Out of scope. |

## Coverage
`covered` (at the upstream default, or via a non-default option, as the notes say), `partial` (the primary mapping exists, `gap` says what is missing), `missing`, `sdk`, `wontfix`, `deferred`.

## Counts
| Tool | Entries | Covered | Partial | Missing | SDK | Wontfix | Deferred | Planned |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| biome | 34 | 13 | 6 | 2 | 0 | 13 | 0 | 2 |
| csharpier | 16 | 11 | 5 | 0 | 0 | 0 | 0 | 0 |
| dotnet-format-editorconfig | 56 | 50 | 3 | 0 | 1 | 2 | 0 | 0 |
| eslint-plugin-jsonc | 44 | 36 | 6 | 0 | 0 | 2 | 0 | 0 |
| hadolint | 71 | 38 | 10 | 0 | 0 | 1 | 22 | 0 |
| markdownlint | 53 | 25 | 16 | 0 | 0 | 0 | 12 | 0 |
| prettier | 29 | 16 | 4 | 1 | 0 | 8 | 0 | 1 |
| psscriptanalyzer | 6 | 4 | 1 | 0 | 0 | 1 | 0 | 0 |
| roslynator | 67 | 23 | 7 | 16 | 18 | 1 | 2 | 16 |
| shellcheck | 520 | 17 | 8 | 0 | 0 | 0 | 495 | 0 |
| shfmt | 9 | 1 | 1 | 0 | 0 | 3 | 4 | 0 |
| stylecop | 197 | 120 | 10 | 1 | 45 | 21 | 0 | 1 |
| taplo | 23 | 11 | 7 | 0 | 0 | 0 | 5 | 0 |
| yamllint | 65 | 62 | 3 | 0 | 0 | 0 | 0 | 0 |

## Batches
Planned work is grouped so related entries land together.

| Batch | Scope |
|---|---|
| B1 yaml-defaults | yamllint defaults: comment spacing, `truthy` options, `line-length` for unbreakable words, a yamllint-compatible preset |
| B2 shared-format-options | `end_of_line`, `bracket_spacing`, JSON/YAML options several tools share |
| B3 yaml-optin-lint | yamllint opt-in rules (all done) |
| B4 json-optin-lint, sorted-keys-options | JSON opt-in rules and `sorted-keys` parity |
| B5 stylecop-lexical | comment and preprocessor spacing, empty statements, token checks |
| B6 stylecop-syntax | syntax-structure checks, one type per file, file name matches type |
| B7 stylecop-ordering | element and using order |
| B8 stylecop-layout-formatter | blank-line and closing-parenthesis layout as formatter options |
| B9 stylecop-semantic | rules that need the type layer |
| B10 stylecop-docs | documentation-comment rules (StyleCop's are done; Roslynator's code fixes that add the missing elements are deferred) |
| B11 csharpier-gaps | using sort, inactive `#if` branches, layout differences |
| B12 editorconfig-mapping | the `.editorconfig` keys not mapped yet |
| B13 existing-rule-options | options of rules that exist |
| B14 markdown-defaults | markdownlint rules on by default that Sudsy covers by rule or formatter |
| B15 markdown-deferred | markdownlint rules not built: configured structure, anchors and references, sibling checks |
| B16 dockerfile-hadolint | hadolint rules not built: label schema, stage-level analysis (apt lists, WORKDIR, pip cache), shell structure |

## Migration
`sudsy init --from <tool>` ([config/cli.md](../config/cli.md#init---from)) turns a tool's config into a `sudsy.yaml` with this catalog as the source of truth: the catalog files are embedded in the binary, the reason printed for a setting that cannot be migrated is the entry's `gap` (or its status), and the C# `.editorconfig` entries that switch off covered SA rules come from the `covered` StyleCop entries. The option and rule values are mapped in code (`crates/cli/src/init/`); each mapping names its catalog entry and debug builds fail when the entry points elsewhere. A new `rule:` or `option:` status therefore changes what `init` writes for the settings that use it; the fixtures under `tests/fixtures/init/` show the result.

## Equivalent ids of rules
`sudsy rules <id>` and the table in [rules/builtin.md](../rules/builtin.md) list as `Equivalent` every `covered` or `partial` entry whose status is `rule: <id>` in a catalog for one of the rule's languages (`languages::catalog::equivalents`). Rule descriptions do not name other tools; add or fix the entry here instead.

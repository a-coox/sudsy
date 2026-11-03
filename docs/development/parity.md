# Differential testing against upstream tools

`scripts/parity.sh <tool>` runs the upstream tool and `sudsy lint` on the fixtures in `tests/parity/<tool>/` and compares the diagnostics as `(rule, line, column)`. The upstream rule name is mapped to a Sudsy rule id through the catalog (`docs/parity/<tool>.yaml`: the top-level entries whose status is `rule: <id>`); an upstream rule with no mapping is a mismatch. Every mismatch is printed (`only <tool>`, `only sudsy`) and the exit code is 1 when there is any. Run it from Git Bash.

Tools: `yamllint`, `stylecop`. The script installs the catalog's `version` into a virtualenv under `target/parity/<tool>-<version>/` on first use (needs Python 3 and network) and builds the debug CLI.

## Fixtures
`tests/parity/<tool>/` holds one file per case plus the two configs that make the tools comparable: the upstream config (`.yamllint`) enables only rules Sudsy has an equivalent for, with the parameters Sudsy's rules take, and `sudsy.yaml` enables the matching Sudsy rules. A fixture is one file for every config, so the parameters must be the same for all cases. Fixtures where the tools legitimately differ are not included; those differences are recorded in the catalog entry's `gap`. Rules the formatter covers (yamllint `comments`) are off in `.yamllint`: they are tested as formatter output instead.

To add a case: add the file, enable the rule in both configs, run the script. To add a tool: add a function to `scripts/parity_compare.py` that returns the upstream diagnostics as `{(rule, line, column)}`, list the tool in `scripts/parity.sh`, and add its fixtures.

## stylecop
`scripts/parity.sh stylecop` copies `tests/parity/stylecop/` (a net10.0 project referencing StyleCop.Analyzers at the catalog's version, plus `.cs` fixtures) to `target/parity/stylecop-plain/` and `stylecop-fixed/`, enables every catalog SA id of status `rule:` or fully covered `formatter` as a warning through a generated `.editorconfig`, and runs `dotnet build` with an SARIF error log. Two comparisons:
- plain: each `rule:` SA id, mapped through the catalog, must be reported by `sudsy lint` at the same `(rule, line, column)`, and `sudsy lint` must report nothing else of the mapped rules;
- fixed: after `sudsy fix --unsafe .`, StyleCop must report no SA id the catalog marks `formatter` with coverage `covered`, and what it still reports for `rule:` ids must match `sudsy lint`.
`partial` formatter entries are skipped; their `gap` records what the harness found. Needs the .NET SDK and network for the NuGet restore. To add a case, add a `.cs` file and enable its Sudsy rules in `sudsy.yaml`; the catalog drives the id mapping.

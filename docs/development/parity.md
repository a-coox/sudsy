# Differential testing against upstream tools

`scripts/parity.sh <tool>` runs the upstream tool and `sudsy lint` on the fixtures in `tests/parity/<tool>/` and compares the diagnostics as `(rule, line, column)`. The upstream rule name is mapped to a Sudsy rule id through the catalog (`docs/parity/<tool>.yaml`: the top-level entries whose status is `rule: <id>`); an upstream rule with no mapping is a mismatch. Every mismatch is printed (`only <tool>`, `only sudsy`) and the exit code is 1 when there is any. Run it from Git Bash.

Tools: `yamllint`. The script installs the catalog's `version` into a virtualenv under `target/parity/<tool>-<version>/` on first use (needs Python 3 and network) and builds the debug CLI.

## Fixtures
`tests/parity/<tool>/` holds one file per case plus the two configs that make the tools comparable: the upstream config (`.yamllint`) enables only rules Sudsy has an equivalent for, with the parameters Sudsy's rules take, and `sudsy.yaml` enables the matching Sudsy rules. Fixtures where the tools legitimately differ (`key-ordering` reports a different key, `truthy` flags `True`) are not included; those differences are recorded in the catalog entry's `gap`.

To add a case: add the file, enable the rule in both configs, run the script. To add a tool: add a function to `scripts/parity_compare.py` that returns the upstream diagnostics as `{(rule, line, column)}`, list the tool in `scripts/parity.sh`, and add its fixtures.

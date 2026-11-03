#!/usr/bin/env bash
# Differential test against an upstream tool. Usage: scripts/parity.sh <tool>
# Runs the tool and `sudsy lint` on the fixtures in tests/parity/<tool>/ and compares rule (mapped
# through docs/parity/<tool>.yaml), line and column. Prints every mismatch; exit 1 on any.
# Tools: yamllint (installed at the catalog's version into a venv under target/parity/).
set -euo pipefail
cd "$(dirname "$0")/.."
tool=${1:-}
case "$tool" in
  yamllint) ;;
  *) echo "usage: scripts/parity.sh yamllint" >&2; exit 2 ;;
esac

py=
for candidate in python3 python; do
  if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
done
[ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }

version=$(sed -nE 's/^version: "([0-9][0-9.]*).*/\1/p' "docs/parity/$tool.yaml" | head -n1)
venv="target/parity/$tool-$version"
if [ ! -d "$venv" ]; then
  "$py" -m venv "$venv"
  vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"
  "$vpy" -m pip install --quiet "$tool==$version"
fi
vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"

cargo build --quiet -p cli
sudsy=$PWD/target/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe

exec "$vpy" scripts/parity_compare.py "$tool" "$version" "$sudsy"

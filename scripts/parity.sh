#!/usr/bin/env bash
# Differential test against an upstream tool. Usage: scripts/parity.sh [format] <tool>
# Lint mode runs the tool and `sudsy lint` on tests/parity/<tool>/ (or the rule fixtures) and compares
# rule (mapped through docs/parity/<tool>.yaml), line and column; format mode compares `sudsy format`
# with the tool's output. Prints every mismatch; exit 1 on any. The tools, their versions' catalogs
# and how they are installed are the entries of scripts/parity_tools.json (docs/development/parity.md).
set -euo pipefail
cd "$(dirname "$0")/.."
tdir=${CARGO_TARGET_DIR:-$PWD/target}
mode=lint
if [ "${1:-}" = format ]; then mode=format; shift; fi
tool=${1:-}
py=
for candidate in python3 python; do
  if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
done
[ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
"$py" -c 'import json,sys; sys.exit(sys.argv[2] not in json.load(open("scripts/parity_tools.json"))[sys.argv[1]])' "$mode" "$tool" ||
  { echo "usage: scripts/parity.sh [format] <$("$py" -c 'import json,sys; print("|".join(json.load(open("scripts/parity_tools.json"))[sys.argv[1]]))' "$mode")>" >&2; exit 2; }
cargo build --quiet -p cli
sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
# The comparison needs PyYAML (the lint catalogs, the differences.yaml of both modes).
venv="$tdir/parity/venv"
if [ ! -d "$venv" ]; then
  "$py" -m venv "$venv"
  vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"
  "$vpy" -m pip install --quiet pyyaml
fi
py="$venv/bin/python"; [ -x "$py" ] || py="$venv/Scripts/python.exe"
exec "$py" "scripts/parity_$mode.py" "$tool" "$sudsy" "$tdir"

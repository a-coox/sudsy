#!/usr/bin/env bash
# Differential test against an upstream tool. Usage: scripts/parity.sh <tool>
# Runs the tool and `sudsy lint` on the fixtures in tests/parity/<tool>/ and compares rule (mapped
# through docs/parity/<tool>.yaml), line and column. Prints every mismatch; exit 1 on any.
# Tools: yamllint (installed at the catalog's version into a venv under target/parity/), stylecop
# (StyleCop.Analyzers at the catalog's version, restored by `dotnet build` of the fixture project).
set -euo pipefail
cd "$(dirname "$0")/.."
tool=${1:-}
case "$tool" in
  yamllint|stylecop) ;;
  *) echo "usage: scripts/parity.sh yamllint|stylecop" >&2; exit 2 ;;
esac

py=
for candidate in python3 python; do
  if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
done
[ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }

version=$(sed -nE 's/^version: "([^ "]+).*/\1/p' "docs/parity/$tool.yaml" | head -n1)
# The comparison script needs PyYAML; yamllint is the upstream tool itself.
case "$tool" in
  yamllint) pkg="yamllint==$version" ;;
  *) pkg=pyyaml ;;
esac
venv="target/parity/$tool-$version"
if [ ! -d "$venv" ]; then
  "$py" -m venv "$venv"
  vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"
  "$vpy" -m pip install --quiet "$pkg"
fi
vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"

cargo build --quiet -p cli
if [ "$tool" = stylecop ]; then
  dotnet build --nologo -v:q workers/csharp/Sudsy.Worker
  export SUDSY_CSHARP_WORKER=${SUDSY_CSHARP_WORKER:-$PWD/workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll}
fi
sudsy=$PWD/target/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe

exec "$vpy" scripts/parity_compare.py "$tool" "$version" "$sudsy"

#!/usr/bin/env bash
# Differential test against an upstream tool. Usage: scripts/parity.sh <tool>
# Runs the tool and `sudsy lint` on the fixtures in tests/parity/<tool>/ and compares rule (mapped
# through docs/parity/<tool>.yaml), line and column. Prints every mismatch; exit 1 on any.
# Tools: yamllint (installed at the catalog's version into a venv under target/parity/), eslint-jsonc
# (ESLint 9 + eslint-plugin-jsonc at the catalog's version, installed with npm), stylecop
# (StyleCop.Analyzers at the catalog's version, restored by `dotnet build` of the fixture project).
# Format mode: scripts/parity.sh format prettier|biome|taplo|dotnet-format|csharpier (see docs/development/parity.md).
set -euo pipefail
cd "$(dirname "$0")/.."
tdir=${CARGO_TARGET_DIR:-$PWD/target}
tool=${1:-}
if [ "$tool" = format ]; then
  # Format mode: `sudsy format` under the matching preset against the tool's own output, on
  # tests/parity/<tool>/. The tool is installed with npm at the catalog's version
  # (dotnet-format is the SDK's `dotnet format whitespace`, nothing to install).
  tool=${2:-}
  case "$tool" in
    prettier|biome|taplo|dotnet-format|csharpier) ;;
    *) echo "usage: scripts/parity.sh format prettier|biome|taplo|dotnet-format|csharpier" >&2; exit 2 ;;
  esac
  if [ "$tool" = csharpier ]; then
    version=$(sed -nE 's/^version: "([0-9][0-9.]*).*/\1/p' docs/parity/csharpier.yaml | head -n1)
    dir="$tdir/parity/csharpier-$version"
    [ -x "$dir/csharpier" ] || [ -x "$dir/csharpier.exe" ] || dotnet tool install csharpier --version "$version" --tool-path "$dir"
    dotnet build --nologo -v:q workers/csharp/Sudsy.Worker
    export SUDSY_CSHARP_WORKER=${SUDSY_CSHARP_WORKER:-$PWD/workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll}
    cargo build --quiet -p cli
    sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
    py=
    for candidate in python3 python; do
      if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
    done
    [ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
    exec "$py" scripts/parity_format.py csharpier "$version" "$sudsy" "$dir"
  fi
  if [ "$tool" = taplo ]; then
    # taplo's release binary at the catalog's version; the toml-test valid files join the fixtures.
    version=$(sed -nE 's/^version: "([0-9][0-9.]*).*/\1/p' docs/parity/taplo.yaml | head -n1)
    dir="$tdir/parity/taplo-$version"
    if [ ! -x "$dir/taplo" ] && [ ! -x "$dir/taplo.exe" ]; then
      mkdir -p "$dir"
      case "$(uname -s)-$(uname -m)" in
        MINGW*|MSYS*|CYGWIN*) asset=taplo-windows-x86_64.zip ;;
        Linux-x86_64) asset=taplo-linux-x86_64.gz ;;
        Darwin-arm64) asset=taplo-darwin-aarch64.gz ;;
        Darwin-x86_64) asset=taplo-darwin-x86_64.gz ;;
        *) echo "parity.sh: no taplo release for this platform" >&2; exit 2 ;;
      esac
      curl -fsSL -o "$dir/$asset" "https://github.com/tamasfe/taplo/releases/download/$version/$asset"
      case "$asset" in
        *.zip) unzip -oq "$dir/$asset" -d "$dir" ;;
        *) gunzip -c "$dir/$asset" > "$dir/taplo"; chmod +x "$dir/taplo" ;;
      esac
    fi
    bash tests/conformance/fetch-toml-corpus.sh >/dev/null
    cargo build --quiet -p cli
    sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
    py=
    for candidate in python3 python; do
      if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
    done
    [ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
    exec "$py" scripts/parity_format.py taplo "$version" "$sudsy" "$dir"
  fi
  if [ "$tool" = dotnet-format ]; then
    dotnet build --nologo -v:q workers/csharp/Sudsy.Worker
    export SUDSY_CSHARP_WORKER=${SUDSY_CSHARP_WORKER:-$PWD/workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll}
    cargo build --quiet -p cli
    sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
    py=
    for candidate in python3 python; do
      if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
    done
    [ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
    exec "$py" scripts/parity_format.py dotnet-format "$(dotnet --version)" "$sudsy" "$tdir/parity"
  fi
  version=$(sed -nE 's/^version: "([0-9][0-9.]*).*/\1/p' "docs/parity/$tool.yaml" | head -n1)
  dir="$tdir/parity/$tool-$version"
  pkg=$tool; [ "$tool" = biome ] && pkg=@biomejs/biome
  if [ ! -d "$dir/node_modules/$pkg" ]; then
    mkdir -p "$dir"
    npm install --silent --no-audit --no-fund --prefix "$dir" "$pkg@$version"
  fi
  cargo build --quiet -p cli
  sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
  py=
  for candidate in python3 python; do
    if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
  done
  [ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
  exec "$py" scripts/parity_format.py "$tool" "$version" "$sudsy" "$dir"
fi
case "$tool" in
  yamllint|stylecop|eslint-jsonc) ;;
  *) echo "usage: scripts/parity.sh yamllint|stylecop|eslint-jsonc | format prettier|biome|taplo|dotnet-format|csharpier" >&2; exit 2 ;;
esac

py=
for candidate in python3 python; do
  if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
done
[ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }

version=$(sed -nE 's/^version: "([^ "]+).*/\1/p' "docs/parity/${tool/eslint-jsonc/eslint-plugin-jsonc}.yaml" | head -n1)
# The comparison script needs PyYAML; yamllint is the upstream tool itself.
case "$tool" in
  yamllint) pkg="yamllint==$version" ;;
  *) pkg=pyyaml ;;
esac
venv="$tdir/parity/$tool-$version"
# eslint-jsonc: the npm install lives in that folder; PyYAML for the comparison goes below it.
[ "$tool" = eslint-jsonc ] && venv="$venv/venv"
if [ ! -d "$venv" ]; then
  "$py" -m venv "$venv"
  vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"
  "$vpy" -m pip install --quiet "$pkg"
fi
vpy="$venv/bin/python"; [ -x "$vpy" ] || vpy="$venv/Scripts/python.exe"

cargo build --quiet -p cli
if [ "$tool" = eslint-jsonc ]; then
  # ESLint 9 with the plugin at the catalog's version (needs Node and network on first use).
  dir="$tdir/parity/eslint-jsonc-$version"
  if [ ! -d "$dir/node_modules/eslint-plugin-jsonc" ]; then
    mkdir -p "$dir"
    npm install --silent --no-audit --no-fund --prefix "$dir" eslint@9 "eslint-plugin-jsonc@$version"
  fi
fi
if [ "$tool" = stylecop ]; then
  dotnet build --nologo -v:q workers/csharp/Sudsy.Worker
  export SUDSY_CSHARP_WORKER=${SUDSY_CSHARP_WORKER:-$PWD/workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll}
fi
sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe

exec "$vpy" scripts/parity_compare.py "$tool" "$version" "$sudsy"

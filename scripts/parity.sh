#!/usr/bin/env bash
# Differential test against an upstream tool. Usage: scripts/parity.sh <tool>
# Runs the tool and `sudsy lint` on the fixtures in tests/parity/<tool>/ and compares rule (mapped
# through docs/parity/<tool>.yaml), line and column. Prints every mismatch; exit 1 on any.
# Tools: yamllint (installed at the catalog's version into a venv under target/parity/), eslint-jsonc
# (ESLint 9 + eslint-plugin-jsonc at the catalog's version, installed with npm), stylecop
# (StyleCop.Analyzers at the catalog's version, restored by `dotnet build` of the fixture project),
# hadolint and checkmake (the release binary at the catalog's version, downloaded from GitHub).
# Format mode: scripts/parity.sh format <tool>, the tools of scripts/parity_format.json (see docs/development/parity.md).
set -euo pipefail
cd "$(dirname "$0")/.."
tdir=${CARGO_TARGET_DIR:-$PWD/target}
tool=${1:-}
if [ "$tool" = format ]; then
  # Format mode: `sudsy format` under the matching preset against the tool's own output, on
  # tests/parity/<tool>/. The tools are the entries of scripts/parity_format.json; the runner
  # (parity_format.py) installs the tool at the catalog's version and compares.
  tool=${2:-}
  py=
  for candidate in python3 python; do
    if "$candidate" -c 'import sys' >/dev/null 2>&1; then py=$candidate; break; fi
  done
  [ -n "$py" ] || { echo "parity.sh: python 3 is required" >&2; exit 2; }
  "$py" -c 'import json,sys; sys.exit(sys.argv[1] not in json.load(open("scripts/parity_format.json")))' "$tool" ||
    { echo "usage: scripts/parity.sh format <$("$py" -c 'import json; print("|".join(json.load(open("scripts/parity_format.json"))))')>" >&2; exit 2; }
  cargo build --quiet -p cli
  sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe
  exec "$py" scripts/parity_format.py "$tool" "$sudsy" "$tdir"
fi
case "$tool" in
  yamllint|stylecop|eslint-jsonc|markdownlint|hadolint|shellcheck|psscriptanalyzer|buf|checkmake|actionlint) ;;
  *) echo "usage: scripts/parity.sh yamllint|stylecop|eslint-jsonc|markdownlint|hadolint|shellcheck|psscriptanalyzer|buf|checkmake|actionlint | format <tool>" >&2; exit 2 ;;
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
[ "$tool" = eslint-jsonc ] || [ "$tool" = markdownlint ] || [ "$tool" = psscriptanalyzer ] && venv="$venv/venv"
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
if [ "$tool" = markdownlint ]; then
  # markdownlint-cli2 at the catalog's version (needs Node and network on first use).
  dir="$tdir/parity/markdownlint-$version"
  if [ ! -d "$dir/node_modules/markdownlint-cli2" ]; then
    mkdir -p "$dir"
    npm install --silent --no-audit --no-fund --prefix "$dir" "markdownlint-cli2@$version"
  fi
fi
extra=
if [ "$tool" = hadolint ]; then
  # The release binary at the catalog's version (needs network on first use).
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) asset=hadolint-windows-x86_64.exe; exe=hadolint.exe ;;
    Darwin) asset=hadolint-macos-$(uname -m | sed 's/aarch64/arm64/'); exe=hadolint ;;
    *) asset=hadolint-linux-$(uname -m | sed 's/aarch64/arm64/'); exe=hadolint ;;
  esac
  dir="$tdir/parity/hadolint-$version"
  if [ ! -x "$dir/$exe" ]; then
    mkdir -p "$dir"
    curl -fsSL -o "$dir/$exe" "https://github.com/hadolint/hadolint/releases/download/v$version/$asset"
    chmod +x "$dir/$exe"
  fi
  extra="$dir/$exe"
fi
if [ "$tool" = actionlint ]; then
  # The release archive at the catalog's version (zip on Windows, tar.gz elsewhere; needs network on first use), and the corpus.
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) asset=actionlint_${version}_windows_amd64.zip; exe=actionlint.exe ;;
    Darwin) asset=actionlint_${version}_darwin_$(uname -m | sed 's/x86_64/amd64/').tar.gz; exe=actionlint ;;
    *) asset=actionlint_${version}_linux_$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/').tar.gz; exe=actionlint ;;
  esac
  dir="$tdir/parity/actionlint-$version"
  if [ ! -x "$dir/$exe" ]; then
    mkdir -p "$dir"
    curl -fsSL -o "$dir/$asset" "https://github.com/rhysd/actionlint/releases/download/v$version/$asset"
    "$py" -c 'import shutil,sys; shutil.unpack_archive(sys.argv[1], sys.argv[2])' "$dir/$asset" "$dir"
    chmod +x "$dir/$exe"
  fi
  extra="$dir/$exe"
  bash tests/conformance/fetch-github-actions-corpus.sh
fi
if [ "$tool" = checkmake ]; then
  # The release executable at the catalog's version (needs network on first use).
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) asset=checkmake-v$version.windows.amd64.exe; exe=checkmake.exe ;;
    Darwin) asset=checkmake-v$version.darwin.$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/'); exe=checkmake ;;
    *) asset=checkmake-v$version.linux.$(uname -m | sed 's/x86_64/amd64/;s/aarch64/arm64/'); exe=checkmake ;;
  esac
  dir="$tdir/parity/checkmake-$version"
  if [ ! -x "$dir/$exe" ]; then
    mkdir -p "$dir"
    curl -fsSL -o "$dir/$exe" "https://github.com/checkmake/checkmake/releases/download/v$version/$asset"
    chmod +x "$dir/$exe"
  fi
  extra="$dir/$exe"
fi
if [ "$tool" = shellcheck ]; then
  # The release archive at the catalog's version (zip on Windows, tar.xz elsewhere; needs network on first use).
  case "$(uname -s)" in
    MINGW*|MSYS*|CYGWIN*) asset=shellcheck-v$version.zip; exe=shellcheck.exe ;;
    Darwin) asset=shellcheck-v$version.darwin.$(uname -m).tar.xz; exe=shellcheck ;;
    *) asset=shellcheck-v$version.linux.$(uname -m).tar.xz; exe=shellcheck ;;
  esac
  dir="$tdir/parity/shellcheck-$version"
  if [ ! -x "$dir/$exe" ]; then
    mkdir -p "$dir/unpack"
    curl -fsSL -o "$dir/$asset" "https://github.com/koalaman/shellcheck/releases/download/v$version/$asset"
    "$py" -c 'import shutil,sys; shutil.unpack_archive(sys.argv[1], sys.argv[2])' "$dir/$asset" "$dir/unpack"
    mv "$(find "$dir/unpack" -name "$exe" | head -n1)" "$dir/$exe"
    chmod +x "$dir/$exe"
  fi
  extra="$dir/$exe"
fi
if [ "$tool" = stylecop ]; then
  dotnet build --nologo -v:q workers/csharp/Sudsy.Worker
  export SUDSY_CSHARP_WORKER=${SUDSY_CSHARP_WORKER:-$PWD/workers/csharp/Sudsy.Worker/bin/Debug/net10.0/Sudsy.Worker.dll}
fi
sudsy=$tdir/debug/sudsy; [ -x "$sudsy" ] || sudsy=$sudsy.exe

exec "$vpy" scripts/parity_compare.py "$tool" "$version" "$sudsy" $extra

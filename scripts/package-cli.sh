#!/usr/bin/env bash
# Makes the CLI archive of one platform from a release build and the published worker:
# sudsy-<version>-<platform>.tar.gz (.zip on Windows), holding the executable, csharp-worker/
# beside it (where `sudsy` looks for it), LICENSE and CHANGELOG.md.
# With a packs dir (scripts/pack.sh output) it makes the offline sudsy-full-<version>-<platform>
# instead, with the packs in packs/ (`sudsy languages add --all --from packs`).
# Usage: scripts/package-cli.sh <platform> <path of sudsy[.exe]> <published worker dir> <output dir> [packs dir]
set -euo pipefail
cd "$(dirname "$0")/.."
[ $# -ge 4 ] || { echo "usage: package-cli.sh <platform> <sudsy executable> <worker dir> <output dir> [packs dir]" >&2; exit 2; }
platform=$1; exe=$2; worker=$3; out=$4; packs=${5:-}
version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
name="sudsy-$version-$platform"
[ -z "$packs" ] || name="sudsy-full-$version-$platform"
stage=$(mktemp -d)/"$name"
mkdir -p "$stage" "$out"
cp "$exe" "$stage/"
cp -r "$worker" "$stage/csharp-worker"
cp LICENSE CHANGELOG.md "$stage/"
if [ -n "$packs" ]; then mkdir "$stage/packs"; cp "$packs"/sudsy-* "$stage/packs/"; fi
out=$(cd "$out" && pwd)
case "$platform" in
  win32-*) (cd "$(dirname "$stage")" && 7z a -tzip -bso0 "$out/$name.zip" "$name") ;;
  *) tar -C "$(dirname "$stage")" -czf "$out/$name.tar.gz" "$name" ;;
esac
ls -l "$out"

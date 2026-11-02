#!/usr/bin/env bash
# Formats a copy of the C# corpus with the pinned CSharpier (dotnet-tools.json) for the
# differential test (crates/lang-csharp/tests/differential.rs) and for the speed measurement.
#   $SUDSY_DIFF_DIR/in    the corpus files, as fetched (same relative paths as in corpus/)
#   $SUDSY_DIFF_DIR/out   CSharpier's output
# The directory must be outside the repository: CSharpier honors .gitignore, and target/ is ignored.
set -euo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
corpus="$repo/tests/conformance/corpus"
dir="${SUDSY_DIFF_DIR:-${TMPDIR:-${TEMP:-/tmp}}/sudsy-differential}"

if [ ! -d "$dir/in" ]; then
  mkdir -p "$dir/in"
  for name in runtime roslyn aspnetcore; do
    (cd "$corpus" && find "$name" -name '*.cs' -not -path '*/.git/*' -print0 | xargs -0 cp --parents -t "$dir/in")
  done
fi
rm -rf "$dir/out"
cp -r "$dir/in" "$dir/out"
cd "$repo"
dotnet tool restore >/dev/null
start=$(date +%s)
dotnet csharpier format "$dir/out" --no-cache
echo "csharpier format: $(( $(date +%s) - start )) s; output in $dir/out"

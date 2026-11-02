#!/usr/bin/env bash
# Wall-clock comparison of `sudsy format --check` (csharp/csharpier-compat) and `csharpier check`
# on the corpus copy made by csharpier-baseline.sh. Usage: speed.sh [subdir of $SUDSY_DIFF_DIR/in]
# [runs]. The first run of each tool is the cold one (nothing of it in memory: .NET JIT, file
# cache); the later ones are warm.
set -uo pipefail

repo="$(cd "$(dirname "$0")/../.." && pwd)"
dir="${SUDSY_DIFF_DIR:-${TMPDIR:-${TEMP:-/tmp}}/sudsy-differential}"
target="$dir/in/${1:-}"
runs="${2:-3}"
sudsy="$repo/target/release/sudsy"
export SUDSY_CSHARP_WORKER="${SUDSY_CSHARP_WORKER:-$repo/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll}"

printf 'extends:\n  - preset:csharp/csharpier-compat\n' > "$dir/sudsy.yaml"

now() { date +%s.%N; }
for i in $(seq "$runs"); do
  s=$(now); (cd "$dir" && "$sudsy" format --check "$target" > /dev/null 2>&1); e=$(now)
  printf 'sudsy     run %s: %.1f s\n' "$i" "$(awk "BEGIN{print $e - $s}")"
done
cd "$repo"
for i in $(seq "$runs"); do
  s=$(now); dotnet csharpier check "$target" --no-cache > /dev/null 2>&1; e=$(now)
  printf 'csharpier run %s: %.1f s\n' "$i" "$(awk "BEGIN{print $e - $s}")"
done

#!/usr/bin/env bash
# Wall-clock comparison of JSON/YAML formatters in check mode: `sudsy format --check`,
# `prettier --check`, `biome format` (JSON only, its YAML formatter is not stable) and `dprint check`
# (incremental cache off). Every run is a new process; prints best (min - max) of $runs.
# Biome gets a raised files.maxSize (default 1 MiB skips the large file) and no diagnostics (printing
# diffs of an 11 MB file takes minutes). Inputs: the many small files of the JSON/YAML conformance corpora (run fetch-json-corpus.sh and
# fetch-yaml-corpus.sh first) and the large generated files of tools/perf-bench (--write).
# Needs: cargo build --release, node/npm. Usage: speed-json-yaml.sh [runs]
set -uo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
repo="$here/../.."
runs="${1:-3}"
sudsy="$repo/target/release/sudsy"
work="${SUDSY_SPEED_DIR:-${TMPDIR:-${TEMP:-/tmp}}/sudsy-speed-json-yaml}"
tools="$here/speed-tools"
bin="$tools/node_modules/.bin"

(cd "$tools" && npm install --silent --no-audit --no-fund) || exit 1
cargo build --release -q -p cli -p perf-bench --manifest-path "$repo/Cargo.toml" || exit 1

rm -rf "$work"; mkdir -p "$work/json-small" "$work/yaml-small" "$work/large-json" "$work/large-yaml"
cp "$here"/corpus/JSONTestSuite/test_parsing/y_*.json "$work/json-small/"
# valid yaml-test-suite cases only (no `error` file); flatten to one file per case
find "$here/corpus/yaml-test-suite" -name in.yaml -not -path '*/.git/*' | while read -r f; do
  d="$(dirname "$f")"; [ -e "$d/error" ] && continue
  cp "$f" "$work/yaml-small/$(echo "${d#"$here"/corpus/yaml-test-suite/}" | tr / _).yaml"
done
"$repo/target/release/perf-bench" --write "$work/large"
mv "$work/large/bench.json" "$work/large-json/"; mv "$work/large/bench.yaml" "$work/large-yaml/"; rmdir "$work/large"

now() { date +%s.%N; }
# bench LABEL DIR CMD... : run in $DIR (no sudsy.yaml there), print best (min - max) seconds
bench() {
  local label="$1" dir="$2"; shift 2
  local ts=()
  for _ in $(seq "$runs"); do
    s=$(now); (cd "$dir" && "$@" > /dev/null 2>&1); e=$(now)
    ts+=("$(awk "BEGIN{print $e - $s}")")
  done
  printf '%-10s %-12s %s\n' "$label" "$(basename "$dir")" "$(printf '%s\n' "${ts[@]}" | sort -n | awk '{a[NR]=$1} END{printf "%.2f s (%.2f - %.2f)", a[1], a[1], a[NR]}')"
}

for set in json-small large-json yaml-small large-yaml; do
  d="$work/$set"
  printf '%s: %s files, %s bytes\n' "$set" "$(ls "$d" | wc -l)" "$(cat "$d"/* | wc -c)"
  bench sudsy "$d" "$sudsy" format --check .
  bench prettier "$d" "$bin/prettier" --check --no-cache --log-level warn .
  case "$set" in *json*) bench biome "$d" "$bin/biome" format --colors=off --files-max-size=100000000 --max-diagnostics=0 . ;; esac
  bench dprint "$d" "$bin/dprint" check --config "$tools/dprint.json" "$d"
done

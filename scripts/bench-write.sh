#!/usr/bin/env bash
# Wall-clock of `sudsy format` rewriting many small files, to compare two binaries (e.g. before
# and after a change to how files are written): scripts/bench-write.sh OLD.exe NEW.exe [files]
set -euo pipefail
n=${3:-1500}
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
for exe in "$1" "$2" "$1" "$2"; do
  rm -rf "$dir"/*
  for i in $(seq 1 "$n"); do printf '{"a":1,   "b":[1,2,3],"c":{"d":"%s"}}\n' "$i" > "$dir/f$i.json"; done
  start=$(date +%s%N)
  "$exe" format "$dir" 2> /dev/null || true
  echo "$exe: $(( ($(date +%s%N) - start) / 1000000 )) ms for $n files"
done

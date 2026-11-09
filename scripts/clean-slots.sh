#!/usr/bin/env bash
# Frees disk in the repo's target/ and the build slots (docs/development/dev-loop.md), or only the
# slot directories given. Deletes what only ever grows: cargo's incremental-compilation caches and
# the older copies of each test binary and its debug info in */deps (every worktree path gets new
# file hashes, so each worktree that used a slot left a full set of .exe/.pdb behind). Builds stay
# correct; the next one is slower. Usage: scripts/clean-slots.sh [--all] [slot dir...]
#   --all: whole debug/ dirs
set -euo pipefail
cd "$(dirname "$0")/.."
all=0; [ "${1:-}" = --all ] && { all=1; shift; }
slots=("$@")
if [ ${#slots[@]} -eq 0 ]; then
  slots=(target)
  for s in "${SUDSY_SLOTS_DIR:-C:/Coding/.sudsy-targets}"/*/; do [ -d "$s" ] && slots+=("${s%/}"); done
fi
for t in "${slots[@]}"; do
  [ -d "$t" ] || continue
  # Nested target dirs too: the sample plugin's (sample-host/) and bench's (bench/<side>/).
  if [ $all = 1 ]; then dirs=$(find "$t" -maxdepth 3 -type d -name debug); else dirs=$(find "$t" -maxdepth 4 -type d -name incremental); fi
  for d in $dirs; do
    echo "removing $d"
    rm -rf "$d"
  done
  for d in $(find "$t" -maxdepth 4 -type d -name deps); do
    # Newest first; a name seen before, without its hash, is an older copy.
    old=$(find "$d" -maxdepth 1 \( -name '*.exe' -o -name '*.pdb' \) -printf '%T@ %f\n' | sort -rn \
      | awk '{ k = $2; sub(/-[0-9a-f]{16}\./, ".", k); if (seen[k]++) print $2 }')
    [ -n "$old" ] || continue
    echo "removing $(wc -l <<<"$old") older test binaries and debug info in $d"
    (cd "$d" && xargs rm -f <<<"$old") || echo "  some are in use; left in place" >&2
  done
done
df -h "${slots[0]}" | tail -1

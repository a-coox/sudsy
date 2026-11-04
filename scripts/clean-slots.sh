#!/usr/bin/env bash
# Frees disk by deleting cargo's incremental-compilation caches, which only ever grow, in the
# repo's target/ and in every build slot (docs/development/dev-loop.md). Builds stay correct; the
# next one per slot is slower. Usage: scripts/clean-slots.sh [--all]  (--all: whole debug/ dirs)
set -euo pipefail
cd "$(dirname "$0")/.."
slots=(target)
for s in "${SUDSY_SLOTS_DIR:-C:/Coding/.sudsy-targets}"/*/; do [ -d "$s" ] && slots+=("${s%/}"); done
for t in "${slots[@]}"; do
  if [ "${1:-}" = --all ]; then dirs=("$t/debug"); else dirs=("$t"/*/incremental); fi
  for d in "${dirs[@]}"; do
    [ -d "$d" ] || continue
    echo "removing $d"
    rm -rf "$d"
  done
done
df -h "${slots[0]}" | tail -1

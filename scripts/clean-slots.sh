#!/usr/bin/env bash
# Frees disk by hand in the repo's target/ and the build slots (docs/development/dev-loop.md), or
# only the slot directories given, and in their WSL bench caches. gate.sh and finish.sh keep each
# slot under its cap on their own (sweep_slot in scripts/slot-run.sh); this is for a full disk.
#   scripts/clean-slots.sh [slot dir...]: every incremental cache (next build slower)
#   scripts/clean-slots.sh --all [slot dir...]: whole debug/ dirs
set -euo pipefail
cd "$(dirname "$0")/.."
mode=default; case "${1:-}" in --all) mode=all; shift ;; esac
slots=("$@")
if [ ${#slots[@]} -eq 0 ]; then
  slots=(target)
  for s in "${SUDSY_SLOTS_DIR:-C:/Coding/.sudsy-targets}"/*/; do [ -d "$s" ] && slots+=("${s%/}"); done
fi

for t in "${slots[@]}"; do
  [ -d "$t" ] || continue
  # Nested target dirs too: the sample plugin's (sample-host/) and bench's (bench/<side>/).
  name=$([ $mode = all ] && echo debug || echo incremental)
  for d in $(find "$t" -maxdepth 4 -type d -name "$name"); do echo "removing $d"; rm -rf "$d"; done
done
df -h "${slots[0]}" | tail -1

# bench.sh builds each slot's instruction-count side in WSL (~/.cache/sudsy-bench/<slot name>).
case "$(uname -s)" in MINGW*|MSYS*)
  if [ $mode != all ] && command -v wsl >/dev/null; then
    keys=; for t in "${slots[@]}"; do keys+=" \$HOME/.cache/sudsy-bench/$(basename "$t")"; done
    p=$(cygpath -m "$PWD"); drive=${p:0:1}
    MSYS_NO_PATHCONV=1 wsl -d "${BENCH_WSL_DISTRO:-Ubuntu}" -e bash -c "cd /mnt/${drive,,}${p:2} && bash scripts/clean-slots.sh$keys" || true
  fi ;;
esac

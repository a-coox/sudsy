#!/usr/bin/env bash
# Frees disk in the repo's target/ and the build slots (docs/development/dev-loop.md), or only the
# slot directories given, and in their WSL bench caches.
#   scripts/clean-slots.sh --stale [slot dir...]: only what no build can use again (the gate runs
#     this for its slot): the workspace outputs and incremental caches of earlier worktrees (every
#     worktree path gets new file hashes, so each one leaves its whole build behind). Builds are
#     not slowed. Run from a worktree; the main checkout prunes nothing this way.
#   scripts/clean-slots.sh [slot dir...]: the above and every incremental cache (next build slower)
#   scripts/clean-slots.sh --all [slot dir...]: whole debug/ dirs
set -euo pipefail
cd "$(dirname "$0")/.."
mode=default; case "${1:-}" in --stale|--all) mode=${1#--}; shift ;; esac
slots=("$@")
if [ ${#slots[@]} -eq 0 ]; then
  slots=(target)
  for s in "${SUDSY_SLOTS_DIR:-C:/Coding/.sudsy-targets}"/*/; do [ -d "$s" ] && slots+=("${s%/}"); done
fi

# A slot serves one worktree at a time, and every checkout path gets its own file hashes, so the
# worktree's first build makes new outputs for all workspace crates. Workspace outputs (a relative
# first source in their .d file; registry crates have absolute ones and are shared) and
# incremental caches older than this worktree's .git file are an earlier worktree's: never used
# again. Only from a worktree (the main checkout uses target/), and not bench's base side, which
# only ever builds bench/base-src.
born=$PWD/.git; [ -f "$born" ] || born=
prune_deps() { # DIR
  local stems
  stems=$(find "$1" -maxdepth 1 -name '*.d' ! -newer "$born" -print0 | xargs -0r awk '
    FNR == 1 { if ($2 !~ /^([A-Za-z]:|\/)/) { sub(/\.d$/, "", FILENAME); print FILENAME }; nextfile }')
  [ -n "$stems" ] || return 0
  # Globs expand in this shell and one rm takes them all: a process per output takes minutes on Windows.
  local files=() s
  while IFS= read -r s; do files+=("$s" "$s".* "${s%/*}/lib${s##*/}".*); done <<<"$stems"
  printf '%s\0' "${files[@]}" | xargs -0 rm -f
  echo "removed $(wc -l <<<"$stems") outputs of earlier worktrees in $1"
}

for t in "${slots[@]}"; do
  [ -d "$t" ] || continue
  # Nested target dirs too: the sample plugin's (sample-host/) and bench's (bench/<side>/).
  if [ $mode = all ]; then
    for d in $(find "$t" -maxdepth 3 -type d -name debug); do echo "removing $d"; rm -rf "$d"; done
    continue
  fi
  if [ $mode = default ]; then
    for d in $(find "$t" -maxdepth 4 -type d -name incremental); do echo "removing $d"; rm -rf "$d"; done
  fi
  [ -n "$born" ] || continue
  for d in $(find "$t" -maxdepth 4 -type d \( -name incremental -o -name deps \) ! -path '*/bench/base*' ! -path '*/sudsy-bench/*/base*'); do
    if [ "${d##*/}" = deps ]; then prune_deps "$d"
    else find "$d" -mindepth 1 -maxdepth 1 -type d ! -newer "$born" -exec rm -rf {} +; fi
  done
done
[ $mode = stale ] || df -h "${slots[0]}" | tail -1

# bench.sh builds each slot's instruction-count side in WSL (~/.cache/sudsy-bench/<slot name>),
# whose .d files name the /mnt/c sources: the same cleanup there.
case "$(uname -s)" in MINGW*|MSYS*)
  if [ $mode != all ] && command -v wsl >/dev/null; then
    keys=; for t in "${slots[@]}"; do keys+=" \$HOME/.cache/sudsy-bench/$(basename "$t")"; done
    p=$(cygpath -m "$PWD"); drive=${p:0:1}; flag=; [ $mode = stale ] && flag=--stale
    MSYS_NO_PATHCONV=1 wsl -d "${BENCH_WSL_DISTRO:-Ubuntu}" -e bash -c "cd /mnt/${drive,,}${p:2} && bash scripts/clean-slots.sh $flag$keys" || true
  fi ;;
esac

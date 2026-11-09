#!/usr/bin/env bash
# Times the end-of-change commands for the dev loop rows of docs/performance/numbers.md after an
# edit of each FILE (default: one language, crates/lang-toml/src/lib.rs; a core crate such as
# crates/core-syntax/src/lib.rs affects everything and takes far longer): gate --affected, finish
# without cached baselines, finish with them. The edit is a comment line appended to the file
# (selection reads the diff) and is reverted after.
# Run in an idle slot of a clean worktree: CARGO_TARGET_DIR=<slot> scripts/devloop-times.sh [FILE...]
set -uo pipefail
cd "$(dirname "$0")/.."
run() { local t=$SECONDS; "$@" >/dev/null 2>&1; echo "$*: exit $? in $((SECONDS - t))s"; }
[ $# -gt 0 ] || set -- crates/lang-toml/src/lib.rs
for f in "$@"; do
  echo "== edit of $f"
  echo "// devloop-times" >>"$f"
  trap 'git checkout -- "$f"' EXIT
  # Against HEAD, so that unpushed commits do not count as part of the edit.
  run scripts/gate.sh --affected --base HEAD
  export BENCH_CACHE=${CARGO_TARGET_DIR:-target}/bench/devloop-cache; rm -rf "$BENCH_CACHE"
  run scripts/finish.sh --base HEAD --no-record
  run scripts/finish.sh --base HEAD --no-record
  unset BENCH_CACHE
  git checkout -- "$f"
done

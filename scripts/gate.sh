#!/usr/bin/env bash
# Pre-commit gate. Usage: scripts/gate.sh [--rust-only] [--affected [--base REV]] [-p <crate>]...
# Default: everything. --affected: what `cargo xtask affected` selects for the change against REV
# (default the merge-base with origin/main): its crates (clippy, tests, doctests), the codegen check,
# the sample plugin, the changed workers (workers/<name>/check.sh) and the extension. -p (repeatable)
# limits clippy and tests to those crates and skips the rest. fmt always checks the whole workspace.
# Each step is killed after GATE_TIMEOUT seconds (default 1800) and the gate fails.
set -uo pipefail
cd "$(dirname "$0")/.."
rust_only=0 affected=0 pkg=() base=()
while [ $# -gt 0 ]; do
  case "$1" in
    --rust-only) rust_only=1 ;;
    --affected) affected=1 ;;
    --base) base=(--base "$2"); shift ;;
    -p) pkg+=(-p "$2"); shift ;;
    *) echo "usage: gate.sh [--rust-only] [--affected [--base REV]] [-p <crate>]..." >&2; exit 2 ;;
  esac; shift
done
gate() {
  local name=$1 t=$SECONDS; shift; echo "== $name"
  timeout -k 15 "${GATE_TIMEOUT:-1800}" "$@"; local rc=$?
  [ $rc -eq 0 ] || { [ $rc -eq 124 ] && echo "GATE TIMED OUT after ${GATE_TIMEOUT:-1800}s (GATE_TIMEOUT): $name" >&2; echo "GATE FAILED: $name" >&2; exit 1; }
  echo "   ($name: $((SECONDS - t))s)"
}

. scripts/slot-run.sh; claim_slot

# The first gate of a worktree in a slot deletes the earlier worktrees' outputs there, which are
# never used again (clean-slots.sh --stale); the slot remembers which worktree it last pruned for.
# It touches no output this build uses, so it runs alongside the steps.
cleanup=
if [ -n "${CARGO_TARGET_DIR:-}" ] && [ "$(cat "$CARGO_TARGET_DIR/.pruned-for" 2>/dev/null)" != "$PWD" ]; then
  slot=$CARGO_TARGET_DIR
  { t=$SECONDS; scripts/clean-slots.sh --stale "$slot" >"$slot/cleanup.log" 2>&1 && echo "$PWD" >"$slot/.pruned-for"
    echo "   (slot cleanup, alongside: $((SECONDS - t))s)"; } &
  cleanup=$!
fi

# What runs: crates for clippy and tests (ws) and the other steps.
if [ ${#pkg[@]} -gt 0 ]; then
  ws=("${pkg[@]}") steps=
elif [ $affected = 1 ]; then
  sel=$(cargo xtask affected "${base[@]}") || { echo "GATE FAILED: cargo xtask affected" >&2; exit 1; }
  grep -v '^bench ' <<<"$sel"
  crates=$(sed -n 's/^crates //p' <<<"$sel") steps=$(sed -n 's/^steps //p' <<<"$sel")
  ws=(); if [ "$crates" = all ]; then ws=(--workspace); else for c in $crates; do ws+=(-p "$c"); done; fi
else
  ws=(--workspace) steps="codegen sample-plugin vscode"
  for w in workers/*/check.sh; do steps="$steps worker:$(basename "$(dirname "$w")")"; done
fi
has() { case " $steps " in *" $1 "*) return 0 ;; esac; return 1; }

gate "cargo fmt" cargo fmt --all -- --check
if [ ${#ws[@]} -gt 0 ]; then
  gate "cargo clippy" cargo clippy "${ws[@]}" --all-targets -- -D warnings
  # nextest runs the tests of all binaries in parallel (cargo test runs one binary after another);
  # it skips doctests, which cargo test --doc runs.
  command -v cargo-nextest >/dev/null || { echo "GATE FAILED: cargo-nextest missing (cargo install cargo-nextest --locked)" >&2; exit 1; }
  gate "cargo nextest" cargo nextest run "${ws[@]}" --no-fail-fast --no-tests=pass
  gate "cargo test --doc" cargo test "${ws[@]}" --doc
fi
has codegen && gate "cargo xtask codegen --check" cargo xtask codegen --check
if has sample-plugin; then
  # The sample plugin is a workspace of its own: format check and its native rule tests. With a
  # shared CARGO_TARGET_DIR it gets its own subdirectory (two workspaces in one deps dir confuse rustc).
  [ -n "${CARGO_TARGET_DIR:-}" ] && export CARGO_TARGET_DIR=$CARGO_TARGET_DIR/sample-host
  gate "sample plugin fmt" cargo fmt --manifest-path tests/plugins/sample/Cargo.toml -- --check
  gate "sample plugin tests" cargo test --manifest-path tests/plugins/sample/Cargo.toml
fi
if [ $rust_only -eq 0 ]; then
  for s in $steps; do
    case "$s" in worker:*) gate "worker ${s#worker:}" bash "workers/${s#worker:}/check.sh" ;; esac
  done
  if has vscode; then
    [ -d editors/vscode/node_modules ] || gate "npm ci" npm --prefix editors/vscode ci
    gate "vscode typecheck" npm --prefix editors/vscode run typecheck
    gate "vscode lint" npm --prefix editors/vscode run lint
    gate "vscode test" npm --prefix editors/vscode run test
    gate "vscode build" npm --prefix editors/vscode run build
  fi
fi
[ -z "$cleanup" ] || wait $cleanup
echo "ALL GATES PASSED (${SECONDS}s)"
# Build caches only grow; warn before the disk fills (it has, mid-build).
free_gb=$(df -BG --output=avail "${CARGO_TARGET_DIR:-target}" 2>/dev/null | tail -1 | tr -dc 0-9)
[ -n "$free_gb" ] && [ "$free_gb" -lt 30 ] && echo "warning: ${free_gb} GB free on the build disk; run scripts/clean-slots.sh" >&2
exit 0

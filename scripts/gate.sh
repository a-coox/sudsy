#!/usr/bin/env bash
# Pre-commit gate. Usage: scripts/gate.sh [--rust-only] [-p <crate>]
# -p limits fmt/clippy/test to one crate and skips dotnet, codegen and the extension.
# Each step is killed after GATE_TIMEOUT seconds (default 1800) and the gate fails.
set -uo pipefail
cd "$(dirname "$0")/.."
rust_only=0; pkg=()
while [ $# -gt 0 ]; do
  case "$1" in
    --rust-only) rust_only=1 ;;
    -p) pkg=(-p "$2"); shift ;;
    *) echo "usage: gate.sh [--rust-only] [-p <crate>]" >&2; exit 2 ;;
  esac; shift
done
ws=(--workspace); [ ${#pkg[@]} -gt 0 ] && ws=("${pkg[@]}")
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

gate "cargo fmt" cargo fmt --all -- --check
gate "cargo clippy" cargo clippy "${ws[@]}" --all-targets -- -D warnings
gate "cargo test" cargo test "${ws[@]}"
if [ ${#pkg[@]} -eq 0 ]; then
  gate "cargo xtask codegen --check" cargo xtask codegen --check
  # The sample plugin is a workspace of its own: format check and its native rule tests. With a
  # shared CARGO_TARGET_DIR it gets its own subdirectory (two workspaces in one deps dir confuse rustc).
  [ -n "${CARGO_TARGET_DIR:-}" ] && export CARGO_TARGET_DIR=$CARGO_TARGET_DIR/sample-host
  gate "sample plugin fmt" cargo fmt --manifest-path tests/plugins/sample/Cargo.toml -- --check
  gate "sample plugin tests" cargo test --manifest-path tests/plugins/sample/Cargo.toml
  if [ $rust_only -eq 0 ]; then
    gate "dotnet build" dotnet build workers/csharp
    gate "dotnet test" dotnet test workers/csharp
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

#!/usr/bin/env bash
# Pre-commit gate. Usage: scripts/gate.sh [--rust-only] [-p <crate>]
# -p limits fmt/clippy/test to one crate and skips dotnet, codegen and the extension.
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
  "$@" || { echo "GATE FAILED: $name" >&2; exit 1; }
  echo "   ($name: $((SECONDS - t))s)"
}
vscode() { (cd editors/vscode && npm run "$@"); }

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
    [ -d editors/vscode/node_modules ] || gate "npm ci" bash -c 'cd editors/vscode && npm ci'
    gate "vscode typecheck" vscode typecheck
    gate "vscode lint" vscode lint
    gate "vscode test" vscode test
    gate "vscode build" vscode build
  fi
fi
echo "ALL GATES PASSED (${SECONDS}s)"

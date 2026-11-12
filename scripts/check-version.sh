#!/usr/bin/env bash
# Release gate: the Cargo workspace, the VS Code extension and (when given) the tag agree on the
# version, and CHANGELOG.md has a section for it. Usage: scripts/check-version.sh [vX.Y.Z]
set -euo pipefail
cd "$(dirname "$0")/.."
cargo_version=$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')
ext_version=$(node -p "require('./editors/vscode/package.json').version")
lock_version=$(node -p "require('./editors/vscode/package-lock.json').version")
fail() { echo "version check failed: $*" >&2; exit 1; }
[ "$cargo_version" = "$ext_version" ] || fail "Cargo.toml is $cargo_version but package.json is $ext_version"
[ "$cargo_version" = "$lock_version" ] || fail "Cargo.toml is $cargo_version but package-lock.json is $lock_version"
if [ $# -gt 0 ]; then
  [ "v$cargo_version" = "$1" ] || fail "the tag is $1 but the version is $cargo_version"
fi
grep -q "^## \[\?$cargo_version\]\?" CHANGELOG.md || fail "CHANGELOG.md has no section for $cargo_version"
echo "version $cargo_version"

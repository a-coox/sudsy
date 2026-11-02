#!/usr/bin/env bash
# Fetches the pinned YAML conformance corpus: the `data` branch of yaml/yaml-test-suite (one
# directory per test case) into tests/conformance/corpus/yaml-test-suite (gitignored).
set -euo pipefail

# yaml/yaml-test-suite branch `data` as of 2026-11-01
SUITE_SHA=6ad3d2c62885d82fc349026c136ef560838fdf3d

dest="$(cd "$(dirname "$0")" && pwd)/corpus/yaml-test-suite"
if [ -d "$dest/.git" ] && [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" = "$SUITE_SHA" ]; then
  echo "corpus already at $SUITE_SHA"
  exit 0
fi

rm -rf "$dest"
mkdir -p "$dest"
git -C "$dest" init -q
git -C "$dest" config core.autocrlf false
git -C "$dest" remote add origin https://github.com/yaml/yaml-test-suite.git
git -C "$dest" fetch -q --depth 1 origin "$SUITE_SHA"
git -C "$dest" checkout -q FETCH_HEAD
echo "corpus ready: $dest"

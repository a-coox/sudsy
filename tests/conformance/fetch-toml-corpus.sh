#!/usr/bin/env bash
# Fetches the pinned TOML conformance corpus: toml-lang/toml-test's tests/ directory into
# tests/conformance/corpus/toml-test (gitignored).
set -euo pipefail

# toml-lang/toml-test v1.6.0
SUITE_SHA=65a4da1357c5b38235982f1320bc8173929d8d24

dest="$(cd "$(dirname "$0")" && pwd)/corpus/toml-test"
if [ -d "$dest/.git" ] && [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" = "$SUITE_SHA" ]; then
  echo "corpus already at $SUITE_SHA"
  exit 0
fi

rm -rf "$dest"
mkdir -p "$dest"
git -C "$dest" init -q
git -C "$dest" remote add origin https://github.com/toml-lang/toml-test.git
git -C "$dest" sparse-checkout set --no-cone "tests/"
git -C "$dest" fetch -q --depth 1 --filter=blob:none origin "$SUITE_SHA"
git -C "$dest" checkout -q FETCH_HEAD
echo "corpus ready: $dest/tests"

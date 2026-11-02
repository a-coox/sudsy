#!/usr/bin/env bash
# Fetches the pinned JSON conformance corpus: nst/JSONTestSuite's test_parsing directory into
# tests/conformance/corpus/JSONTestSuite (gitignored).
set -euo pipefail

# nst/JSONTestSuite master as of 2026-11-01
SUITE_SHA=1ef36fa01286573e846ac449e8683f8833c5b26a

dest="$(cd "$(dirname "$0")" && pwd)/corpus/JSONTestSuite"
if [ -d "$dest/.git" ] && [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" = "$SUITE_SHA" ]; then
  echo "corpus already at $SUITE_SHA"
  exit 0
fi

rm -rf "$dest"
mkdir -p "$dest"
git -C "$dest" init -q
git -C "$dest" remote add origin https://github.com/nst/JSONTestSuite.git
git -C "$dest" sparse-checkout set --no-cone "test_parsing/"
git -C "$dest" fetch -q --depth 1 --filter=blob:none origin "$SUITE_SHA"
git -C "$dest" checkout -q FETCH_HEAD
echo "corpus ready: $dest/test_parsing"

#!/usr/bin/env bash
# Fetches the pinned C# conformance corpus: a shallow sparse checkout of
# dotnet/runtime's System.Private.CoreLib sources into tests/conformance/corpus/runtime (gitignored).
set -euo pipefail

# dotnet/runtime tag v10.0.0
RUNTIME_SHA=60629d14374c56f1cb51819049ad1fa529307f8d
CORPUS_PATH=src/libraries/System.Private.CoreLib/src

dest="$(cd "$(dirname "$0")" && pwd)/corpus/runtime"
if [ -d "$dest/.git" ] && [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" = "$RUNTIME_SHA" ]; then
  echo "corpus already at $RUNTIME_SHA"
  exit 0
fi

rm -rf "$dest"
mkdir -p "$dest"
git -C "$dest" init -q
git -C "$dest" remote add origin https://github.com/dotnet/runtime.git
git -C "$dest" sparse-checkout set --no-cone "$CORPUS_PATH/"
git -C "$dest" fetch -q --depth 1 --filter=blob:none origin "$RUNTIME_SHA"
git -C "$dest" checkout -q FETCH_HEAD
echo "corpus ready: $dest/$CORPUS_PATH"

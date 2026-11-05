#!/usr/bin/env bash
# Fetches the pinned Markdown corpora into tests/conformance/corpus (gitignored):
#   commonmark/spec.json   the CommonMark 0.31.2 examples
#   runtime-docs           the .md files of docs/coding-guidelines in dotnet/runtime (tag v10.0.0), for the
#                          Prettier comparison (scripts/parity.sh format prettier)
set -euo pipefail

SPEC_SHA256=d431b29d97b6f73e69d547109cf5081578fac931e72afe95639ebe766c1b2a20
RUNTIME_SHA=60629d14374c56f1cb51819049ad1fa529307f8d

root="$(cd "$(dirname "$0")" && pwd)/corpus"

dest="$root/commonmark"
mkdir -p "$dest"
if [ "$(sha256sum "$dest/spec.json" 2>/dev/null | cut -d' ' -f1)" != "$SPEC_SHA256" ]; then
  curl -sSL https://spec.commonmark.org/0.31.2/spec.json -o "$dest/spec.json"
  [ "$(sha256sum "$dest/spec.json" | cut -d' ' -f1)" = "$SPEC_SHA256" ] || { echo "spec.json does not match the pinned hash" >&2; exit 1; }
fi
echo "commonmark ready: $dest/spec.json"

dest="$root/runtime-docs"
if [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" != "$RUNTIME_SHA" ]; then
  rm -rf "$dest"
  mkdir -p "$dest"
  git -C "$dest" init -q
  git -C "$dest" remote add origin https://github.com/dotnet/runtime.git
  git -C "$dest" sparse-checkout set --no-cone "/docs/coding-guidelines/**/*.md"
  git -C "$dest" fetch -q --depth 1 --filter=blob:none origin "$RUNTIME_SHA"
  git -C "$dest" checkout -q FETCH_HEAD
fi
echo "runtime-docs ready: $dest/docs/coding-guidelines"

#!/usr/bin/env bash
# Fetches the pinned JavaScript/TypeScript corpus into tests/conformance/corpus/js/<repo> (gitignored):
# Prettier's own js, jsx and typescript format-test inputs (tag 3.9.10; no test runners, snapshots,
# error-case folders or Flow-typed files) and slices of real projects (Prettier-formatted: vuejs/core,
# docusaurus, excalidraw, MUI; unformatted: lodash, three.js, express, the TypeScript compiler, vscode's base).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs js <<'EOF'
prettier/prettier 1172b82c295c4f889fd787dc6c78c3e194ebf353 */tests/format/js/* */tests/format/jsx/* */tests/format/typescript/*
vuejs/core 8b479a89445868d474f17e2d837765e7c9ece5e4 */packages/*/src/*.ts
facebook/docusaurus c00f04e2376e057090d65d6bfb6a82d49ea3eca6 */packages/docusaurus/src/*.ts */packages/docusaurus-theme-classic/src/*.tsx
excalidraw/excalidraw 701b901a0f1ba3d0b390d9cdf63d112e25abac4b */packages/excalidraw/components/*.tsx */packages/excalidraw/*.ts
mui/material-ui dbf007c3406c7d7352089377cf620ceca6fee666 */packages/mui-material/src/*/*.tsx
lodash/lodash f299b52f39486275a9e6483b60a410e06520c538 */lodash.js
mrdoob/three.js 0a980dcf032223a274fcdd60ab56330f1321cca0 */src/*.js
expressjs/express 43fca52d28a422c8be48fd489cbd4f2d6152bab6 */lib/*.js */test/*.js
microsoft/TypeScript c63de15a992d37f0d6cec03ac7631872838602cb */src/compiler/*.ts
microsoft/vscode 51eff2913ba27b17fafe21a2a29bab1c572ffb18 */src/vs/base/common/*.ts
EOF
prettier="$(dirname "$0")/corpus/js/prettier"
find "$prettier" \( -name format.test.js -o -name __snapshots__ -o -name _errors_ -o -name flow-types \) -prune -exec rm -rf {} +
find "$prettier" -type f ! -regex '.*\.\(js\|jsx\|mjs\|cjs\|ts\|tsx\|mts\|cts\)' -delete

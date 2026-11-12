#!/usr/bin/env bash
# Build the debug CLI + C# worker and copy them to target/dogfood/ for the editor to use
# so VS Code never locks the build outputs. The extension reads the two path settings only from
# user settings (machine scope), so the script prints the lines to put there.
# Afterwards restart the server with the extension's restart command.
set -euo pipefail
cd "$(dirname "$0")/.."
out=${CARGO_TARGET_DIR:-$PWD/target}/dogfood
cargo build -p cli
dotnet build workers/csharp/Sudsy.Worker
mkdir -p "$out/worker"
cp "${CARGO_TARGET_DIR:-target}"/debug/sudsy* "$out/" 2>/dev/null || true
rm -f "$out"/sudsy.d "$out"/sudsy.pdb
cp -r workers/csharp/Sudsy.Worker/bin/Debug/net10.0/. "$out/worker/"
abs=$(cygpath -m "$out" 2>/dev/null || echo "$out")
echo "dogfood build in $out; user settings (not workspace settings) must contain:"
echo "  \"sudsy.serverPath\": \"$abs/sudsy\","
echo "  \"sudsy.csharpWorkerPath\": \"$abs/worker/Sudsy.Worker.dll\""
echo "then restart the Sudsy server in VS Code"

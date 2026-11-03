#!/usr/bin/env bash
# Build the debug CLI + C# worker and copy them to target/dogfood/ for the editor to use
# (.vscode/settings.json points there), so VS Code never locks the build outputs.
# Afterwards restart the server with the extension's restart command.
set -euo pipefail
cd "$(dirname "$0")/.."
out=target/dogfood
cargo build -p cli
dotnet build workers/csharp/Sudsy.Worker
mkdir -p "$out/worker"
cp "${CARGO_TARGET_DIR:-target}"/debug/sudsy* "$out/" 2>/dev/null || true
rm -f "$out"/sudsy.d "$out"/sudsy.pdb
cp -r workers/csharp/Sudsy.Worker/bin/Debug/net10.0/. "$out/worker/"
echo "dogfood build in $out; restart the Sudsy server in VS Code"

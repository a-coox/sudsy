#!/usr/bin/env bash
# What pattern rules and Wasm plugins cost (docs/decisions.md, "M6: Extensibility, performance").
# Builds the sample plugin (wasm32-wasip2), the release CLI and the release perf-bench, then runs
# `perf-bench extensions` on a C# tree and `perf-bench extensions-cli` on the CLI.
# Usage: tests/conformance/speed-extensions.sh [C# directory] [file limit]
# Default tree: the dotnet/runtime corpus (tests/conformance/fetch-corpus.sh), first 500 files.
set -euo pipefail
cd "$(dirname "$0")/../.."
tree=${1:-tests/conformance/corpus/runtime}
limit=${2:-500}
target=${CARGO_TARGET_DIR:-target}
export CARGO_TARGET_DIR=$target
CARGO_TARGET_DIR=$target/sample-plugin cargo build --release --target wasm32-wasip2 --quiet \
  --manifest-path tests/plugins/sample/Cargo.toml
export PERF_PLUGIN=$target/sample-plugin/wasm32-wasip2/release/sample_plugin.wasm
cargo build --release -p cli -p perf-bench --features perf-bench/mimalloc --quiet
dotnet build workers/csharp/Sudsy.Worker -c Release --nologo -v q
export SUDSY_CSHARP_WORKER=$PWD/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll
exe=$target/release/sudsy
[ -f "$exe" ] || exe=$exe.exe
# Loading the plugin in a fresh process: no compile cache, an empty one, the same one again.
cache=$(mktemp -d)
echo "plugin load, one fresh process each (ms):"
echo "    cache off:  $(SUDSY_WASM_CACHE=off "$target/release/perf-bench" plugin-load)"
echo "    cache empty: $(SUDSY_WASM_CACHE=$cache "$target/release/perf-bench" plugin-load)"
sleep 1  # the cache is written by a background thread
echo "    cache warm: $(SUDSY_WASM_CACHE=$cache "$target/release/perf-bench" plugin-load)"
rm -rf "$cache"
"$target/release/perf-bench" extensions "$tree" "$limit"
"$target/release/perf-bench" extensions-cli "$exe"

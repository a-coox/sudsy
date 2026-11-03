#!/usr/bin/env bash
# Lint cost per analysis depth on a C# tree, to compare two checkouts (docs/decisions.md, "M6:
# Extensibility, performance": the cost of M6 on the path of a project that uses no extension).
# Builds the release perf-bench and worker of THIS checkout, then runs `perf-bench semantic`.
# Usage: tests/conformance/speed-lint.sh [C# directory] [file limit]   (PERF_DEPTHS=fast,standard)
set -euo pipefail
cd "$(dirname "$0")/../.."
tree=${1:-tests/conformance/corpus/runtime}
limit=${2:-500}
target=${CARGO_TARGET_DIR:-target}
export CARGO_TARGET_DIR=$target
cargo build --release -p perf-bench -p cli --features perf-bench/mimalloc --quiet
dotnet build workers/csharp/Sudsy.Worker -c Release --nologo -v q
export SUDSY_CSHARP_WORKER=$PWD/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll
export PERF_DEPTHS=${PERF_DEPTHS:-fast,standard}
"$target/release/perf-bench" semantic "$tree" "$limit" | grep -E "files|^fast|^standard|^deep"
ls -l "$target"/release/sudsy* | grep -v "\.d$\|\.pdb$" | awk '{print "sudsy binary:", $5, "bytes"}'

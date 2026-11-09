#!/usr/bin/env bash
# The C# worker's build and its own tests: scripts/gate.sh runs workers/<name>/check.sh for every
# worker the change touches (all of them in the full gate).
set -euo pipefail
cd "$(dirname "$0")"
dotnet build
dotnet test

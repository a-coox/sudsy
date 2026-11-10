#!/usr/bin/env bash
# What a change touches, for the CI gates. Usage: ci-changes.sh BASE [HEAD]  (BASE empty or all zeros: unknown)
# Prints `code=true|false` (false: only files no check reads, so the gates are skipped) and
# `package=true|false` (extension packaging inputs changed). Unknown base: both true.
set -euo pipefail
base=${1:-} head=${2:-HEAD}
all() { echo "code=true"; echo "package=true"; exit 0; }
case "$base" in ""|0000000*) all ;; esac
files=$(git diff --name-only "$base" "$head" 2>/dev/null) || all
# Generated docs live under docs/, so only root prose files, the license, bench history and the site are skipped.
if printf '%s\n' "$files" | grep -qvE '^(README|CONTRIBUTING|CHANGELOG|SECURITY|THIRD-PARTY-NOTICES)\.md$|^LICENSE$|^bench/history\.jsonl$|^site/'; then echo "code=true"; else echo "code=false"; fi
if printf '%s\n' "$files" | grep -qE '^(editors/vscode/|workers/csharp/|crates/lang-csharp/src/|\.github/workflows/)'; then echo "package=true"; else echo "package=false"; fi

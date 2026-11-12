#!/usr/bin/env bash
# Push the current branch, then watch the CI run for HEAD until it finishes.
set -euo pipefail
cd "$(dirname "$0")/.."
if ! git diff --cached --quiet; then echo "refusing: staged but uncommitted changes" >&2; exit 1; fi
git push -u origin HEAD
sha=$(git rev-parse HEAD); id=""
for _ in $(seq 1 30); do
  id=$(gh run list --commit "$sha" --limit 1 --json databaseId --jq '.[0].databaseId // empty')
  [ -n "$id" ] && break; sleep 5
done
[ -n "$id" ] || { echo "no CI run found for $sha" >&2; exit 1; }
if gh run watch "$id" --exit-status; then echo "CI GREEN ($sha)"; else echo "CI FAILED: gh run view $id --log-failed" >&2; exit 1; fi

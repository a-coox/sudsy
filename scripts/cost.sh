#!/usr/bin/env bash
# Added/removed lines per category. Usage: scripts/cost.sh [<rev-range>]
# Default: the merge-base with origin/main up to the working tree (untracked files count as added).
cd "$(dirname "$0")/.." || exit 1
range=${1:-$(git merge-base origin/main HEAD)}
{
  git diff --numstat -M "$range" -- . ':!Cargo.lock'
  [ -n "${1:-}" ] || git ls-files -o --exclude-standard | while IFS= read -r f; do
    printf '%s\t0\t%s\n' "$(wc -l <"$f")" "$f"
  done
} | awk -F'\t' '
function cat(f) {
  if (f ~ /(^|\/)generated\.rs$|\.generated\.|\.g\.cs$|\.snap$|^schemas\/|\/registry\.rs$|^docs\/rules\/rules\.json$|package-lock\.json$/) return "generated"
  if (f ~ /^tests\/rules\//) return "rule fixtures"
  if (f ~ /^docs\/|\.md$/) return "docs"
  if (f ~ /\/tests\/.*\.rs$|^tests\/.*\.rs$/) return "Rust tests"
  if (f ~ /^tests\//) return "other test data"
  if (f ~ /\.rs$/) return "Rust src"
  return "scripts/other"
}
{ f = $3; sub(/.*=> /, "", f); gsub(/[{}]/, "", f); c = cat(f); a[c] += $1; d[c] += $2
  if (f ~ /(^|\/)rules\/.*\.yaml$/ && f !~ /^tests\//) r[f] = 1 }
END {
  n = split("Rust src|Rust tests|rule fixtures|other test data|docs|generated|scripts/other", o, "|")
  printf "%-16s %7s %7s\n", "category", "added", "removed"
  for (i = 1; i <= n; i++) { printf "%-16s %7d %7d\n", o[i], a[o[i]], d[o[i]]; A += a[o[i]]; D += d[o[i]] }
  printf "%-16s %7d %7d\n", "total", A, D
  for (k in r) m++
  if (m) printf "pattern rule files touched: %d\n", m
}'

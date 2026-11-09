#!/usr/bin/env bash
# Sourced by fetch-<lang>-corpus.sh. `fetch_tarballs <lang>` reads `repo sha tar-pattern...` lines
# from stdin and extracts the matching files of each pinned GitHub tarball into
# tests/conformance/corpus/<lang>/<repo> (gitignored).
set -euo pipefail
set -f  # the tar patterns are not for the shell

fetch_tarballs() {
  local dest repo sha patterns out p args
  dest="$(cd "$(dirname "${BASH_SOURCE[1]}")" && pwd)/corpus/$1"
  mkdir -p "$dest"
  while read -r repo sha patterns; do
    out="$dest/${repo#*/}"
    [ -d "$out" ] && continue
    rm -rf "$out.tmp"; mkdir -p "$out.tmp"
    args=()
    for p in $patterns; do args+=(--wildcards "$p"); done
    curl -sSfL "https://codeload.github.com/$repo/tar.gz/$sha" | tar -xz -C "$out.tmp" --strip-components=1 "${args[@]}"
    mv "$out.tmp" "$out"
  done
  echo "corpus ready: $dest"
}

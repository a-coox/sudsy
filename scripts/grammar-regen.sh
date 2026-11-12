#!/usr/bin/env bash
# Regenerates src/parser.c of a forked grammar in grammars/tree-sitter-<lang>/ from its grammar.js.
# Usage: scripts/grammar-regen.sh <lang>. The tree-sitter CLI is dev-only and pinned: commit the
# regenerated parser.c with the grammar.js change (build, gate and CI compile the committed C).
set -euo pipefail
cd "$(dirname "$0")/.."
pin=0.25.10
lang=${1:?usage: scripts/grammar-regen.sh <lang>}
dir=grammars/tree-sitter-$lang
[ -f "$dir/grammar.js" ] || { echo "grammar-regen.sh: no fork in $dir" >&2; exit 2; }
if [ "$(tree-sitter --version 2>/dev/null | awk '{print $2}')" != "$pin" ]; then
  cargo install tree-sitter-cli --locked --version "$pin"
fi
cd "$dir"
tree-sitter generate

#!/usr/bin/env bash
# Fetches the pinned HTML corpus into tests/conformance/corpus/html/<repo> (gitignored): Prettier's own
# html and vue format-test inputs (tag 3.9.10; the .html and .vue files, no snapshots or runners) and real
# pages: Bootstrap 5.3.3 (its JS test pages and docs site, whose Hugo templates are not HTML) and
# mdn/learning-area (a 128 MB tarball).
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs html <<'EOF'
prettier/prettier 1172b82c295c4f889fd787dc6c78c3e194ebf353 */tests/format/html/* */tests/format/vue/*
twbs/bootstrap 6e1f75f420f68e1d52733b8e407fc7c3766c9dba */js/tests/*.html */site/*.html
mdn/learning-area 21973263ba859ff3477919087b488157f9d618a6 *.html
vuejs/core 7ef6817112bb420ef47a7252821b37caae016ac9 *.vue
sveltejs/prettier-plugin-svelte 44dda204c660fafbf0e5c6c71b6f6c3c4423adce */test/formatting/samples/*/input.html
sveltejs/realworld df796708040f5200ec572b28ab7f88ecee5794dd *.svelte
EOF
prettier="$(dirname "$0")/corpus/html/prettier"
find "$prettier" -name __snapshots__ -prune -exec rm -rf {} +
find "$prettier" -type f ! -regex '.*\.\(html\|htm\|vue\)' -delete
# The plugin's samples are named input.html and read as Svelte.
find "$(dirname "$0")/corpus/html/prettier-plugin-svelte" -name input.html | while read -r f; do mv "$f" "${f%.html}.svelte"; done

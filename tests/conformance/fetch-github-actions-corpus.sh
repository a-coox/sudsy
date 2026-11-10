#!/usr/bin/env bash
# Fetches the pinned GitHub Actions corpus into tests/conformance/corpus/github-actions/<repo>
# (gitignored): actionlint's own test workflows (valid ones, ones with errors, its examples) and the
# starter workflows GitHub offers.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs github-actions <<'EOF'
rhysd/actionlint 914e7df21a07ef503a81201c76d2b11c789d3fca */testdata/ok/*.yaml */testdata/err/*.yaml */testdata/examples/*.yaml
actions/starter-workflows e3c451d60f119b71caebf13c98ac45da6e15b4b7 */ci/*.yml */code-scanning/*.yml */deployments/*.yml */automation/*.yml
EOF

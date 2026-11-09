#!/usr/bin/env bash
# Fetches the pinned gitignore-syntax corpus into tests/conformance/corpus/gitignore/<repo>
# (gitignored): the .gitignore (and .dockerignore) of moby, nvm, rustup, pyenv, asdf, bats-core
# and the dotnet install scripts.
source "$(dirname "$0")/tarball-corpus.sh"
fetch_tarballs gitignore <<'EOF'
moby/moby b40ad2123f4d2e2cb182d7f27d9b91aa64bc304e */.gitignore */.dockerignore
nvm-sh/nvm 4e8758a801eea9479b32a904f31093e410ddaba4 */.gitignore
rust-lang/rustup 71668d2439cf8b5d512282932cf531273b92f58c */.gitignore
pyenv/pyenv 0dd55f83b6a85ed5b460311ec60439c66e257208 */.gitignore
bats-core/bats-core 48d8d75a836fb322cf98bd305e2db5a84c1be419 */.gitignore
asdf-vm/asdf ae14f6e673cc45d29e4977c4e9a70c70bef48196 */.gitignore
dotnet/install-scripts ee83f9c42135205be4e0503bd472b6f99c9b7d51 */.gitignore
EOF

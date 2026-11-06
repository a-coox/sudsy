#!/usr/bin/env bash
# Fetches the pinned shell corpus into tests/conformance/corpus/shell/<repo> (gitignored): install
# scripts (dotnet, nvm, rustup, docker, Homebrew), the docker-library entrypoints and the bash
# sources of pyenv, bats-core and asdf.
set -euo pipefail
set -f  # the tar patterns are not for the shell

dest="$(cd "$(dirname "$0")" && pwd)/corpus/shell"
mkdir -p "$dest"
# repo sha tar-pattern...
while read -r repo sha patterns; do
  out="$dest/${repo#*/}"
  [ -d "$out" ] && continue
  rm -rf "$out.tmp"; mkdir -p "$out.tmp"
  args=()
  for p in $patterns; do args+=(--wildcards "$p"); done
  curl -sSfL "https://codeload.github.com/$repo/tar.gz/$sha" | tar -xz -C "$out.tmp" --strip-components=1 "${args[@]}"
  mv "$out.tmp" "$out"
done <<'EOF'
dotnet/install-scripts ee83f9c42135205be4e0503bd472b6f99c9b7d51 */src/dotnet-install.sh
nvm-sh/nvm 4e8758a801eea9479b32a904f31093e410ddaba4 */install.sh */nvm.sh
rust-lang/rustup 71668d2439cf8b5d512282932cf531273b92f58c */rustup-init.sh
docker-library/postgres dfc76bb299b057785026eb2eea39556b6fe52983 */18/trixie/docker-entrypoint.sh
docker-library/mysql 9b6c81572125a4ff828e3037c71940f3f8311978 */8.4/docker-entrypoint.sh
docker/docker-install 8ad27bd3ac871feabe62f1d926c4067800c1732f */install.sh
Homebrew/install 959bdd728192d78c47287c177796c3d86d111f09 */install.sh */uninstall.sh
pyenv/pyenv 0dd55f83b6a85ed5b460311ec60439c66e257208 *.bash *.sh
bats-core/bats-core 48d8d75a836fb322cf98bd305e2db5a84c1be419 *.bash *.sh
asdf-vm/asdf ae14f6e673cc45d29e4977c4e9a70c70bef48196 *.bash
EOF
echo "corpus ready: $dest"

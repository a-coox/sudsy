#!/usr/bin/env bash
# Builds language packs (release) for this machine's target into <out>, named
# sudsy-<version>-<language>-<target triple>.<dll|so|dylib>, and writes <out>/manifest.txt
# (`language file sha256 size`), which a pack-mode CLI embeds:
#   scripts/pack.sh out toml xml          # no languages: all
#   SUDSY_PACK_MANIFEST=$PWD/out/manifest.txt cargo build --release -p cli --no-default-features
set -euo pipefail
cd "$(dirname "$0")/.."

out=${1:?usage: pack.sh <out dir> [language...]}
shift
languages=("$@")
[ ${#languages[@]} -gt 0 ] || languages=(toml xml markdown editorconfig ini dockerfile shell powershell gitignore protobuf graphql makefile hcl)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
triple=$(rustc -vV | sed -n 's/^host: //p')
case "$(uname -s)" in
  Linux*) built=libpack.so ext=so ;;
  Darwin*) built=libpack.dylib ext=dylib ;;
  *) built=pack.dll ext=dll ;;
esac
target=${CARGO_TARGET_DIR:-target}

mkdir -p "$out"
: >"$out/manifest.txt"
for lang in "${languages[@]}"; do
  cargo build --release -p pack --features "$lang"
  file="sudsy-$version-$lang-$triple.$ext"
  cp "$target/release/$built" "$out/$file"
  echo "$lang $file $(sha256sum "$out/$file" | cut -d' ' -f1) $(wc -c <"$out/$file" | tr -d ' ')" >>"$out/manifest.txt"
done

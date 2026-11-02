#!/usr/bin/env bash
# Fetches the pinned C# conformance corpus into tests/conformance/corpus/<name> (gitignored):
# shallow, sparse checkouts of the .cs files under a few source directories of
#   runtime     dotnet/runtime tag v10.0.0 (System.Private.CoreLib)         ~28 MB
#   roslyn      dotnet/roslyn tag Visual-Studio-2026-Version-18.7           ~45 MB
#   aspnetcore  dotnet/aspnetcore tag v10.0.0                               ~20 MB
set -euo pipefail

root="$(cd "$(dirname "$0")" && pwd)/corpus"

# fetch <name> <repo> <sha> <dir>...
fetch() {
  local name=$1 repo=$2 sha=$3
  shift 3
  local dest="$root/$name"
  if [ -d "$dest/.git" ] && [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" = "$sha" ] \
    && [ "$(git -C "$dest" config sudsy.dirs 2>/dev/null)" = "$*" ]; then
    echo "$name already at $sha"
    return
  fi
  rm -rf "$dest"
  mkdir -p "$dest"
  git -C "$dest" init -q
  git -C "$dest" remote add origin "https://github.com/$repo.git"
  git -C "$dest" config sudsy.dirs "$*"
  local patterns=()
  for dir in "$@"; do
    patterns+=("/$dir/**/*.cs")
  done
  git -C "$dest" sparse-checkout set --no-cone "${patterns[@]}"
  git -C "$dest" fetch -q --depth 1 --filter=blob:none origin "$sha"
  git -C "$dest" checkout -q FETCH_HEAD
  echo "$name ready: $dest"
}

fetch runtime dotnet/runtime 60629d14374c56f1cb51819049ad1fa529307f8d \
  src/libraries/System.Private.CoreLib/src

fetch roslyn dotnet/roslyn c86410db39ebb45ae344fb1dec588b1c7802f629 \
  src/Compilers/CSharp/Portable \
  src/Compilers/Core/Portable \
  src/Workspaces/Core/Portable

fetch aspnetcore dotnet/aspnetcore 7387de91234d3ef751fa50b3d1bfede4130213ff \
  src/Http \
  src/Mvc/Mvc.Core \
  src/Middleware \
  src/Servers/Kestrel/Core/src \
  src/Components/Components/src \
  src/SignalR/common

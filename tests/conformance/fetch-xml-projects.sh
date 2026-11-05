#!/usr/bin/env bash
# Fetches real .NET project files for the XML format parity run (scripts/parity.sh format
# csharpier-xml): the MSBuild files (.csproj, .props, .targets, .config, .resx, .xml) under a few
# directories of dotnet/aspnetcore v10.0.0 (the commit of fetch-corpus.sh), shallow and sparse,
# into tests/conformance/corpus/xml-projects (gitignored).
set -euo pipefail

SHA=7387de91234d3ef751fa50b3d1bfede4130213ff

dest="$(cd "$(dirname "$0")" && pwd)/corpus/xml-projects"
if [ -d "$dest/.git" ] && [ "$(git -C "$dest" rev-parse HEAD 2>/dev/null)" = "$SHA" ]; then
  echo "corpus already at $SHA"
  exit 0
fi

rm -rf "$dest"
mkdir -p "$dest"
git -C "$dest" init -q
git -C "$dest" remote add origin https://github.com/dotnet/aspnetcore.git
patterns=()
for dir in eng src/Http src/Middleware src/Mvc src/Components/Components src/SignalR/common src/Servers/Kestrel/Core; do
  for ext in csproj props targets config resx xml; do
    patterns+=("/$dir/**/*.$ext")
  done
done
git -C "$dest" sparse-checkout set --no-cone "${patterns[@]}"
git -C "$dest" fetch -q --depth 1 --filter=blob:none origin "$SHA"
git -C "$dest" checkout -q FETCH_HEAD
echo "corpus ready: $dest"

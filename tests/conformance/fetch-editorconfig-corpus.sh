#!/usr/bin/env bash
# Fetches the pinned .editorconfig corpus into tests/conformance/corpus/editorconfig (gitignored):
# the file of dotnet/runtime, dotnet/roslyn and dotnet/aspnetcore, as <repo>.editorconfig.
set -euo pipefail

dest="$(cd "$(dirname "$0")" && pwd)/corpus/editorconfig"
mkdir -p "$dest"
while read -r repo sha; do
  out="$dest/$repo.editorconfig"
  [ -s "$out" ] || curl -sSfL "https://raw.githubusercontent.com/dotnet/$repo/$sha/.editorconfig" -o "$out"
done <<'EOF'
runtime f43900c8a9c7bec4d6dea589002f41f1b8f8816a
roslyn 3fdf3c3fff5747937bf99568f41bf12db657e014
aspnetcore 70dc164847bb029321a3d0eea9b74db1fdb48055
EOF
echo "corpus ready: $dest"

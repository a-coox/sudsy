#!/usr/bin/env bash
# Fetches the pinned PowerShell corpus into tests/conformance/corpus/powershell/<repo> (gitignored):
# dotnet-install.ps1, the build and packaging modules of PowerShell, the build scripts and rule
# sources of PSScriptAnalyzer, and the sources of Pester.
set -euo pipefail
set -f  # the tar patterns are not for the shell

dest="$(cd "$(dirname "$0")" && pwd)/corpus/powershell"
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
dotnet/install-scripts ee83f9c42135205be4e0503bd472b6f99c9b7d51 */src/dotnet-install.ps1
PowerShell/PowerShell 1aac7f5e38679b3c1469b67071a4c07f50cc064b */build.psm1 */tools/*.psm1 */tools/*.ps1
PowerShell/PSScriptAnalyzer 0151b9dafd1a7ba4279e0deeb03f04661223e629 */build.ps1 */build.psm1 */Engine/*.ps1 */Engine/*.psm1
pester/Pester 98e56d65da1642a7158992b50ded240c42a3ac18 */src/*.ps1
EOF
echo "corpus ready: $dest"

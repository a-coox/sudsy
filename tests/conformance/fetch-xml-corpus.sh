#!/usr/bin/env bash
# Fetches the pinned XML conformance corpus: the W3C XML Conformance Test Suite (2013-09-23) into
# tests/conformance/corpus/xmlconf (gitignored); the list is xmlconf/xmlconf.xml.
set -euo pipefail

SHA256=9b61db9f5dbffa545f4b8d78422167083a8568c59bd1129f94138f936cf6fc1f

dest="$(cd "$(dirname "$0")" && pwd)/corpus/xmlconf"
if [ -f "$dest/xmlconf/xmlconf.xml" ] && [ "$(cat "$dest/.sha256" 2>/dev/null)" = "$SHA256" ]; then
  echo "corpus already at $SHA256"
  exit 0
fi

rm -rf "$dest"
mkdir -p "$dest"
curl -fsSL -o "$dest/xmlts.tar.gz" https://www.w3.org/XML/Test/xmlts20130923.tar.gz
echo "$SHA256  $dest/xmlts.tar.gz" | sha256sum -c - >/dev/null
tar xzf "$dest/xmlts.tar.gz" -C "$dest"
rm "$dest/xmlts.tar.gz"
echo "$SHA256" > "$dest/.sha256"
echo "corpus ready: $dest/xmlconf"

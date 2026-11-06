#!/usr/bin/env bash
# Static grammars vs language pack: release `lint` of a 4.5 MB TOML file, interleaved wall clock
# (best and median of 8 runs per round), and the sizes of both binaries and of the pack.
set -euo pipefail
cd "$(dirname "$0")/.."
target=${CARGO_TARGET_DIR:-target}
work=$target/pack-bench
exe=$(case "$(uname -s)" in MINGW* | MSYS* | CYGWIN*) echo .exe ;; esac)
mkdir -p "$work"

scripts/pack.sh "$work" toml
cargo build --release -p cli
cp "$target/release/sudsy$exe" "$work/static$exe"
SUDSY_PACK_MANIFEST=$(cd "$work" && { pwd -W 2>/dev/null || pwd; })/manifest.txt cargo build --release -p cli --no-default-features
cp "$target/release/sudsy$exe" "$work/packed$exe"
export SUDSY_CACHE_DIR=$work/cache
"$work/packed$exe" languages add toml --from "$work"

for i in $(seq 0 59999); do
  printf '[t%d]\nname = "item %d"\nvalues = [1, 2, 3, %d]\nenabled = true\n\n' "$i" "$i" "$i"
done >"$work/big.toml"
ms() { local s e; s=$(date +%s%N); "$@" >/dev/null 2>&1; e=$(date +%s%N); echo $(((e - s) / 1000000)); }
for round in 1 2 3; do
  for b in static packed; do
    t=()
    for _ in 1 2 3 4 5 6 7 8; do t+=("$(ms "$work/$b$exe" lint --analysis fast "$work/big.toml")"); done
    sorted=($(printf '%s\n' "${t[@]}" | sort -n))
    echo "$b round $round: best ${sorted[0]} ms, median ${sorted[4]} ms"
  done
done
ls -l "$work"/static"$exe" "$work"/packed"$exe" "$work"/sudsy-*-toml-*

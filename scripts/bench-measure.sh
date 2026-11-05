#!/usr/bin/env bash
# Builds one checkout and measures it; called by scripts/bench.sh (natively and inside WSL).
#   bench-measure.sh --src DIR --target DIR --inputs DIR --out FILE --mode allocs|instr [--langs "json yaml cs"]
# Writes "<lang>.<phase>.<allocs|instr|work> <number>" lines. Both modes use the same release
# perf-bench build (--features count); `instr` runs each phase under callgrind and reads the
# instructions executed between entry and exit of fixed::measured (the phase, not the set-up).
set -euo pipefail
src= target= inputs= out= mode= langs="json yaml toml xml cs"
while [ $# -gt 0 ]; do
  case "$1" in
    --src) src=$2 ;; --target) target=$2 ;; --inputs) inputs=$2 ;; --out) out=$2 ;;
    --mode) mode=$2 ;; --langs) langs=$2 ;; *) echo "bad arg $1" >&2; exit 2 ;;
  esac; shift 2
done
[ -n "$src" ] && [ -n "$target" ] && [ -n "$inputs" ] && [ -n "$out" ] || { echo "missing args" >&2; exit 2; }
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cd "$src"
mkdir -p "$target"

CARGO_TARGET_DIR=$target/plugin cargo build --release --target wasm32-wasip2 --quiet \
  --manifest-path tests/plugins/sample/Cargo.toml
CARGO_TARGET_DIR=$target cargo build --release -p perf-bench --features count --quiet
bin=$target/release/perf-bench; [ -f "$bin" ] || bin=$bin.exe
export PERF_PLUGIN=$target/plugin/wasm32-wasip2/release/sample_plugin.wasm
# A warm compile cache, so the plugin phase measures calls and not Cranelift.
export SUDSY_WASM_CACHE=$target/wasm-cache
case " $langs " in *" cs "*)
  dotnet build workers/csharp/Sudsy.Worker -c Release --nologo -v q >/dev/null
  export SUDSY_CSHARP_WORKER=$PWD/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll ;;
esac
"$bin" fixed json plugins --inputs "$inputs" >/dev/null; sleep 1

phases() { # phases LANG
  case "$1" in json) echo parse format format-query lint lint-all patterns plugins ;; yaml) echo parse format format-query lint lint-all patterns ;;
    toml|xml) echo parse format format-query lint lint-all ;;
    cs) echo parse format format-query lint lint-all lint-docs patterns plugins ;; esac; }

one() { # one LANG PHASE -> metric lines on stdout
  local lang=$1 phase=$2 res
  if [ "$mode" = instr ]; then
    local log; log=$(mktemp)
    valgrind --tool=callgrind --smc-check=all-non-file --callgrind-out-file=/dev/null \
      --toggle-collect='*fixed*measured*' "$bin" fixed "$lang" "$phase" --inputs "$inputs" >"$log.out" 2>"$log"
    res=$(cat "$log.out")
    echo "$lang.$phase.instr $(sed -n 's/.*I *refs: *//p' "$log" | tr -d ',' | head -1)"
    rm -f "$log" "$log.out"
  else
    res=$("$bin" fixed "$lang" "$phase" --inputs "$inputs")
    echo "$lang.$phase.allocs $(sed -n 's/.* allocs=//p' <<<"$res")"
  fi
  echo "$lang.$phase.work $(sed -n 's/.* work=\([0-9]*\).*/\1/p' <<<"$res")"
}

: >"$out"
tmp=$(mktemp -d); n=0; jobs_max=$(( $(nproc 2>/dev/null || echo 4) / 2 )); [ "$jobs_max" -ge 1 ] || jobs_max=1
for lang in $langs; do
  [ "$mode" = instr ] && [ "$lang" = cs ] && continue # the worker is not traced; see docs/performance/tooling.md
  for phase in $(phases "$lang"); do
    one "$lang" "$phase" >"$tmp/$lang.$phase" &
    n=$((n+1)); [ $((n % jobs_max)) -eq 0 ] && wait
  done
done
wait
for f in "$tmp"/*; do cat "$f"; done | sort >"$out"
# A phase that failed leaves an empty value; refuse rather than compare nothing.
if grep -qE ' $' "$out" || [ ! -s "$out" ]; then echo "measurement failed:" >&2; cat "$out" >&2; exit 1; fi
rm -rf "$tmp"

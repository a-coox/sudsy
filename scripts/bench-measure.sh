#!/usr/bin/env bash
# Builds one checkout and measures it; called by scripts/bench.sh (natively and inside WSL).
#   bench-measure.sh --src DIR --target DIR --inputs DIR --out FILE --mode allocs|instr
# Writes "<lang>.<phase>.<allocs|instr|work> <number>" lines. Both modes use the same release
# perf-bench build (--features count); `instr` runs each phase under callgrind and reads the
# instructions executed between entry and exit of fixed::measured (the phase, not the set-up).
# The languages and their phases are what `perf-bench list` reports for the folders of --inputs.
# Each measured run is killed after BENCH_TIMEOUT seconds (default 900), and the script fails.
set -euo pipefail
src= target= inputs= out= mode=
while [ $# -gt 0 ]; do
  case "$1" in
    --src) src=$2 ;; --target) target=$2 ;; --inputs) inputs=$2 ;; --out) out=$2 ;;
    --mode) mode=$2 ;; *) echo "bad arg $1" >&2; exit 2 ;;
  esac; shift 2
done
[ -n "$src" ] && [ -n "$target" ] && [ -n "$inputs" ] && [ -n "$out" ] || { echo "missing args" >&2; exit 2; }
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
cd "$src"
mkdir -p "$target"
start=$SECONDS

CARGO_TARGET_DIR=$target/plugin cargo build --release --target wasm32-wasip2 --quiet \
  --manifest-path tests/plugins/sample/Cargo.toml
CARGO_TARGET_DIR=$target cargo build --release -p perf-bench --features count --quiet
bin=$target/release/perf-bench; [ -f "$bin" ] || bin=$bin.exe
built=$SECONDS
export PERF_PLUGIN=$target/plugin/wasm32-wasip2/release/sample_plugin.wasm
# A warm compile cache, so the plugin phase measures calls and not Cranelift.
export SUDSY_WASM_CACHE=$target/wasm-cache
limit=${BENCH_TIMEOUT:-900}
# Runs a command under the time limit; on overrun the command is killed and named.
limited() { # limited WHAT CMD...
  local what=$1 rc=0; shift
  timeout -k 10 "$limit" "$@" || { rc=$?; [ $rc = 124 ] && echo "TIMEOUT after ${limit}s (BENCH_TIMEOUT): $what" >&2; return $rc; }
}
rows=$("$bin" list --inputs "$inputs")
# Worker languages (C#) need dotnet, and callgrind does not trace the worker.
worker=0
if [ "$mode" != instr ] && grep -q ' worker ' <<<"$rows"; then
  if command -v dotnet >/dev/null; then
    worker=1
    dotnet build workers/csharp/Sudsy.Worker -c Release --nologo -v q >/dev/null
    export SUDSY_CSHARP_WORKER=$PWD/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll
  else echo "no dotnet: worker-language phases skipped" >&2; fi
fi
limited warm-up "$bin" fixed json plugins --inputs "$inputs" >/dev/null; sleep 1

one() { # one LANG PHASE -> metric lines on stdout
  local lang=$1 phase=$2 res t0=$SECONDS
  if [ "$mode" = instr ]; then
    local log; log=$(mktemp)
    limited "$lang $phase" valgrind --tool=callgrind --smc-check=all-non-file --callgrind-out-file=/dev/null \
      --toggle-collect='*fixed*measured*' "$bin" fixed "$lang" "$phase" --inputs "$inputs" >"$log.out" 2>"$log"
    res=$(cat "$log.out")
    echo "$lang.$phase.instr $(sed -n 's/.*I *refs: *//p' "$log" | tr -d ',' | head -1)"
    rm -f "$log" "$log.out"
  else
    res=$(limited "$lang $phase" "$bin" fixed "$lang" "$phase" --inputs "$inputs")
    echo "$lang.$phase.allocs $(sed -n 's/.* allocs=//p' <<<"$res")"
  fi
  echo "$lang.$phase.work $(sed -n 's/.* work=\([0-9]*\).*/\1/p' <<<"$res")"
  echo "$lang.$phase $((SECONDS - t0))" >>"$out.secs"
}

: >"$out"; : >"$out.secs"
tmp=$(mktemp -d); n=0; jobs_max=$(( $(nproc 2>/dev/null || echo 4) / 2 )); [ "$jobs_max" -ge 1 ] || jobs_max=1
while read -r lang kind phases; do
  [ "$kind" = worker ] && [ $worker = 0 ] && continue
  for phase in $phases; do
    one "$lang" "$phase" >"$tmp/$lang.$phase" &
    n=$((n+1)); [ $((n % jobs_max)) -eq 0 ] && wait
  done
done <<<"$rows"
wait
for f in "$tmp"/*; do cat "$f"; done | sort >"$out"
# A phase that failed leaves an empty value; refuse rather than compare nothing.
if grep -qE ' $' "$out" || [ ! -s "$out" ]; then echo "measurement failed:" >&2; cat "$out" >&2; exit 1; fi
rm -rf "$tmp"
echo "   (build $((built - start))s, measure $((SECONDS - built))s; seconds per phase: $out.secs)"

#!/usr/bin/env bash
# Builds one checkout and measures it; called by scripts/bench.sh (natively and inside WSL).
#   bench-measure.sh --src DIR --target DIR --inputs DIR --out FILE --mode allocs|instr --langs L1,L2
# Writes "<lang>.<phase>.<allocs|instr|work> <number>" lines. Both modes use the same release
# perf-bench build (--features count), whose runs print their allocation counts; `instr` also runs
# each phase under callgrind and reads the instructions executed between entry and exit of
# fixed::measured (the phase, not the set-up), except lint-all and worker phases (callgrind does
# not trace the worker), which run without it. The languages and their phases are the rows of
# `perf-bench list` for the folders of --inputs that --langs names (`startup` included). Worker
# languages need dotnet; without it they are left out and named in FILE.left.
# Each measured run is killed after BENCH_TIMEOUT seconds (default 900), and the script fails.
set -euo pipefail
src= target= inputs= out= mode= langs=
while [ $# -gt 0 ]; do
  case "$1" in
    --src) src=$2 ;; --target) target=$2 ;; --inputs) inputs=$2 ;; --out) out=$2 ;;
    --mode) mode=$2 ;; --langs) langs=$2 ;; *) echo "bad arg $1" >&2; exit 2 ;;
  esac; shift 2
done
[ -n "$src" ] && [ -n "$target" ] && [ -n "$inputs" ] && [ -n "$out" ] && [ -n "$langs" ] || { echo "missing args" >&2; exit 2; }
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
rows=$("$bin" list --inputs "$inputs" | awk -v L=",$langs," 'index(L, "," $1 ",")')
# Worker languages (C#) need dotnet.
worker=0
if grep -q ' worker ' <<<"$rows"; then
  if command -v dotnet >/dev/null; then
    worker=1
    dotnet build workers/csharp/Sudsy.Worker -c Release --nologo -v q >/dev/null
    export SUDSY_CSHARP_WORKER=$PWD/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll
  fi
fi
[ $worker = 1 ] && : >"$out.left" || awk '$2 == "worker" { print $1 }' <<<"$rows" >"$out.left"
limited warm-up "$bin" fixed json plugins --inputs "$inputs" >/dev/null; sleep 1

one() { # one LANG KIND PHASE -> metric lines on stdout
  local lang=$1 kind=$2 phase=$3 res t0=$SECONDS
  # lint-all grows with every rule and is never flagged: allocations only, no callgrind run.
  if [ "$mode" = instr ] && [ "$kind" != worker ] && [ "$phase" != lint-all ]; then
    local log total alloc; log=$(mktemp)
    limited "$lang $phase" valgrind --tool=callgrind --smc-check=all-non-file --callgrind-out-file="$log.cg" \
      --toggle-collect='*fixed*measured*' "$bin" fixed "$lang" "$phase" --inputs "$inputs" >"$log.out" 2>"$log"
    res=$(cat "$log.out")
    total=$(sed -n 's/.*I *refs: *//p' "$log" | tr -d ',' | head -1)
    # The allocator's instructions (the counting allocator's methods, inclusive) depend on the heap
    # the set-up left (one more language built before the phase shifts them), so they are reported
    # apart: .instr is the rest, .alloc-instr informational; the allocation count still flags.
    alloc=$(callgrind_annotate --inclusive=yes --threshold=100 "$log.cg" \
      | awk '/Counting.*GlobalAlloc(5alloc|7dealloc|7realloc|>::(alloc|dealloc|realloc))/ { n = $1; gsub(",", "", n); s += n } END { print s + 0 }')
    echo "$lang.$phase.instr $((total - alloc))"
    echo "$lang.$phase.alloc-instr $alloc"
    rm -f "$log" "$log.out" "$log.cg"
  else
    res=$(limited "$lang $phase" "$bin" fixed "$lang" "$phase" --inputs "$inputs")
  fi
  echo "$lang.$phase.allocs $(sed -n 's/.* allocs=//p' <<<"$res")"
  echo "$lang.$phase.work $(sed -n 's/.* work=\([0-9]*\).*/\1/p' <<<"$res")"
  echo "$lang.$phase $((SECONDS - t0))" >>"$out.secs"
}

# A job pool, longest phase (by the last run's seconds) first, so no slot idles behind a slow phase.
prev=$(cat "$out.secs" 2>/dev/null || true)
: >"$out"; : >"$out.secs"
queue=$(while read -r lang kind phases; do
  [ "$kind" = worker ] && [ $worker = 0 ] && continue
  for phase in $phases; do
    echo "$(awk -v k="$lang.$phase" '$1 == k { print $2 }' <<<"$prev" | tail -1) $lang $kind $phase"
  done
done <<<"$rows" | awk '{ if (NF == 3) $0 = "0 " $0; print }' | sort -s -rn -k1,1)
# Only left-out languages (C# without dotnet): nothing measured here.
[ -n "$queue" ] || { echo "   (nothing to measure here; left out: $(paste -sd, - <"$out.left"))"; exit 0; }
# All logical CPUs: a callgrind run is single-threaded and peaks near 120 MB, and 12 jobs took 65 s
# where 6 took 124 s (hyperthreads included).
tmp=$(mktemp -d); running=0; jobs_max=$(nproc 2>/dev/null || echo 4)
while read -r _ lang kind phase; do
  one "$lang" "$kind" "$phase" >"$tmp/$lang.$phase" &
  running=$((running + 1)); if [ $running -ge $jobs_max ]; then wait -n || true; running=$((running - 1)); fi
done <<<"$queue"
wait
for f in "$tmp"/*; do cat "$f"; done | sort >"$out"
# A phase that failed leaves an empty value; refuse rather than compare nothing.
if grep -qE ' $' "$out" || [ ! -s "$out" ]; then echo "measurement failed:" >&2; cat "$out" >&2; exit 1; fi
rm -rf "$tmp"
echo "   (build $((built - start))s, measure $((SECONDS - built))s; seconds per phase: $out.secs)"

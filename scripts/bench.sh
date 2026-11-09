#!/usr/bin/env bash
# Standard benchmark: run after every feature change.
#   scripts/bench.sh [--base REV] [--time] [--rounds N] [--no-instr] [--no-record]
#   scripts/bench.sh --stop: end this slot's running bench, its WSL processes included
# Tier 1 (default, deterministic): per language and phase (parse, format, format-query, lint, patterns, plugins) on
#   fixed inputs (bench/inputs + generated), the candidate (working tree) against the baseline
#   (--base, default the merge-base with origin/main; HEAD or HEAD^ when that is HEAD), built in a
#   git worktree: instructions (callgrind, in WSL on Windows) and allocations. Exit 1 on a
#   regression: instructions +1%, allocations +1% (and 10 or more); the lint-all and lint-docs metrics (every
#   rule on, resp. the C# documentation rules, so they grow with each new rule) are printed but never flagged.
# Tier 2 (--time): interleaved A/B wall time of the CLI, with the C# worker; exit 1 only when the
#   95% confidence interval of the candidate/baseline ratio lies above 1.03.
# Languages: the folders of bench/inputs that a provider claims (`perf-bench list`).
# Each measured run is killed after BENCH_TIMEOUT seconds (default 900; inside WSL for the
# instruction runs) and the run fails.
# Run from Git Bash on Windows (WSL needs scripts/bench-setup-wsl.sh once) or natively on Linux.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
# WSL cache key: the slot (CARGO_TARGET_DIR name) when set, else the checkout name.
slot=$(basename "${CARGO_TARGET_DIR:-$root}")
base_rev= time=0 rounds=11 instr=1 record=1 stop=0
while [ $# -gt 0 ]; do
  case "$1" in
    --base) base_rev=$2; shift ;; --time) time=1 ;; --rounds) rounds=$2; shift ;;
    --no-instr) instr=0 ;; --no-record) record=0 ;; --stop) stop=1 ;;
    *) sed -n '2,12p' "$0" >&2; exit 2 ;;
  esac; shift
done

windows=0; case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) windows=1 ;; esac
distro=${BENCH_WSL_DISTRO:-Ubuntu}
wsl_path() { local p; p=$(cygpath -m "$1"); echo "/mnt/$(echo "${p:0:1}" | tr A-Z a-z)${p:2}"; }
wslrun() { MSYS_NO_PATHCONV=1 wsl -d "$distro" -e bash -c "export PATH=\$HOME/.local/bin:\$HOME/.cargo/bin:\$PATH; $1"; }

# --- --stop: killing bench's shell leaves its builds and measurements running (under Git Bash the
# Windows process tree does not follow the shells, and no Windows signal reaches WSL). So: bench
# itself by its pid file (it starts nothing more), every bench-measure of this slot with its
# children (they name `--target $work/...`), and the slot's WSL process groups.
work=${CARGO_TARGET_DIR:-$root/target}/bench; mkdir -p "$work"
if [ $stop = 1 ]; then
  p=$(cat "$work/pid" 2>/dev/null || true)
  # Only a bench.sh: a stale file can name a pid given to something else since.
  [ -n "$p" ] && grep -q bench.sh "/proc/$p/cmdline" 2>/dev/null && kill -9 "$p" 2>/dev/null
  if [ $windows = 1 ]; then
    powershell -NoProfile -Command "Get-CimInstance Win32_Process | Where-Object { \$_.CommandLine -like '*--target $work/*' } | ForEach-Object { taskkill /F /T /PID \$_.ProcessId }" >/dev/null 2>&1 || true
    wslrun "for p in \$(pgrep -f '[s]udsy-bench/$slot/'); do kill -- -\$(ps -o pgid= -p \$p 2>/dev/null | tr -d ' ') 2>/dev/null; done; true"
  else
    pkill -f -- "--target $work/" || true
  fi
  rm -f "$work/pid"; echo "bench stopped"; exit 0
fi
echo $$ >"$work/pid"
trap 'rm -f "$work/pid"' EXIT

# --- revisions --------------------------------------------------------------------------------
head=$(git rev-parse HEAD)
dirty=0; [ -n "$(git status --porcelain --untracked-files=no)" ] && dirty=1
if [ -z "$base_rev" ]; then
  base_rev=$(git merge-base HEAD origin/main 2>/dev/null || git rev-parse HEAD^)
  if [ "$base_rev" = "$head" ]; then [ $dirty = 1 ] || base_rev=$(git rev-parse HEAD^); fi
fi
base=$(git rev-parse "$base_rev")
echo "candidate: ${head:0:9}$([ $dirty = 1 ] && echo '+working tree')   baseline: ${base:0:9}"
[ "$base" = "$head" ] && [ $dirty = 0 ] && { echo "nothing to compare" >&2; exit 2; }

# --- baseline worktree, with the candidate's harness so both sides run the same workload -----------
src_base=$work/base-src; out=$work/out
mkdir -p "$out"
# A restored build cache can hold the directory without the worktree registration: start over then.
[ -e "$src_base/.git" ] && git -C "$src_base" checkout -q -f --detach "$base" 2>/dev/null   || { rm -rf "$src_base"; git worktree prune; git worktree add -q --detach "$src_base" "$base"; }
rm -rf "$src_base/tools/perf-bench"; cp -r tools/perf-bench "$src_base/tools/perf-bench"
inputs=$root/bench/inputs

# --- tier 1 ---------------------------------------------------------------------------------------
measure() { # measure SIDE SRC MODE ; native
  scripts/bench-measure.sh --src "$2" --target "$work/$1" --inputs "$inputs" --mode "$3" --out "$out/$1.$3"
}
instr_ok=$instr
if [ $instr = 1 ]; then
  if [ $windows = 1 ]; then
    wslrun 'command -v valgrind && command -v cargo && rustup target list --installed | grep -q wasm32-wasip2' >/dev/null 2>&1 \
      || { echo "WSL has no valgrind/rust yet: run scripts/bench-setup-wsl.sh once (it asks for your sudo password), or use --no-instr" >&2; exit 2; }
  else
    command -v valgrind >/dev/null || { echo "valgrind missing: run scripts/bench-setup-wsl.sh, or use --no-instr" >&2; exit 2; }
  fi
fi
# The baseline's counts depend only on its commit, the workload (candidate harness, inputs, measure
# script) and the toolchains, so a rerun against the same baseline reuses them.
key=$({ echo "$base $instr"; rustc -V; dotnet --version 2>/dev/null || true
  [ $instr = 1 ] && [ $windows = 1 ] && wslrun 'rustc -V; valgrind --version'
  find tools/perf-bench bench/inputs scripts/bench-measure.sh -type f | sort | xargs sha1sum; } | sha1sum | cut -c1-16)
cache=$work/base-cache/$key
for side in cand base; do
  if [ $side = cand ]; then s=$root; else s=$src_base; fi
  if [ $side = base ] && [ -f "$cache.allocs" ]; then
    echo "== base: cached ($cache)"; cp "$cache.allocs" "$out/base.allocs"; [ $instr = 0 ] || cp "$cache.instr" "$out/base.instr"
    continue
  fi
  echo "== $side: allocations (native)"; measure $side "$s" allocs
  if [ $instr = 1 ]; then
    echo "== $side: instructions"
    if [ $windows = 1 ]; then
      wslrun "$(wsl_path "$root")/scripts/bench-measure.sh --src $(wsl_path "$s") --target \$HOME/.cache/sudsy-bench/$slot/$side --inputs $(wsl_path "$inputs") --mode instr --out \$HOME/.cache/sudsy-bench/$slot/$side.instr"
      wslrun "cat \$HOME/.cache/sudsy-bench/$slot/$side.instr" >"$out/$side.instr"
    else
      scripts/bench-measure.sh --src "$s" --target "$work/$side" --inputs "$inputs" --mode instr \
        --out "$out/$side.instr"
    fi
  fi
  if [ $side = base ]; then mkdir -p "$work/base-cache"; cp "$out/base.allocs" "$cache.allocs"; [ $instr = 0 ] || cp "$out/base.instr" "$cache.instr"; fi
done
for side in cand base; do cat "$out"/$side.allocs "$out"/$side.instr 2>/dev/null | sort >"$out/$side.all" || true; done

# --- compare ----------------------------------------------------------------------------------------
status=0
awk -v B="$out/base.all" -v C="$out/cand.all" 'BEGIN {
  while ((getline l < B) > 0) { split(l, a, " "); b[a[1]] = a[2] }
  while ((getline l < C) > 0) { split(l, a, " "); c[a[1]] = a[2]; k[++n] = a[1] }
  printf "%-26s %14s %14s %8s\n", "metric", "baseline", "candidate", "change"
  for (i = 1; i <= n; i++) {
    m = k[i]; if (!(m in b)) { printf "%-26s %14s %14d  (new)\n", m, "-", c[m]; continue }
    d = (b[m] == 0) ? 0 : 100 * (c[m] - b[m]) / b[m]
    if (m ~ /\.work$/) { if (c[m] != b[m]) printf "%-26s %14d %14d  output size changed\n", m, b[m], c[m]; continue }
    flag = ""; if (m ~ /lint-(all|docs)/) flag = "  (informational)"; else if (d >= 1 && (m ~ /\.instr$/ || c[m] - b[m] >= 10)) { flag = "  REGRESSION"; bad++ } else if (d <= -1) flag = "  improved"
    printf "%-26s %14d %14d %+7.2f%%%s\n", m, b[m], c[m], d, flag
  }
  exit bad > 0
}' || status=1

# --- history ----------------------------------------------------------------------------------------
if [ "$record" = 1 ]; then
  host=$(hostname)-$(uname -s | cut -c1-5)
  counts=$(awk '{ printf "%s\"%s\":%s", (NR>1?",":""), $1, $2 }' "$out/cand.all")
  mkdir -p bench
  printf '{"kind":"counts","commit":"%s","base":"%s","dirty":%s,"date":"%s","host":"%s","rustc":"%s","regressed":%s,"counts":{%s}}\n' \
    "$head" "$base" "$([ $dirty = 1 ] && echo true || echo false)" "$(date -u +%FT%TZ)" "$host" \
    "$(rustc -V | cut -d' ' -f2)" "$([ $status = 1 ] && echo true || echo false)" "$counts" >>bench/history.jsonl
  echo "recorded in bench/history.jsonl"
fi

# --- tier 2 -----------------------------------------------------------------------------------------
if [ $time = 1 ]; then
  scripts/bench-time.sh --base-src "$src_base" --rounds "$rounds" --head "$head" --base "$base" \
    --record "$record" || status=1
fi
[ $status = 0 ] && echo "BENCH OK (${SECONDS}s)" || echo "BENCH: regression (${SECONDS}s)" >&2
exit $status

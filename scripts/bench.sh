#!/usr/bin/env bash
# Standard benchmark: run after every feature change.
#   scripts/bench.sh [--base REV] [--all] [--time] [--rounds N] [--no-instr] [--no-record]
#   scripts/bench.sh --stop: end this slot's running bench, its WSL processes included
# Tier 1 (default, deterministic): per language and phase (parse, format, format-query, lint, patterns, plugins) on
#   fixed inputs (bench/inputs + generated), the candidate (working tree) against the baseline
#   (--base, default the merge-base with origin/main; HEAD or HEAD^ when that is HEAD), built in a
#   git worktree: instructions (callgrind, in WSL on Windows) and allocations. Exit 1 on a
#   regression: instructions +1%, allocations +1% (and 10 or more); the lint-all and lint-docs metrics (every
#   rule on, resp. the C# documentation rules, so they grow with each new rule) are printed but never flagged,
#   and lint-all is counted in allocations only. The startup metrics build every provider, so the baseline
#   is scaled to the candidate's provider count (startup.providers.work): a new language at the usual cost is no regression.
#   startup.providers (building every provider, which each new language changes by its own cost) is informational.
# Tier 2 (--time): interleaved A/B wall time of the CLI, with the C# worker; exit 1 only when the
#   95% confidence interval of the candidate/baseline ratio lies above 1.03.
# Languages: the `bench` rows of `cargo xtask affected` (the bench/inputs folders whose language the
#   change touches; --all: every folder a provider claims). Nothing affected: nothing measured, exit 0.
# Each measured run is killed after BENCH_TIMEOUT seconds (default 900; inside WSL for the
# instruction runs) and the run fails.
# Run from Git Bash on Windows (WSL needs scripts/bench-setup-wsl.sh once) or natively on Linux.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
# WSL cache key: the slot (CARGO_TARGET_DIR name) when set, else the checkout name.
slot=$(basename "${CARGO_TARGET_DIR:-$root}")
base_rev= all=() time=0 rounds=11 instr=1 record=1 stop=0
while [ $# -gt 0 ]; do
  case "$1" in
    --base) base_rev=$2; shift ;; --all) all=(--all) ;; --time) time=1 ;; --rounds) rounds=$2; shift ;;
    --no-instr) instr=0 ;; --no-record) record=0 ;; --stop) stop=1 ;;
    *) sed -n '2,15p' "$0" >&2; exit 2 ;;
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

# --- what to measure: rows "<folder> <crates|*>" ----------------------------------------------------
sel=$(cargo xtask affected --base "$base" "${all[@]}")
rows=$(sed -n 's/^bench //p' <<<"$sel")
[ -n "$rows" ] || { echo "nothing affected: no language to bench"; echo "BENCH OK (${SECONDS}s)"; exit 0; }

src_base=$work/base-src out=$work/out inputs=$root/bench/inputs
mkdir -p "$out"; rm -f "$out"/*.allocs "$out"/*.instr
# The baseline worktree, with the candidate's harness so both sides run the same workload.
base_src() {
  [ -z "${base_ready:-}" ] || return 0
  # A restored build cache can hold the directory without the worktree registration: start over then.
  [ -e "$src_base/.git" ] && git -C "$src_base" checkout -q -f --detach "$base" 2>/dev/null \
    || { rm -rf "$src_base"; git worktree prune; git worktree add -q --detach "$src_base" "$base"; }
  rm -rf "$src_base/tools/perf-bench"; cp -r tools/perf-bench "$src_base/tools/perf-bench"
  base_ready=1
}

# --- tier 1 ---------------------------------------------------------------------------------------
measure() { # measure SIDE SRC MODE LANGS ; native
  scripts/bench-measure.sh --src "$2" --target "$work/$1" --inputs "$inputs" --mode "$3" --langs "$4" --out "$out/$1.$3"
}
if [ $instr = 1 ]; then
  if [ $windows = 1 ]; then
    wslrun 'command -v valgrind && command -v cargo && rustup target list --installed | grep -q wasm32-wasip2' >/dev/null 2>&1 \
      || { echo "WSL has no valgrind/rust yet: run scripts/bench-setup-wsl.sh once (it asks for your sudo password), or use --no-instr" >&2; exit 2; }
  else
    command -v valgrind >/dev/null || { echo "valgrind missing: run scripts/bench-setup-wsl.sh, or use --no-instr" >&2; exit 2; }
  fi
fi
# A language's baseline counts depend only on the code it runs, its inputs, the workload (candidate
# harness, measure script) and the toolchains. The code: the baseline's tree minus what no
# measurement reads (docs, tests but the sample plugin, scripts, editors) and minus the language
# crates other than the row's own, so a baseline commit that touches one language keeps the
# others' counts. The cache is shared by the slots of the machine (it sits next to them): one
# measurement serves every agent.
tools=$({ echo "$instr"; rustc -V; dotnet --version 2>/dev/null || true
  [ $instr = 1 ] && [ $windows = 1 ] && wslrun 'rustc -V; valgrind --version'
  find tools/perf-bench scripts/bench-measure.sh -type f | sort | xargs sha1sum; } | sha1sum)
tree=$(git ls-tree "$base" | awk -F'\t' '$2 !~ /^(crates|docs|editors|site|scripts|bench|tests|\.github|\.vscode|LICENSE|\.gitignore|\.gitattributes)$|\.md$/')
plugins=$(git rev-parse "$base:tests/plugins")
crate_trees=$(git ls-tree "$base" crates/)
row_key() { # row_key FOLDER CRATES (comma-separated: the language's and those its inputs embed; '*': all)
  { echo "$1 $tools $plugins"; echo "$tree"
    awk -F'\t' -v keep="$2" 'BEGIN { n = split(keep, k, ","); for (i = 1; i <= n; i++) mine["crates/" k[i]] }
      keep == "*" || $2 !~ /^crates\/lang-/ || ($2 in mine)' <<<"$crate_trees"
    [ ! -d "$inputs/$1" ] || find "$inputs/$1" -type f | sort | xargs sha1sum | cut -d' ' -f1; } | sha1sum | cut -c1-16
}
# BENCH_CACHE: another directory (scripts/devloop-times.sh times a run without cached baselines).
if [ -n "${BENCH_CACHE:-}" ]; then cache=$BENCH_CACHE
elif [ -n "${CARGO_TARGET_DIR:-}" ]; then cache=$(dirname "$CARGO_TARGET_DIR")/bench-cache; else cache=$work/base-cache; fi
mkdir -p "$cache"
modes=allocs; [ $instr = 0 ] || modes="allocs instr"
langs= missing= keys= fresh=
while read -r folder crate; do
  k=$(row_key "$folder" "$crate"); keys="$keys $folder=$k"; langs="$langs,$folder"
  # A language whose crate is absent at the baseline is measured on the candidate only ("new").
  if [ "$crate" != '*' ]; then
    for c in ${crate//,/ }; do
      git cat-file -e "$base:crates/$c" 2>/dev/null || { fresh="$fresh,$folder"; continue 2; }
    done
  fi
  # Allocations are never empty; instructions are for a worker language (callgrind does not trace it).
  { [ -s "$cache/$k.allocs" ] && { [ $instr = 0 ] || [ -f "$cache/$k.instr" ]; }; } || missing="$missing,$folder"
done <<<"$rows"
langs=${langs#,} missing=${missing#,}
key_of() { tr ' ' '\n' <<<"$keys" | sed -n "s/^$1=//p"; }
echo "languages: $langs   baseline cached: $(tr ',' '\n' <<<"$langs" | grep -vxF -f <(tr ',' '\n' <<<"$missing") | tr '\n' ' ')"

for side in cand base; do
  if [ $side = cand ]; then s=$root l=$langs; else
    [ -n "$missing" ] || break
    base_src; s=$src_base l=$missing
  fi
  echo "== $side: allocations (native): $l"; measure $side "$s" allocs "$l"
  if [ $instr = 1 ]; then
    echo "== $side: instructions"
    if [ $windows = 1 ]; then
      wslrun "$(wsl_path "$root")/scripts/bench-measure.sh --src $(wsl_path "$s") --target \$HOME/.cache/sudsy-bench/$slot/$side --inputs $(wsl_path "$inputs") --mode instr --langs $l --out \$HOME/.cache/sudsy-bench/$slot/$side.instr"
      wslrun "cat \$HOME/.cache/sudsy-bench/$slot/$side.instr" >"$out/$side.instr"
    else
      scripts/bench-measure.sh --src "$s" --target "$work/$side" --inputs "$inputs" --mode instr --langs "$l" \
        --out "$out/$side.instr"
    fi
  fi
  if [ $side = base ]; then # one entry per language, renamed into place so a reader never sees half a file
    for folder in $(tr ',' ' ' <<<"$missing"); do
      k=$(key_of "$folder")
      for m in $modes; do
        grep "^$folder\." "$out/base.$m" >"$cache/$k.$m.$$" || [ $m = instr ] || { echo "no baseline counts for $folder" >&2; exit 1; }
        mv "$cache/$k.$m.$$" "$cache/$k.$m"
      done
    done
  fi
done
for m in $modes; do
  for folder in $(tr ',' ' ' <<<"$langs"); do
    case ",$fresh," in *",$folder,"*) continue ;; esac
    cat "$cache/$(key_of "$folder").$m"
  done >"$out/base.$m"
done
for side in cand base; do cat "$out"/$side.allocs "$out"/$side.instr 2>/dev/null | sort >"$out/$side.all" || true; done

# --- compare ----------------------------------------------------------------------------------------
status=0
awk -v B="$out/base.all" -v C="$out/cand.all" -v BUD=bench/budgets.txt 'BEGIN {
  while ((getline l < BUD) > 0) if (l !~ /^#/) { split(l, a, " "); bud[a[1]] = a[2] }
  while ((getline l < B) > 0) { split(l, a, " "); b[a[1]] = a[2] }
  while ((getline l < C) > 0) { split(l, a, " "); c[a[1]] = a[2]; k[++n] = a[1] }
  printf "%-26s %14s %14s %8s\n", "metric", "baseline", "candidate", "change"
  for (i = 1; i <= n; i++) {
    m = k[i]; if (!(m in b)) { printf "%-26s %14s %14d  (new)\n", m, "-", c[m]; if ((m in bud) && c[m] > bud[m]) bad++; continue }
    # startup scales with the number of providers: compare against the baseline scaled to the candidate count
    if (m ~ /^startup\.(providers|format|lint)\.(instr|allocs)$/ && b["startup.providers.work"] > 0) b[m] = b[m] * c["startup.providers.work"] / b["startup.providers.work"]
    d = (b[m] == 0) ? 0 : 100 * (c[m] - b[m]) / b[m]
    if (m ~ /\.work$/) { if (c[m] != b[m]) printf "%-26s %14d %14d  output size changed\n", m, b[m], c[m]; continue }
    flag = ""; if (m ~ /lint-(all|docs)/ || m ~ /^startup\.providers\./) flag = "  (informational)"; else if (d >= 1 && (m ~ /\.instr$/ || c[m] - b[m] >= 10)) { flag = "  REGRESSION"; bad++ } else if (d <= -1) flag = "  improved"
    if ((m in bud) && c[m] > bud[m]) { flag = flag "  OVER BUDGET " bud[m]; bad++ }
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
  base_src
  scripts/bench-time.sh --base-src "$src_base" --rounds "$rounds" --head "$head" --base "$base" \
    --record "$record" || status=1
fi
[ $status = 0 ] && echo "BENCH OK (${SECONDS}s)" || echo "BENCH: regression (${SECONDS}s)" >&2
exit $status

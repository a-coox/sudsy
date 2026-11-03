#!/usr/bin/env bash
# Tier 2 of scripts/bench.sh: interleaved A/B wall time of the CLI (candidate = this checkout,
# baseline = --base-src), including the C# worker. Each round times baseline and candidate in
# alternating order; the report is the median of the per-round ratios candidate/baseline with a
# bootstrap 95% confidence interval. Only meaningful within one host and one run.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD; work=$root/target/bench
base_src= rounds=11 head= base= record=0
while [ $# -gt 0 ]; do
  case "$1" in --base-src) base_src=$2 ;; --rounds) rounds=$2 ;; --head) head=$2 ;; --base) base=$2 ;;
    --record) record=$2 ;; *) echo "bad arg $1" >&2; exit 2 ;; esac; shift 2
done
exe() { local e=$work/$1/release/sudsy; [ -f "$e" ] || e=$e.exe; echo "$e"; }
for side in cand base; do
  s=$root; [ $side = base ] && s=$base_src
  (cd "$s" && CARGO_TARGET_DIR=$work/$side cargo build --release -p cli --quiet \
    && dotnet build workers/csharp/Sudsy.Worker -c Release --nologo -v q >/dev/null)
done
worker() { echo "$1/workers/csharp/Sudsy.Worker/bin/Release/net10.0/Sudsy.Worker.dll"; }

# The large generated inputs plus the committed subset, copied so `format` never touches them.
data=$work/time-data; rm -rf "$data"; mkdir -p "$data"
cp -r bench/inputs/. "$data/"
"$work/cand/release/perf-bench" --write "$data/large" 2>/dev/null \
  || "$work/cand/release/perf-bench.exe" --write "$data/large"

run() { # run SIDE ARGS... -> milliseconds
  local side=$1; shift; local s=$root; [ $side = base ] && s=$base_src
  local t0 t1; t0=$(date +%s%N)
  (cd "$data" && SUDSY_CSHARP_WORKER=$(worker "$s") "$(exe $side)" "$@" >/dev/null 2>&1) || true
  t1=$(date +%s%N); echo $(( (t1 - t0) / 1000000 ))
}
cmds=("startup:--version" "format:format --check ." "lint:lint .")
bad=0
echo "== wall time, $rounds interleaved rounds (ratio = candidate / baseline)"
for c in "${cmds[@]}"; do
  name=${c%%:*}; args=${c#*:}
  run cand $args >/dev/null; run base $args >/dev/null # warm-up: caches, worker start
  rows=
  for r in $(seq 1 "$rounds"); do
    if [ $((r % 2)) = 0 ]; then b=$(run base $args); a=$(run cand $args); else a=$(run cand $args); b=$(run base $args); fi
    rows="$rows$a $b"$'\n'
  done
  res=$(awk 'BEGIN { srand(1) } NF { n++; a[n] = $1; b[n] = $2; r[n] = $1 / ($2 > 0 ? $2 : 1); sa += $1; sb += $2 }
    function med(v, m,   i, j, t, w) { for (i = 1; i <= m; i++) w[i] = v[i]
      for (i = 2; i <= m; i++) { t = w[i]; for (j = i - 1; j >= 1 && w[j] > t; j--) w[j+1] = w[j]; w[j+1] = t }
      return m % 2 ? w[(m+1)/2] : (w[m/2] + w[m/2+1]) / 2 }
    END { for (k = 1; k <= 1000; k++) { for (i = 1; i <= n; i++) s[i] = r[int(rand() * n) + 1]; bs[k] = med(s, n) }
      for (i = 2; i <= 1000; i++) { t = bs[i]; for (j = i - 1; j >= 1 && bs[j] > t; j--) bs[j+1] = bs[j]; bs[j+1] = t }
      printf "%.3f %.3f %.3f %.0f %.0f", med(r, n), bs[25], bs[975], sa / n, sb / n }' <<<"$rows")
  read -r ratio lo hi ma mb <<<"$res"
  verdict=""; if awk -v l="$lo" 'BEGIN { exit !(l > 1.03) }'; then verdict="  REGRESSION"; bad=1
  elif awk -v h="$hi" 'BEGIN { exit !(h < 0.97) }'; then verdict="  faster"; fi
  printf "%-8s base %6d ms  cand %6d ms  ratio %.3f  CI [%.3f, %.3f]%s\n" "$name" "$mb" "$ma" "$ratio" "$lo" "$hi" "$verdict"
  [ "$record" = 1 ] && printf '{"kind":"time","commit":"%s","base":"%s","date":"%s","host":"%s","cmd":"%s","rounds":%s,"ratio":%s,"ci":[%s,%s],"base_ms":%s,"cand_ms":%s}\n' \
    "$head" "$base" "$(date -u +%FT%TZ)" "$(hostname)-$(uname -s | cut -c1-5)" "$name" "$rounds" "$ratio" "$lo" "$hi" "$mb" "$ma" >>bench/history.jsonl
done
exit $bad

#!/usr/bin/env bash
# The check of a finished change (CLAUDE.md): the gate and bench at the same time on the working
# tree, both on what the change affects (`cargo xtask affected`; --all: everything), about as long
# as the slower of the two. A gate failure stops the bench at once, since the change gets edited
# anyway. Ends in FINISH OK or FINISH FAILED: <gate|bench>; exit 0 only when both pass. Arguments go
# to bench.sh (--lang FOLDER: measure those folders instead of the selection), except -p CRATE
# (repeatable), which the gate checks instead of the selection. At most SUDSY_FINISH_SEATS
# (default 1) finish runs build at once on the machine; the rest wait in line before any build starts.
set -uo pipefail
cd "$(dirname "$0")/.."
# The gate selects against the same base as the bench.
gate=(--affected) bench=() pkg=()
while [ $# -gt 0 ]; do
  case "$1" in
    -p) pkg+=(-p "$2"); shift ;;
    --all) gate=(); bench+=(--all) ;;
    --base) gate+=(--base "$2"); bench+=(--base "$2"); shift ;;
    --lang|--rounds) bench+=("$1" "$2"); shift ;;
    *) bench+=("$1") ;;
  esac; shift
done
[ ${#pkg[@]} -gt 0 ] && gate=("${pkg[@]}")
# The cheap checks agents most often miss fail before the wait for a seat and the builds.
cargo fmt --all -- --check >/dev/null || { echo "FINISH FAILED: gate (cargo fmt --all)"; exit 1; }
. scripts/slot-run.sh; claim_slot; touch_changed; sweep_slot || exit 1; take_seat
# codegen --check builds xtask, which the selection needs anyway: inside the seat, before the gate and bench.
cargo xtask codegen --check || { echo "FINISH FAILED: gate (stale generated files: cargo xtask codegen)"; exit 1; }
log=${CARGO_TARGET_DIR:-target}/bench/finish.log
mkdir -p "$(dirname "$log")"
scripts/bench.sh "${bench[@]}" >"$log" 2>&1 &
bench=$!
scripts/gate.sh "${gate[@]}"
if [ $? != 0 ]; then
  scripts/bench.sh --stop
  wait $bench 2>/dev/null
  echo "FINISH FAILED: gate (bench stopped)"
  exit 1
fi
wait $bench
status=$?
echo "== bench"
cat "$log"
[ -s "$log" ] || echo "bench.sh exited $status without output"
[ $status = 0 ] && echo "FINISH OK" || echo "FINISH FAILED: bench"
exit $status

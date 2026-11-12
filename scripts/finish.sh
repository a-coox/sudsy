#!/usr/bin/env bash
# The check of a finished change (CLAUDE.md): the gate locally and the bench (bench.yml) on GitHub
# Actions, at the same time, both on what the change affects (`cargo xtask affected`; --all:
# everything). The working tree (untracked files included) is pushed as a snapshot commit to
# bench/<slot> and benched there against the merge-base with origin/main; the branch is deleted
# afterwards. A gate failure cancels the bench run at once, since the change gets edited anyway.
# Ends in FINISH OK or FINISH FAILED: <gate|bench>; exit 0 only when both pass. On success the
# bench's history line is appended to bench/history.jsonl. Arguments go to bench.sh (--lang FOLDER:
# measure those folders instead of the selection), except -p CRATE (repeatable), which the gate
# checks instead of the selection. At most SUDSY_FINISH_SEATS (default 1) gates run at once on the
# machine; the rest wait in line. SUDSY_FINISH_REF: the ref whose bench.yml is dispatched (default
# main; a branch to try a changed workflow). `finish.sh --stop` cancels this slot's bench run.
set -uo pipefail
cd "$(dirname "$0")/.."
slot=$(basename "${CARGO_TARGET_DIR:-$PWD}")
wf_ref=${SUDSY_FINISH_REF:-main}
# The CI run of this slot: its title is `bench <slot>-<snapshot>-<epoch>`.
slot_runs() { gh run list --workflow bench.yml --limit 30 --json databaseId,displayTitle,status \
  --jq ".[] | select(.displayTitle | startswith(\"bench $slot-\")) | select(.status != \"completed\") | .databaseId"; }
if [ "${1:-}" = --stop ]; then
  for r in $(slot_runs); do gh run cancel "$r"; done; echo "bench stopped"; exit 0
fi
# The gate selects against the same base as the bench.
gate=(--affected) bench=() pkg=() base_rev=
while [ $# -gt 0 ]; do
  case "$1" in
    -p) pkg+=(-p "$2"); shift ;;
    --all) gate=(); bench+=(--all) ;;
    --base) base_rev=$2; shift ;;
    --lang|--rounds) bench+=("$1" "$2"); shift ;;
    *) bench+=("$1") ;;
  esac; shift
done
[ ${#pkg[@]} -gt 0 ] && gate=("${pkg[@]}")
# The cheap checks agents most often miss fail before the wait for a seat and the builds.
cargo fmt --all -- --check >/dev/null || { echo "FINISH FAILED: gate (cargo fmt --all)"; exit 1; }
. scripts/slot-run.sh; claim_slot; touch_changed; sweep_slot || exit 1
start=$(date -u +%FT%TZ); take_seat; waited=$SECONDS
# Prints the seat wait and run time, and appends them to finish-times.jsonl next to the slots.
timed() {
  local ran=$((SECONDS - waited))
  echo "finish: waited ${waited}s for a seat, ran ${ran}s"
  [ -n "${CARGO_TARGET_DIR:-}" ] || return 0
  printf '{"start":"%s","slot":"%s","seats":%s,"wait_s":%s,"run_s":%s,"result":"%s"}\n' \
    "$start" "$slot" "${SUDSY_FINISH_SEATS:-1}" "$waited" "$ran" "$1" >>"$(dirname "$CARGO_TARGET_DIR")/finish-times.jsonl"
}
# codegen --check builds xtask, which the gate's selection needs anyway.
cargo xtask codegen --check || { timed codegen; echo "FINISH FAILED: gate (stale generated files: cargo xtask codegen)"; exit 1; }

# --- snapshot of the working tree, pushed and benched in CI -------------------------------------
head=$(git rev-parse HEAD)
dirty=0; [ -n "$(git status --porcelain --untracked-files=no)" ] && dirty=1
if [ -z "$base_rev" ]; then
  base_rev=$(git merge-base HEAD origin/main 2>/dev/null || git rev-parse HEAD^)
  if [ "$(git rev-parse "$base_rev")" = "$head" ] && [ $dirty = 0 ]; then base_rev=$(git rev-parse HEAD^); fi
fi
base=$(git rev-parse "$base_rev")
# A temporary index: the branch and the real index stay as they are, untracked files are included.
idx=${CARGO_TARGET_DIR:-.cargo/tmp}/finish-index; mkdir -p "$(dirname "$idx")"; rm -f "$idx"
snap=$(GIT_INDEX_FILE=$idx git add -A -- . ':!.cargo/tmp' && GIT_INDEX_FILE=$idx git write-tree) || { echo "FINISH FAILED: snapshot"; exit 1; }
snap=$(git commit-tree "$snap" -p "$head" -m "finish snapshot of $slot") || { echo "FINISH FAILED: snapshot"; exit 1; }
rm -f "$idx"
branch=bench/$slot
git push -q --force origin "$snap:refs/heads/$branch" || { echo "FINISH FAILED: bench (push of $branch)"; exit 1; }
token=$slot-${snap:0:9}-$(date +%s)
id=
cleanup() { git push -q origin --delete "$branch" >/dev/null 2>&1; }
trap 'rm -f "${SEAT:-}"; cleanup' EXIT
gh workflow run bench.yml --ref "$wf_ref" -f ref="$snap" -f base="$base" -f args="${bench[*]}" -f token="$token" \
  -f head="$head" -f dirty=$dirty >/dev/null || { echo "FINISH FAILED: bench (dispatch)"; exit 1; }
for _ in $(seq 1 40); do
  id=$(gh run list --workflow bench.yml --limit 30 --json databaseId,displayTitle \
    --jq ".[] | select(.displayTitle == \"bench $token\") | .databaseId" | head -1)
  [ -n "$id" ] && break; sleep 3
done
[ -n "$id" ] || { echo "FINISH FAILED: bench (no run found for $token)"; exit 1; }
echo "bench: $(gh run view "$id" --json url --jq .url)"

scripts/gate.sh "${gate[@]}"
if [ $? != 0 ]; then
  gh run cancel "$id" >/dev/null 2>&1
  timed gate; echo "FINISH FAILED: gate (bench stopped)"
  exit 1
fi
gh run watch "$id" --exit-status >/dev/null 2>&1
status=$?
out=${CARGO_TARGET_DIR:-.cargo/tmp}/finish-result; rm -rf "$out"
echo "== bench"
if gh run download "$id" -n bench-result -D "$out" >/dev/null 2>&1; then
  # The artifact keeps the build output; print the languages and the comparison.
  awk 'NR == 1 || /^languages/ { print } /^metric/ { p = 1 } p' "$out/bench-output.txt"
  cat "$out/bench-line.jsonl" >>bench/history.jsonl 2>/dev/null
else
  echo "no bench result: gh run view $id --log-failed"; [ $status != 0 ] || status=1
fi
if [ $status = 0 ]; then timed ok; echo "FINISH OK"; else timed bench; echo "FINISH FAILED: bench"; fi
exit $status

#!/usr/bin/env bash
# The check of a finished change (CLAUDE.md), run on GitHub Actions (finish.yml): the gate on Linux
# and on Windows and the bench, as parallel jobs, all on what the change affects (`cargo xtask
# affected`; --all: everything). Nothing builds locally. The working tree (untracked files
# included) is pushed as a snapshot commit to bench/<slot>, checked there against the merge-base
# with origin/main, and the branch is deleted afterwards. A failing gate job cancels the run at
# once, since the change gets edited anyway, and its log is printed. Ends in FINISH OK or
# FINISH FAILED: <gate|bench>; exit 0 only when everything passes. On a finished bench its history
# line is appended to bench/history.jsonl. Arguments go to bench.sh (--lang FOLDER: measure those
# folders instead of the selection), except -p CRATE (repeatable), which the gate checks instead
# of the selection. SUDSY_FINISH_REF: the ref whose finish.yml is dispatched (default main; a
# branch to try a changed workflow). `finish.sh --stop` cancels this slot's run.
set -uo pipefail
cd "$(dirname "$0")/.."
slot=$(basename "${CARGO_TARGET_DIR:-$PWD}")
wf_ref=${SUDSY_FINISH_REF:-main}
gdir=$(git rev-parse --git-dir)
# The CI run of this slot: its title is `finish <slot>-<snapshot>-<epoch>`.
slot_runs() { gh run list --workflow finish.yml --limit 30 --json databaseId,displayTitle,status \
  --jq ".[] | select(.displayTitle | startswith(\"finish $slot-\")) | select(.status != \"completed\") | .databaseId"; }
if [ "${1:-}" = --stop ]; then
  for r in $(slot_runs); do gh run cancel "$r"; done; echo "finish stopped"; exit 0
fi
gate=--affected bench=() pkg= base_rev=
while [ $# -gt 0 ]; do
  case "$1" in
    -p) pkg="$pkg -p $2"; shift ;;
    --all) gate= bench+=(--all) ;;
    --base) base_rev=$2; shift ;;
    --lang|--rounds) bench+=("$1" "$2"); shift ;;
    *) bench+=("$1") ;;
  esac; shift
done
[ -z "$pkg" ] || gate=$pkg
# The cheapest check agents most often miss fails before the push.
cargo fmt --all -- --check >/dev/null || { echo "FINISH FAILED: gate (cargo fmt --all)"; exit 1; }

# --- snapshot of the working tree, pushed and checked in CI --------------------------------------
head=$(git rev-parse HEAD)
dirty=0; [ -n "$(git status --porcelain --untracked-files=no)" ] && dirty=1
if [ -z "$base_rev" ]; then
  base_rev=$(git merge-base HEAD origin/main 2>/dev/null || git rev-parse HEAD^)
  if [ "$(git rev-parse "$base_rev")" = "$head" ] && [ $dirty = 0 ]; then base_rev=$(git rev-parse HEAD^); fi
fi
base=$(git rev-parse "$base_rev")
# A temporary index: the branch and the real index stay as they are, untracked files are included.
# It starts from HEAD, which keeps the file modes (a Windows checkout has no executable bit).
idx=$gdir/finish-index.$$; rm -f "$idx"; export GIT_INDEX_FILE=$idx
snap=$(git read-tree HEAD && git add -A -- . ':!.cargo/tmp' && git write-tree) || { echo "FINISH FAILED: snapshot"; exit 1; }
snap=$(git commit-tree "$snap" -p "$head" -m "finish snapshot of $slot") || { echo "FINISH FAILED: snapshot"; exit 1; }
rm -f "$idx"; unset GIT_INDEX_FILE
branch=bench/$slot
trap 'git push -q origin --delete "$branch" >/dev/null 2>&1' EXIT
push() { git push -q --force origin "$snap:refs/heads/$branch"; }
# GitHub sometimes rejects a push with a transient ref-update error: one retry.
push || { sleep 5; push; } || { echo "FINISH FAILED: bench (push of $branch)"; exit 1; }
token=$slot-${snap:0:9}-$(date +%s)
gh workflow run finish.yml --ref "$wf_ref" -f ref="$snap" -f base="$base" -f args="${bench[*]}" -f gate=1 -f gate_args="$gate" \
  -f token="$token" -f head="$head" -f dirty=$dirty >/dev/null || { echo "FINISH FAILED: bench (dispatch)"; exit 1; }
id=
for _ in $(seq 1 40); do
  id=$(gh run list --workflow finish.yml --limit 30 --json databaseId,displayTitle \
    --jq ".[] | select(.displayTitle == \"finish $token\") | .databaseId" | head -1)
  [ -n "$id" ] && break; sleep 3
done
[ -n "$id" ] || { echo "FINISH FAILED: bench (no run found for $token)"; exit 1; }
echo "finish: $(gh run view "$id" --json url --jq .url)"

# --- wait: a failed gate job stops the run; otherwise the run's end decides -------------------------
while :; do
  IFS=$'\t' read -r st failed < <(gh run view "$id" --json status,jobs \
    --jq '[.status, ([.jobs[] | select(.name | startswith("gate")) | select(.conclusion == "failure") | .name] | join(","))] | @tsv')
  [ -z "${failed:-}" ] || break
  [ "$st" = completed ] && break
  sleep 15
done
if [ -n "${failed:-}" ]; then
  gh run cancel "$id" >/dev/null 2>&1
  for _ in $(seq 1 20); do [ "$(gh run view "$id" --json status --jq .status)" = completed ] && break; sleep 3; done
  # The failed log without the job and step columns, timestamps and colors.
  gh run view "$id" --log-failed 2>&1 | sed -E 's/\[[0-9;]*m//g; s/^[^	]*	[^	]*	[^ ]* //' | grep -v '^$' | tail -n 120
  echo "FINISH FAILED: gate ($failed; bench stopped)"
  exit 1
fi
conclusion=$(gh run view "$id" --json conclusion --jq .conclusion)
out=$gdir/finish-result; rm -rf "$out"
echo "== bench"
fetch() { rm -rf "$out"; gh run download "$id" -n bench-result -D "$out" >/dev/null 2>&1; }
if fetch || { sleep 20; fetch; } || { sleep 60; fetch; }; then
  # The artifact keeps the build output; print the languages and the comparison.
  awk 'NR == 1 || /^languages/ { print } /^metric/ { p = 1 } p' "$out/bench-output.txt"
  cat "$out/bench-line.jsonl" >>bench/history.jsonl 2>/dev/null
elif [ "$conclusion" = success ]; then
  # A pass without its history line is not a finished bench: the commit would lack the line.
  echo "FINISH FAILED: bench (result not downloaded: gh run download $id -n bench-result, then append bench-line.jsonl to bench/history.jsonl)"
  exit 1
else
  echo "no bench result: gh run view $id --log-failed"
fi
if [ "$conclusion" = success ]; then echo "FINISH OK"; exit 0; fi
echo "FINISH FAILED: bench ($conclusion: gh run view $id)"; exit 1

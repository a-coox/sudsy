# Sourced by gate.sh and finish.sh. A slot serves one run at a time, but under Git Bash a cancelled
# background run (TaskStop kills only its top process) leaves its shells, cargo and tests running,
# and they hold the slot's build lock. So a new top-level run stops what an earlier run in the same
# slot left: every process carrying the slot's run marker (SUDSY_SLOT_RUN, inherited by all a run
# starts) that is not this run or its ancestors, with the Windows process tree under it.
claim_slot() {
  case "$(uname -s)" in MINGW*|MSYS*) ;; *) return 0 ;; esac
  [ -n "${CARGO_TARGET_DIR:-}" ] && [ -z "${SUDSY_SLOT_RUN:-}" ] || return 0
  local mine=" $$ " p=$$ old= d
  while p=$(cat "/proc/$p/ppid" 2>/dev/null) && [ "$p" -gt 1 ]; do mine="$mine$p "; done
  for d in /proc/[0-9]*; do
    p=${d#/proc/}
    case "$mine" in *" $p "*) continue ;; esac
    tr '\0' '\n' <"$d/environ" 2>/dev/null | grep -qxF "SUDSY_SLOT_RUN=$CARGO_TARGET_DIR" && old="$old $p"
  done
  if [ -n "$old" ]; then
    echo "stopping what an earlier run left in this slot (pids$old)"
    for p in $old; do
      taskkill //F //T //PID "$(cat "/proc/$p/winpid" 2>/dev/null)" >/dev/null 2>&1
      kill -9 "$p" 2>/dev/null
    done
    scripts/bench.sh --stop >/dev/null
  fi
  export SUDSY_SLOT_RUN=$CARGO_TARGET_DIR
}

# Machine-wide queue for finish runs: at most SUDSY_FINISH_SEATS (default 2) build at once. One file
# per run in <slot parent>/locks, named <start-ns>-<slot>, holding the run's pid; the first N live
# files hold the seats, so there is no mutex to get stuck. Files of dead pids are reclaimed.
# Call after claim_slot (which stopped what this slot's earlier run left).
take_seat() {
  [ -n "${CARGO_TARGET_DIR:-}" ] || return 0
  local dir seats=${SUDSY_FINISH_SEATS:-2} f n said= ahead
  dir=$(dirname "$CARGO_TARGET_DIR")/locks; mkdir -p "$dir"
  SEAT=$dir/$(date +%s%N)-$(basename "$CARGO_TARGET_DIR"); echo $BASHPID >"$SEAT"
  trap 'rm -f "$SEAT"' EXIT
  while :; do
    n=0; ahead=
    for f in "$dir"/*; do
      [ "$f" = "$SEAT" ] && break
      [ -e "/proc/$(cat "$f" 2>/dev/null)" ] || { rm -f "$f"; continue; }
      n=$((n + 1)); ahead="$ahead $(basename "$f" | cut -d- -f2-)"
    done
    [ $n -lt "$seats" ] && return 0
    [ -n "$said" ] || echo "finish: waiting for a seat ($seats at once), ahead:$ahead"; said=1
    sleep 3
  done
}

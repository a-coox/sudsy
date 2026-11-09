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

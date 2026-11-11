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
    tr '\0' '\n' 2>/dev/null <"$d/environ" | grep -qxF "SUDSY_SLOT_RUN=$CARGO_TARGET_DIR" && old="$old $p"
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

# A slot's builds serve every worktree that uses it, and cargo judges a source file by its mtime
# against the outputs, not by its content: a file this worktree holds unchanged since before another
# worktree's last build in the slot passes as built, though that build compiled the other's version.
# So when the slot changes worktrees, the files that differ from what it last built (the other's
# HEAD and its uncommitted files) get the current time. A slot with no record: every tracked file.
# The record is <slot>/.built-from: "<worktree> <HEAD>", then the worktree's uncommitted files.
touch_changed() {
  [ -n "${CARGO_TARGET_DIR:-}" ] || return 0
  mkdir -p "$CARGO_TARGET_DIR"
  local rec=$CARGO_TARGET_DIR/.built-from wt= head=
  dirty() { git ls-files -mo --exclude-standard -- . ':!.cargo/tmp'; }
  [ -f "$rec" ] && read -r wt head <"$rec"
  if [ "$wt" != "$PWD" ]; then
    { if [ -n "$head" ] && git cat-file -e "$head" 2>/dev/null; then git diff --name-only "$head"; else git ls-files; fi
      [ ! -f "$rec" ] || tail -n +2 "$rec"; dirty; } | sort -u | xargs -r -d '\n' touch -c 2>/dev/null || true
  fi
  { echo "$PWD $(git rev-parse HEAD)"; dirty; } >"$rec"
}

# cargo never deletes outputs: every dependency graph (each Cargo.lock change) and build kind leaves
# a full set behind. At most once an hour, before any build, the slot is cut to SUDSY_SLOT_MAX
# (default 120GB) and its WSL bench builds to SUDSY_SLOT_WSL_MAX (30GB) by cargo-sweep, which
# removes the outputs builds read least recently (the access time of cargo's fingerprint files,
# read for every unit a build uses), so what current builds reuse stays. Incremental caches, which
# it does not track, go when nothing compiled their crate for 3 days.
sweep_slot() {
  [ -n "${CARGO_TARGET_DIR:-}" ] && [ -d "$CARGO_TARGET_DIR" ] || return 0
  [ -n "$(find "$CARGO_TARGET_DIR/.swept" -mmin -60 2>/dev/null)" ] && return 0
  command -v cargo-sweep >/dev/null || { echo "cargo-sweep missing: cargo install cargo-sweep --locked (also in WSL)" >&2; return 1; }
  local t=$SECONDS log=$CARGO_TARGET_DIR/sweep.log
  local i; : >"$log"
  for i in $(find "$CARGO_TARGET_DIR" -maxdepth 4 -type d -name incremental); do
    find "$i" -mindepth 1 -maxdepth 1 -type d -mtime +3 -exec rm -rf {} + >>"$log" 2>&1
  done
  cargo sweep --maxsize "${SUDSY_SLOT_MAX:-120GB}" . >>"$log" 2>&1 || { echo "cargo sweep failed: $log" >&2; return 1; }
  case "$(uname -s)" in MINGW*|MSYS*)
    local p drive; p=$(cygpath -m "$PWD"); drive=${p:0:1}; drive=${drive,,}
    MSYS_NO_PATHCONV=1 wsl -d "${BENCH_WSL_DISTRO:-Ubuntu}" -e bash -c "export PATH=\$HOME/.cargo/bin:\$PATH
      d=\$HOME/.cache/sudsy-bench/$(basename "$CARGO_TARGET_DIR"); [ -d \$d ] || exit 0
      command -v cargo-sweep >/dev/null || { echo 'cargo-sweep missing in WSL: cargo install cargo-sweep --locked' >&2; exit 1; }
      for i in \$(find \$d -maxdepth 4 -type d -name incremental); do find \$i -mindepth 1 -maxdepth 1 -type d -mtime +3 -exec rm -rf {} +; done
      cd /mnt/$drive${p:2} && CARGO_TARGET_DIR=\$d cargo sweep --maxsize ${SUDSY_SLOT_WSL_MAX:-30GB} ." >>"$log" 2>&1 \
      || { echo "cargo sweep in WSL failed: $log" >&2; return 1; } ;;
  esac
  touch "$CARGO_TARGET_DIR/.swept"
  echo "   (slot sweep: $((SECONDS - t))s)"
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

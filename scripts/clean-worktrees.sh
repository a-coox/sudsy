#!/usr/bin/env bash
# Removes finished agent worktrees under .claude/worktrees/ and their branches.
# Usage: scripts/clean-worktrees.sh [--dry-run] [--idle MINUTES]  (default idle: 60)
# A worktree goes when its branch is merged into origin/main, it has no changes (tracked or
# untracked), and its agent is done: the pid in its lock is gone, or the agent's transcript has
# not been written for --idle minutes (a lead session keeps its pid, and the lock, for days).
# Only plain `git worktree remove` and `git branch -d`: nothing unmerged or changed is touched.
# Then deletes folders under .claude/worktrees/ that git no longer tracks (left behind when
# Windows refused to delete a folder a process still had as its working directory).
set -u
dry=0; idle=60
while [ $# -gt 0 ]; do
  case "$1" in --dry-run) dry=1 ;; --idle) idle=$2; shift ;; *) sed -n '2,9p' "$0" >&2; exit 2 ;; esac
  shift
done
# The main checkout: the first worktree git lists, wherever this runs from.
root=$(git worktree list --porcelain | sed -n '1s/^worktree //p'); [ -n "$root" ] || exit 1
cd "$root" || exit 1
git fetch -q origin main || { echo "fetch failed" >&2; exit 1; }
run() { if [ $dry = 1 ]; then echo "  would: $*"; else "$@"; fi; }
pid_alive() { tasklist //FI "PID eq $1" 2>/dev/null | grep -q " $1 " || kill -0 "$1" 2>/dev/null; }
removed=0; kept=0
# One record per worktree: path, branch, lock reason.
while IFS=$'\t' read -r path branch lock; do
  case "$path" in */.claude/worktrees/*) ;; *) continue ;; esac
  name=${path##*/}
  why=""
  if [ -z "$branch" ]; then why="detached"
  elif ! git merge-base --is-ancestor "$branch" origin/main 2>/dev/null; then why="not merged"
  elif [ -n "$(git -C "$path" status --porcelain 2>/dev/null | head -1)" ]; then why="has changes"
  elif [ -n "$lock" ]; then
    pid=$(echo "$lock" | sed -n 's/.*(pid \([0-9]*\)).*/\1/p')
    if [ -n "$pid" ] && pid_alive "$pid"; then
      t=$(ls "$HOME"/.claude/projects/*/*/subagents/"$name".jsonl 2>/dev/null | head -1)
      if [ -z "$t" ] || [ -n "$(find "$t" -mmin -"$idle" 2>/dev/null)" ]; then why="agent active"; fi
    fi
  fi
  if [ -n "$why" ]; then echo "keep   $name ($why)"; kept=$((kept + 1)); continue; fi
  echo "remove $name"
  [ -n "$lock" ] && run git worktree unlock "$path"
  run git worktree remove "$path" && run git branch -q -d "${branch#refs/heads/}"
  removed=$((removed + 1))
done < <(git worktree list --porcelain | awk '
  /^worktree /{ if (p != "") print p "\t" b "\t" l; p = substr($0, 10); b = ""; l = "" }
  /^branch /{ b = substr($0, 8) }
  /^locked/{ l = substr($0, 8); if (l == "") l = "locked" }
  END { if (p != "") print p "\t" b "\t" l }')
# Folders git does not track any more, untouched for 10 minutes (not a worktree being created).
tracked=$(git worktree list --porcelain | sed -n 's/^worktree //p')
stray=0
for d in .claude/worktrees/*/; do
  d=${d%/}; [ -d "$d" ] || continue
  echo "$tracked" | grep -qF "$root/$d" && continue
  [ -n "$(find "$d" -maxdepth 0 -mmin -10)" ] && continue
  echo "stray  ${d##*/}"
  run rm -rf "$d" || echo "  still held by a process: ${d##*/}"
  stray=$((stray + 1))
done
echo "worktrees removed $removed, kept $kept; stray folders $stray"

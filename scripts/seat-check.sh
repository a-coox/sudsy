#!/usr/bin/env bash
# Self-check of take_seat (slot-run.sh): 3 holders with 2 seats, the third waits until one exits;
# a holder killed with -9 gives up its seat.
set -u
cd "$(dirname "$0")/.."
. scripts/slot-run.sh
root=$(mkdir -p "${CARGO_TARGET_DIR:-target}" && mktemp -d "${CARGO_TARGET_DIR:-target}/seat.XXXXXX"); trap 'rm -rf "$root"' EXIT
export SUDSY_FINISH_SEATS=2
hold() { ( CARGO_TARGET_DIR=$root/$1; take_seat; echo "$1 seated"; echo $BASHPID >"$root/$1.pid"; sleep "$2" ) >"$root/$1.out" 2>&1 & }
fail() { echo "FAIL: $1"; exit 1; }
hold a 4; sleep 1; hold b 30; sleep 1; hold c 30; sleep 2
grep -q seated "$root/c.out" && fail "c seated early"
grep -q waiting "$root/c.out" || fail "c did not say it waits"
sleep 6; grep -q seated "$root/c.out" || fail "c not seated after a exited"
hold d 30; sleep 4; grep -q seated "$root/d.out" && fail "d seated with b,c live"
kill -9 "$(cat "$root/b.pid")"
sleep 5; grep -q seated "$root/d.out" || fail "stale seat not reclaimed"
echo "seat-check OK"; kill "$(cat "$root/c.pid")" "$(cat "$root/d.pid")" 2>/dev/null

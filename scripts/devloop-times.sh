#!/usr/bin/env bash
# Times the end-of-change commands after an edit (touching core-syntax), for the dev loop rows of
# docs/performance/numbers.md: gate, bench without a cached baseline, finish without and with one.
# Run in an idle slot: CARGO_TARGET_DIR=<slot> scripts/devloop-times.sh
set -uo pipefail
cd "$(dirname "$0")/.."
run() { local t=$SECONDS; touch crates/core-syntax/src/lib.rs; "$@" >/dev/null 2>&1; echo "$*: exit $? in $((SECONDS - t))s"; }
uncache() { rm -rf "${CARGO_TARGET_DIR:-target}/bench/base-cache"; }
run scripts/gate.sh
uncache; run scripts/bench.sh --no-record
uncache; run scripts/finish.sh --no-record
run scripts/finish.sh --no-record

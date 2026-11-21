#!/usr/bin/env bash
# Train 0.5.3: CI's test job on macOS (nextest, the engine without the desktop feature, FFI tests, clippy).
set -uo pipefail
step() { local name=$1; shift; echo "== $name"; "$@" > "/tmp/$SLOT_JOB-$name.log" 2>&1; local rc=$?
  grep -E '^(error|warning: unused)|FAILED|panicked|FAIL \[|Summary|test result: FAILED' "/tmp/$SLOT_JOB-$name.log" | head -25
  [ $rc -ne 0 ] && tail -15 "/tmp/$SLOT_JOB-$name.log"; echo "STEP $name rc=$rc"; }
command -v cargo-nextest >/dev/null || cargo install --locked cargo-nextest >/dev/null 2>&1
step nextest cargo nextest run --workspace --profile ci
step nextest-engine-nodesktop cargo nextest run -p kaeri-engine --no-default-features --lib --profile ci
step ffi-tests cargo test -p kaeri-ffi --features testing,cli
step clippy cargo clippy --workspace --all-targets -- -D warnings

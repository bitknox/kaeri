#!/usr/bin/env bash
# Train 0.5.3: CI's desktop-linux job.
set -uo pipefail
cd desktop/linux
step() { local name=$1; shift; echo "== $name"; "$@" > "/tmp/$SLOT_JOB-$name.log" 2>&1; local rc=$?
  grep -E '^(error|warning: unused)|FAILED|panicked|test result:' "/tmp/$SLOT_JOB-$name.log" | head -25
  [ $rc -ne 0 ] && tail -15 "/tmp/$SLOT_JOB-$name.log"; echo "STEP $name rc=$rc"; }
step test-fixtures xvfb-run -a cargo test --locked --features fixtures
step test xvfb-run -a cargo test --locked
step clippy-fixtures cargo clippy --locked --all-targets --features fixtures -- -D warnings
step clippy cargo clippy --locked --all-targets -- -D warnings
step fmt cargo fmt --check

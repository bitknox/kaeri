#!/usr/bin/env bash
# The reader's stuck-state checks (ReaderStuckTests in the Kaeri Audit scheme) on iPad Simulators
# made for the run, the way to run them by hand.
#
#   apple/scripts/audit-reader.sh            the whole class
#   apple/scripts/audit-reader.sh <test>…    the tests named, such as testGoToPageByTouch
#
# The Go to page touch test needs a Simulator no hardware key press has reached: once one has,
# its number pad stays below the screen. So that test runs on an iPad Simulator of its own, made
# fresh, booted and settled: the Fixtures app launched once on the test's scene until it prints
# KAERI_READY, then closed. A first launch on a Simulator that has just booted is slow enough
# under load that the reader's first page misses the test's wait. The other tests run together on
# a second fresh iPad Simulator, settled the same way first. Both Simulators are deleted at the
# end, and no Simulator or system setting is changed.
#
# The pinch test gets its hand from outside here: when it writes its mark, the script brings the
# Simulator's Settings app to the front while the fingers are down, as the test asks.
#
# KAERI_DERIVED_DATA picks where xcodebuild builds, apple/.derived unless it says otherwise.
# Exits non-zero when a test failed or did not run, and prints the failures.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2.
shopt -s inherit_errexit 2>/dev/null || true

apple="$(cd "$(dirname "$0")/.." && pwd)"
dd="${KAERI_DERIVED_DATA:-$apple/.derived}"
type=com.apple.CoreSimulator.SimDeviceType.iPad-Pro-11-inch-M5-12GB
touch=testGoToPageByTouch
scene=readermangartl

others=()
touched=0
if (( $# )); then
  for name in "$@"; do
    if [[ "$name" == "$touch" ]]; then touched=1; else others+=("/$name"); fi
  done
else
  others=("")
  touched=1
fi

work="$(mktemp -d -t kaeri-audit-reader)"
sims=()
helper=
cleanup() {
  [[ -n "$helper" ]] && kill "$helper" 2>/dev/null
  local udid
  for udid in ${sims[@]+"${sims[@]}"}; do
    xcrun simctl shutdown "$udid" 2>/dev/null
    xcrun simctl delete "$udid" 2>/dev/null
  done
  rm -rf "$work"
  return 0
}
trap cleanup EXIT

"$apple/scripts/engine.sh" testing ios-sim
"$apple/scripts/mpv.sh" ios-sim
"$apple/scripts/fonts.sh"
(cd "$apple" && xcodegen generate --quiet)

runtime="$(xcrun simctl list runtimes -j | TYPE="$type" /usr/bin/python3 -I -c '
import json, os, sys
ios = [r for r in json.load(sys.stdin)["runtimes"] if r["platform"] == "iOS" and r["isAvailable"]
       and any(d["identifier"] == os.environ["TYPE"] for d in r.get("supportedDeviceTypes", []))]
ios.sort(key=lambda r: [int(p) for p in r["version"].split(".")])
print(ios[-1]["identifier"] if ios else "")')"
[[ -n "$runtime" ]] || {
  echo "audit-reader.sh: no iOS Simulator runtime has $type; xcodebuild -downloadPlatform iOS" >&2
  exit 1
}

# A fresh iPad Simulator, booted; its udid goes in `made`.
made=
fresh() {
  made="$(xcrun simctl create "Kaeri audit iPad" "$type" "$runtime")"
  sims+=("$made")
  xcrun simctl boot "$made"
  xcrun simctl bootstatus "$made" -b >/dev/null
}

# settle UDID: the Fixtures app launched once on the scene until it prints KAERI_READY, which it
# does once the reader is staged, then closed.
settle() {
  local udid=$1 console start=$SECONDS
  echo "audit-reader.sh: settling $udid"
  xcrun simctl install "$udid" "$dd/Build/Products/Debug-iphonesimulator/Kaeri Fixtures.app"
  xcrun simctl launch --console-pty --terminate-running-process "$udid" app.kaeri.fixtures \
    -KaeriScene "$scene" >"$work/settle-$udid.log" 2>&1 &
  console=$!
  until grep -q KAERI_READY "$work/settle-$udid.log"; do
    if (( SECONDS - start > 600 )); then
      echo "audit-reader.sh: the Fixtures app did not say it was ready on $udid" >&2
      tail -n 20 "$work/settle-$udid.log" >&2
      exit 1
    fi
    sleep 0.5
  done
  xcrun simctl terminate "$udid" app.kaeri.fixtures 2>/dev/null || true
  wait "$console" 2>/dev/null || true
  echo "audit-reader.sh: settled in $((SECONDS - start)) s"
}

fresh
main=$made
echo "audit-reader.sh: building on $main"
if ! xcodebuild -quiet -project "$apple/Kaeri.xcodeproj" -scheme "Kaeri Audit" -destination "id=$main" \
  -derivedDataPath "$dd" build-for-testing >"$work/build.log" 2>&1; then
  { grep -E "error:" "$work/build.log" || tail -n 20 "$work/build.log"; } | tail -n 20 >&2
  exit 1
fi

mkdir -p "$work/marks"
(
  while true; do
    if [[ -f "$work/marks/pinching" ]]; then
      rm -f "$work/marks/pinching"
      sleep 2.5
      xcrun simctl launch "$main" com.apple.Preferences >/dev/null 2>&1 || true
    fi
    sleep 0.2
  done
) &
helper=$!

status=0
# run NAME UDID ARGS…: the tests on that Simulator; prints how they went.
run() {
  local name=$1 udid=$2 rc=0
  shift 2
  TEST_RUNNER_KAERI_TEST_MARKS="$work/marks" xcodebuild -project "$apple/Kaeri.xcodeproj" -scheme "Kaeri Audit" \
    -destination "id=$udid" -derivedDataPath "$dd" test-without-building "$@" >"$work/$name.log" 2>&1 || rc=$?
  if ! grep -qE "Executed [0-9]+ tests?," "$work/$name.log"; then
    echo "audit-reader.sh: $name did not run the tests" >&2
    tail -n 20 "$work/$name.log" >&2
    status=1
    return
  fi
  grep -E "Test Case .*(passed|failed|skipped)" "$work/$name.log" |
    sed -E "s/^.*ReaderStuckTests ([A-Za-z0-9_]+)\]' (passed|failed|skipped).*/  \1 \2/" || true
  if (( rc )); then
    grep -E "error: -\[" "$work/$name.log" | sed -e 's/^.*ReaderStuckTests\.swift:/  ReaderStuckTests.swift:/' >&2 || true
    status=1
  fi
}

if (( ${#others[@]} )); then
  only=()
  for name in "${others[@]}"; do only+=("-only-testing:KaeriAuditTests/ReaderStuckTests$name"); done
  settle "$main"
  echo "audit-reader.sh: the tests on $main"
  run rest "$main" "${only[@]}" "-skip-testing:KaeriAuditTests/ReaderStuckTests/$touch"
fi

if (( touched )); then
  fresh
  settle "$made"
  echo "audit-reader.sh: $touch on $made"
  run touch "$made" "-only-testing:KaeriAuditTests/ReaderStuckTests/$touch"
fi

exit "$status"

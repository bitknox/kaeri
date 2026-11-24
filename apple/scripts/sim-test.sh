#!/usr/bin/env bash
# UI tests of the Fixtures app on an iPhone and an iPad Simulator of the run's own: built once,
# then run on each form in turn, with each form's result bundle, attachments and log in the out
# folder.
#
#   apple/scripts/sim-test.sh <scheme> [--forms iphone,ipad] [--out <dir>] [--landscape]
#     [-- <xcodebuild test options, such as -only-testing:KaeriPlayerFlows/SourcesLoadingFlows>]
#
# The scheme's own pre-actions build the testing engine. The Simulators are made for the run,
# dressed as the shots scripts dress theirs (dark, 9:41, the iPad's apps full screen) and deleted
# after, whatever happens. TEST_RUNNER_<NAME> variables reach the tests as <NAME>, as with
# xcodebuild itself; KAERI_HOME is given a scratch folder per form under the out folder unless it
# is set.
#
# Xcode collects a sysdiagnose and log archive from the Simulator after a test run, which hung for
# 600 s after the tests ended in most runs on this Mac. Each run passes
# -collect-test-diagnostics never, and as a guard an xcodebuild still running SIM_TEST_GRACE
# seconds (30) after the run's last result line, with its result bundle complete, is ended and
# judged by that bundle.
#
# Prints one line per step, "STEP <name> rc=<n>", and exits non-zero when any step failed.
set -uo pipefail

usage() {
  echo "usage: apple/scripts/sim-test.sh <scheme> [--forms iphone,ipad] [--out <dir>] [--landscape] [-- <xcodebuild options>]" >&2
  exit 2
}

scheme="${1:-}"
[[ -n "$scheme" && "$scheme" != -* ]] || usage
shift
forms=(iphone ipad)
apple="$(cd "$(dirname "$0")/.." && pwd)"
out="$apple/.shots/sim-test"
landscape=0
while (( $# )); do
  case "$1" in
    --forms) IFS=, read -r -a forms <<<"${2:-}"; shift 2 ;;
    --out) out="${2:-}"; shift 2 ;;
    --landscape) landscape=1; shift ;;
    --) shift; break ;;
    *) usage ;;
  esac
done
extra=("$@")
grace="${SIM_TEST_GRACE:-30}"
mkdir -p "$out"
out="$(cd "$out" && pwd)"
derived="${KAERI_DERIVED_DATA:-$out/derived}"
status=0
step() { echo "STEP $1 rc=$2"; (( $2 == 0 )) || status=1; }
since() { echo "$(( $(date +%s) - $1 )) s"; }

made=()
cleanup() {
  local udid
  for udid in ${made[@]+"${made[@]}"}; do
    xcrun simctl shutdown "$udid" >/dev/null 2>&1 || true
    xcrun simctl delete "$udid" >/dev/null 2>&1 || true
  done
}
trap cleanup EXIT

runtime="$(xcrun simctl list runtimes -j | /usr/bin/python3 -I -c '
import json, sys
ios = [r for r in json.load(sys.stdin)["runtimes"] if r["platform"] == "iOS" and r["isAvailable"]]
ios.sort(key=lambda r: [int(p) for p in r["version"].split(".")])
print(ios[-1]["identifier"] if ios else "")
')"
[[ -n "$runtime" ]] || { echo "sim-test.sh: no iOS Simulator runtime; xcodebuild -downloadPlatform iOS" >&2; exit 1; }

# A Simulator for <form>, booted and dressed; its udid on stdout.
simulator() {
  local form="$1" type udid
  case "$form" in
    iphone) type=com.apple.CoreSimulator.SimDeviceType.iPhone-17 ;;
    ipad) type=com.apple.CoreSimulator.SimDeviceType.iPad-Pro-11-inch-M5-12GB ;;
    *) echo "sim-test.sh: no form $form (iphone or ipad)" >&2; return 1 ;;
  esac
  udid="$(xcrun simctl create "sim-test $form $$" "$type" "$runtime")" || return 1
  echo "$udid"
  xcrun simctl boot "$udid" >&2 || return 1
  xcrun simctl bootstatus "$udid" -b >/dev/null || return 1
  if [[ "$form" == ipad ]]; then
    # The iPad's apps full screen: neither Stage Manager nor windowed apps. SpringBoard reads
    # them as it starts.
    xcrun simctl spawn "$udid" defaults write com.apple.springboard SBChamoisWindowingEnabled -bool NO
    xcrun simctl spawn "$udid" defaults write com.apple.springboard SBMedusaMultitaskingEnabled -bool NO
    xcrun simctl shutdown "$udid" >&2
    xcrun simctl boot "$udid" >&2 || return 1
    xcrun simctl bootstatus "$udid" -b >/dev/null || return 1
  fi
  xcrun simctl ui "$udid" appearance "${KAERI_APPEARANCE:-dark}"
  xcrun simctl status_bar "$udid" override --time 9:41 --dataNetwork wifi --wifiMode active \
    --wifiBars 3 --cellularMode active --cellularBars 4 --operatorName '' \
    --batteryState discharging --batteryLevel 100
  if (( landscape )); then
    xcrun devicectl --quiet device orientation set --device "$udid" landscapeLeft >/dev/null 2>&1 || true
  fi
}

# Whether the result bundle at <path> is whole: xcresulttool reads its summary.
complete() { xcrun xcresulttool get test-results summary --path "$1" >/dev/null 2>&1; }

# The bundle's verdict: 0 when its tests passed, 1 otherwise.
verdict() {
  xcrun xcresulttool get test-results summary --path "$1" 2>/dev/null | /usr/bin/python3 -I -c '
import json, sys
summary = json.load(sys.stdin)
sys.exit(0 if summary.get("result") == "Passed" else 1)
'
}

# One test-without-building on <udid>, its log in <log> and bundle in <bundle>; the run's exit
# code, or the bundle's verdict when the guard ended it.
run_tests() {
  local udid="$1" log="$2" bundle="$3" pid rc ended=0 last=""
  rm -rf "$bundle"
  xcodebuild -project "$apple/Kaeri.xcodeproj" -scheme "$scheme" -destination "id=$udid" \
    -derivedDataPath "$derived" -resultBundlePath "$bundle" -collect-test-diagnostics never \
    test-without-building ${extra[@]+"${extra[@]}"} >"$log" 2>&1 &
  pid=$!
  while kill -0 "$pid" 2>/dev/null; do
    if [[ -z "$last" ]] && grep -qE "^Test Suite 'All tests' (passed|failed)|^\*\* TEST (SUCCEEDED|FAILED|EXECUTE FAILED)" "$log"; then
      last=$(date +%s)
    fi
    if [[ -n "$last" ]] && (( $(date +%s) - last >= grace )) && complete "$bundle"; then
      echo "sim-test.sh: xcodebuild still running $grace s after the tests ended; ending it" >&2
      kill "$pid" 2>/dev/null
      ended=1
      break
    fi
    sleep 1
  done
  wait "$pid"
  rc=$?
  if (( ended )); then verdict "$bundle"; rc=$?; fi
  return "$rc"
}

# The bundle's attachments, by their names, beside it: <out>/<form>/<name>.<ext>.
attachments() {
  local bundle="$1" dir="$2"
  rm -rf "$dir/.export"
  mkdir -p "$dir/.export"
  xcrun xcresulttool export attachments --path "$bundle" --output-path "$dir/.export" >/dev/null 2>&1 || return 0
  /usr/bin/python3 -I - "$dir/.export" "$dir" <<'PY'
import json, os, shutil, sys
src, out = sys.argv[1], sys.argv[2]
for test in json.load(open(os.path.join(src, "manifest.json"))):
    for a in test.get("attachments", []):
        name = a.get("suggestedHumanReadableName") or a["exportedFileName"]
        ext = os.path.splitext(a["exportedFileName"])[1]
        stem = os.path.splitext(name)[0] if name.endswith(ext) else name
        # XCTest adds "_<n>_<uuid>" to a name; the name the test gave comes first.
        stem = stem.split("_0_")[0]
        shutil.copy(os.path.join(src, a["exportedFileName"]), os.path.join(out, stem + ext))
PY
  rm -rf "$dir/.export"
}

# What the Fixtures app builds with beside the engine: libmpv for the Simulator, the player's
# media (the Playing scenes' episode) and the subtitle font.
"$apple/scripts/mpv.sh" ios-sim >"$out/prepare.log" 2>&1 &&
  "$apple/scripts/player-media.sh" >>"$out/prepare.log" 2>&1 &&
  "$apple/scripts/fonts.sh" >>"$out/prepare.log" 2>&1
rc=$?
(( rc == 0 )) || tail -n 20 "$out/prepare.log" >&2
step prepare "$rc"
(( rc == 0 )) || exit 1
(cd "$apple" && xcodegen generate --quiet)
rc=$?
step xcodegen "$rc"
(( rc == 0 )) || exit 1

started=$(date +%s)
first="$(simulator "${forms[0]}")"
rc=$?
[[ -n "$first" ]] && made+=("$first")
step "simulator-${forms[0]}" "$rc"
(( rc == 0 )) || exit 1
xcodebuild -project "$apple/Kaeri.xcodeproj" -scheme "$scheme" -destination "id=$first" \
  -derivedDataPath "$derived" build-for-testing >"$out/build.log" 2>&1
rc=$?
grep -E "error:" "$out/build.log" | head -20 >&2
step build-for-testing "$rc"
echo "sim-test.sh: built in $(since "$started")"
(( rc == 0 )) || exit 1

for form in "${forms[@]}"; do
  if [[ "$form" == "${forms[0]}" ]]; then
    udid="$first"
  else
    udid="$(simulator "$form")"
    rc=$?
    [[ -n "$udid" ]] && made+=("$udid")
    step "simulator-$form" "$rc"
    (( rc == 0 )) || continue
  fi
  mkdir -p "$out/$form"
  home_set=0
  [[ -n "${TEST_RUNNER_KAERI_HOME:-}" ]] && home_set=1
  (( home_set )) || export TEST_RUNNER_KAERI_HOME="$out/$form/home"
  form_started=$(date +%s)
  run_tests "$udid" "$out/xcodebuild-$form.log" "$out/$form.xcresult"
  rc=$?
  (( home_set )) || unset TEST_RUNNER_KAERI_HOME
  step "test-$form" "$rc"
  echo "sim-test.sh: $form ran in $(since "$form_started")"
  grep -E "^Test Case .*(passed|failed)|error: -\[" "$out/xcodebuild-$form.log" | head -20
  attachments "$out/$form.xcresult" "$out/$form"
  xcrun simctl shutdown "$udid" >/dev/null 2>&1 || true
done
echo "sim-test.sh: all forms in $(since "$started")"
exit "$status"

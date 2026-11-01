#!/usr/bin/env bash
# The app's own update, end to end on an emulator, with keys the test makes for itself and never
# the release key:
#
#   android/scripts/update-test.sh build    the test's two keys, and the app flavour in release
#                                           (R8) three times: 0.10.0 to install, and 0.10.1 and 0.10.2
#                                           to offer, all signed with the test key, and 0.10.1 signed
#                                           again with the other key
#   android/scripts/update-test.sh install  0.10.0 on the emulator, in place of any Kaeri there
#   android/scripts/update-test.sh serve [--slow]
#                                           the APKs to the emulator over plain HTTP on port 8471,
#                                           through adb reverse, until stopped; --slow sends 256 kB
#                                           a second, long enough to cancel
#   android/scripts/update-test.sh offer <update [0.10.1|0.10.2]|other-key|bad-checksum>
#                                           a release as the engine reports one, with its notice,
#                                           through the update test's receiver: 0.10.1 (or 0.10.2) as
#                                           built, 0.10.1 with the other key, or 0.10.1 with a
#                                           checksum it does not have
#
# Everything goes to android/build/update-test, which git ignores. These builds carry the update
# test's receiver and fetch over plain HTTP (KAERI_UPDATE_TEST_KEYSTORE), so they are never for
# real use, and they never read the release key. SERIAL names the emulator, emulator-5640 unless it
# says otherwise. KAERI_PLAYER_NATIVE=off leaves libmpv out, which the update does not need.
set -euo pipefail
shopt -s inherit_errexit 2>/dev/null || true

android="$(cd "$(dirname "$0")/.." && pwd)"
out="$android/build/update-test"
serial="${SERIAL:-emulator-5640}"
port=8471
password=update-test
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
export JAVA_HOME="${JAVA_HOME:-/Applications/Android Studio.app/Contents/jbr/Contents/Home}"
export ANDROID_HOME="$sdk"
adb() { "$sdk/platform-tools/adb" -s "$serial" "$@"; }
tools="$(ls -d "$sdk"/build-tools/*/ | sort -V | tail -1)"

usage() {
  sed -n '2,23p' "$0" >&2
  exit 2
}

key() {
  local name="$1"
  [[ -f "$out/$name.p12" ]] && return 0
  "$JAVA_HOME/bin/keytool" -genkeypair -keystore "$out/$name.p12" -storetype PKCS12 \
    -storepass "$password" -keypass "$password" -alias update-test -keyalg RSA -keysize 2048 \
    -validity 30 -dname "CN=kaeri update test $name" >/dev/null
}

build() {
  mkdir -p "$out"
  key test
  key other
  local version name code
  for version in 0.10.0:200000099 0.10.1:200000199 0.10.2:200000299; do
    name="${version%%:*}"
    code="${version##*:}"
    (cd "$android" && ./gradlew --console=plain --max-workers=3 --no-configuration-cache \
      -PKAERI_UPDATE_TEST_KEYSTORE="$out/test.p12" \
      -PKAERI_VERSION_NAME="$name" -PKAERI_VERSION_CODE="$code" :app:assembleAppRelease)
    cp "$android/app/build/outputs/apk/app/release/app-app-release.apk" "$out/kaeri-$name.apk"
  done
  "$tools/apksigner" sign --ks "$out/other.p12" --ks-pass "pass:$password" --key-pass "pass:$password" \
    --ks-key-alias update-test --out "$out/kaeri-0.10.1-other-key.apk" "$out/kaeri-0.10.1.apk"
  rm -f "$out"/*.idsig
  for apk in "$out"/kaeri-*.apk; do
    echo "$(basename "$apk"): $("$tools/apksigner" verify --print-certs "$apk" | sed -n 's/.*SHA-256 digest: //p' | head -1)"
  done
}

install() {
  adb uninstall app.kaeri.app >/dev/null 2>&1 || true
  adb install "$out/kaeri-0.10.0.apk"
  adb shell dumpsys package app.kaeri.app | grep -m1 versionCode
}

serve() {
  local slow=0
  [[ "${1:-}" == --slow ]] && slow=1
  adb reverse "tcp:$port" "tcp:$port"
  echo "update-test.sh: serving $out on 127.0.0.1:$port$( ((slow)) && echo ', slowly')"
  python3 -I - "$out" "$port" "$slow" <<'PY'
import http.server, os, sys, time
root, port, slow = sys.argv[1], int(sys.argv[2]), sys.argv[3] == "1"
class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *a, **k):
        super().__init__(*a, directory=root, **k)
    def copyfile(self, source, target):
        while chunk := source.read(64 * 1024):
            target.write(chunk)
            if slow:
                time.sleep(0.25)
http.server.ThreadingHTTPServer(("127.0.0.1", port), Handler).serve_forever()
PY
}

offer() {
  local apk sha version=0.10.1
  case "${1:-}" in
    update)
      version="${2:-0.10.1}"
      apk="kaeri-$version.apk"
      ;;
    other-key) apk=kaeri-0.10.1-other-key.apk ;;
    bad-checksum) apk=kaeri-0.10.1.apk ;;
    *) usage ;;
  esac
  sha="$(shasum -a 256 "$out/$apk" | cut -d' ' -f1)"
  [[ "$1" == bad-checksum ]] && sha="$(printf '0%.0s' {1..64})"
  adb shell am broadcast -n app.kaeri.app/.UpdateTestReceiver \
    --es version "$version" --es url "http://127.0.0.1:$port/$apk" --es sha256 "$sha" \
    --el size "$(stat -f %z "$out/$apk")" --es notes "'Test release $version'" --ez notify true
}

case "${1:-}" in
  build) build ;;
  install) install ;;
  serve) shift; serve "$@" ;;
  offer) shift; offer "$@" ;;
  *) usage ;;
esac

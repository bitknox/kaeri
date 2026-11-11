#!/usr/bin/env bash
# Screenshots of the Fixtures app, for comparing with the design boards.
#
#   apple/scripts/shots.sh <mac|iphone|ipad> [--landscape] [shot…]
#
# A shot is a scene, and optionally a screen: watching (Continue filled, signed in; the default),
# sourceless (watching with no source set up), unsure (watching with a source ahead that cannot
# choose 900001's match; sheet=match-<menu|results|empty|failed|picking|picked|cleared> stages the
# match sheet), storeowner (your plugin store with invites in use, used up and cancelled; sheet=members|invite), storeinvited (sheet=join-invited pastes its link), fresh (a first launch) or playing (the player's
# generated episode first on Continue), then after a colon a section (calendar, seasonal, search, library,
# downloads, providers, pluginStores, settings) or title=<id>, select=<id>, query=<words>,
# genre=<name> (Search browsing a genre), filters=set (Search with Fantasy, 2024, Spring, TV and two sheet filters set, on the fuller catalogue, which finds two titles), scroll=<points> (Search's results scrolled), available=<here|services> (Search with its Available filter set) or palette=<words>, with more joined by +: pane=<Settings
# pane>, notice=alert (a failure toast), sheet=<torrent|proxy|join|kitsu|update|show|about|report|welcome-sources|welcome-setup|filters|messages|discussion-<episode>>,
# prelaunch=<state> (a reading prelaunch state, see PrelaunchScene), sync=<state> (a Devices state or sync sheet, see SyncScene), tv=<paired|picker|code|expired|remote|lost|refused|empty|bar|away>, Play on TV with the Fixtures app's
# fake TVs (FixtureTVs.swift says what each shows), cast=<scene>, casting to the made-up TVs with
# Chromecast built in (FixtureCasts.swift lists the scenes), day=<days after today>, the calendar opened on
# that day, for one with broadcasts, mine=1 the calendar on My list,
# together=<state>, Watch together in a state TogetherScene (KaeriUI/TogetherShots.swift) lists, on
# the together scene for its recent rooms (together:together=lobby-filling),
# reader=<chrome|settings|chapters|ask|end|endskip|scrub|pill|plate|opening|error> on a reader scene (reader-manga-rtl, reader-strip, reader-novel,
# reader-moved), the reader open with its chrome or a sheet up,
# medium=<read|watch|both> on a reading scene, what Kaeri is for chosen at launch as Settings would,
# work=<id> a series' page by its work id (work=a:900301), reading=1 the Library and Search on their reading side (with filters=set, reading's Fantasy, Manga, Japan and Releasing),
# sheet=sources-<mid|failed|only|nothing|sourcehits|anilistfailed> on reading's Search with words (search+reading=1+query=lantern) stages its sources' part with made-up sources, sheet=read-filters there opens the iPhone Filters sheet, sheet=viewall on reading's Home opens its first row's View all,
# send=<preparing|waiting|failed> on a series' Send to e-reader sheet (sheet=send, work=<id>; sheet=stopped asks where you stopped), feed=<on|off> the Mac Reading settings' OPDS feed pane told as if it were (settings+pane=reading+sheet=feed), never switched on,
# party=<form|joining|refused|invite|panel|ready|sync|chat|chatline|chatbusy|chatreact|chatfield|chatkeys|chatwait|chatoff|chatfail|chatpanel|chatmute|chathidden|chatshort|lost|left|nosource|choose|unaired|quit>, a
# watch party on the testing engine's fake Syncplay server (PartyScene; the player's on the playing
# scene: playing:party=panel), showcase=<sources|stores|downloads|invite|pale|messages|proxy|checking|unsaved|all>, the invented rows of `Showcase`
# (pale: 900001's page with a long title on a near-white banner; proxy: torrents through a saved proxy,
# which sheet=proxy edits, and checking or unsaved its check under way or failed), and watched=<id>:<episodes>,
# episodes marked watched as the shot starts (900006:8 is caught up), and
# continuefrom=<device>, Home's Continue cards saying another device played last, fold=open, the title page's watched episodes opened, world=1, Home's invented season of ten
# drawn titles (with the scenes suggested, oneleft, continuefull, notfound, slow, failed and offline,
# which stage Home's states), offline=1, the fake AniList gone once the first Home has loaded,
# for Home's offline line over what it kept, and source=<licensed|server|folder>, the row
# Sources has selected (scenes licensed, servers and library seed one each, sourcesbusy all of them mid-scan, signed out and matching),
# and skippause=<low-power|storage>, Settings' "Find skip times" row paused for that reason:
#
#   apple/scripts/shots.sh mac watching watching:calendar watching:title=900001 fresh
#   apple/scripts/shots.sh mac watching:settings+pane=sources+showcase=sources
#
# Each goes to apple/.shots/<device>-<shot>.png, ":" and "=" made "-", which git ignores.
#
# The starting screen on the engine's made-up starts: player-start-<state>-<seconds>[-details]
# [-choose], with the state's name from StartingFixture in kebab case (player-start-slow-10,
# player-start-failed-no-episode-3-details). KAERI_REDUCE_MOTION=1 runs it with Reduce Motion.
#
# The player's states are shots too: player-starting, player-playing, player-skip, player-up-next,
# player-still-watching, player-stall, player-buffering, player-scrubbing, player-tracks (the
# Subtitles and audio menu open), player-speed, player-picture (Picture and sound), player-episodes
# (with the playing episode's sources), player-picture-in-picture and Stats for nerds
# (player-stats, -stats-hidden, -stats-waiting, -stats-stream, -stats-buffering, -stats-hdr,
# -stats-paused, -stats-copied) play the Playing scene's generated episode (apple/scripts/player-media.sh) to that state and shoot the player; "player" is
# all of them. KAERI_POINTED=<row title> draws a menu's row as if the pointer were over it.
#
# - mac: the dev harness (apple/Packages/KaeriDev) renders its own window once the store has
#   settled. It needs no Screen Recording permission.
# - iphone, ipad: the Kaeri Fixtures app, built for the Simulator and run on a device of its own
#   ("Kaeri iPhone", "Kaeri iPad", made on first use, or the one KAERI_SIMULATOR names),
#   with a clean status bar at 9:41 and the dark appearance. The iPad runs its apps full screen, not
#   in windows. The app prints KAERI_READY once settled; the shot is taken then, and a launch
#   that never says so stops the run with its console kept beside the shots.
#
# The Simulator is shot in portrait. --landscape turns the device first (devicectl's orientation
# command), and the files are named <device>-landscape-<shot>.png.
#
# KAERI_LAUNCH_ARGS adds launch arguments to every iPhone or iPad shot, split on spaces:
# KAERI_LAUNCH_ARGS="-AppleLanguages (en-XA)" shoots the pseudo-locale.
#
# KAERI_APPEARANCE=light shoots the iPhone or iPad (and the Mac dev harness) in the light appearance;
# dark is the default for the Simulator.
#
# KAERI_DERIVED_DATA picks where xcodebuild builds, apple/.derived unless it says otherwise.
set -euo pipefail
# Bash 4.4 and newer only. macOS's own /bin/bash is 3.2.
shopt -s inherit_errexit 2>/dev/null || true

players=(player-starting player-playing player-skip player-up-next player-still-watching player-stall
  player-buffering player-scrubbing player-tracks player-speed player-picture player-episodes
  player-picture-in-picture player-stats player-stats-hidden player-stats-waiting player-stats-stream
  player-stats-buffering player-stats-hdr player-stats-paused player-stats-copied player-discussion player-discussion-open
  player-mark-start player-mark-end player-mark-review player-mark-short player-mark-long player-mark-replaces
  player-mark-shares player-mark-noentry player-mark-editing player-mark-previewing player-prompt-opening
  player-prompt-wrong player-prompt-share player-skip-times player-found-skip player-found-controls
  player-found-prompt player-found-saved player-found-never player-found-review player-found-nudged
  player-found-undo player-found-panel player-found-panel-paused)

usage() {
  echo "usage: apple/scripts/shots.sh <mac|iphone|ipad> [--landscape] [<watching|sourceless|unsure|linkopened|fresh|playing|demoonly|demoplayed|demoandstore|demoremoved|subtitlestyled|crashoff|crashsaved|crashheld|storeowner|storeinvited|together>[:<section|key=value>]|player|player-<state>]…" >&2
  exit 2
}

device="${1:-}"
case "$device" in mac | iphone | ipad) ;; *) usage ;; esac
shift
orientation=portrait
if [[ "${1:-}" == --landscape ]]; then
  [[ "$device" != mac ]] || usage
  orientation=landscape
  shift
fi
shots=("$@")
(( ${#shots[@]} )) || shots=(watching)
# "player" is every player state.
expanded=()
for shot in "${shots[@]}"; do
  if [[ "$shot" == player ]]; then expanded+=("${players[@]}"); else expanded+=("$shot"); fi
done
shots=("${expanded[@]}")
for shot in "${shots[@]}"; do
  case "${shot%%:*}" in watching | sourceless | unsure | linkopened | fresh | playing | suggested | oneleft | continuefull | notfound | slow | failed | offline | licensed | servers | library | sourcesbusy | available | demoonly | demoplayed | demoandstore | demoremoved | subtitlestyled | crashoff | crashsaved | crashheld | storeowner | storeinvited | together | browsing | reader-manga-rtl | reader-strip | reader-novel | reader-moved | reading-dark | reading-first-run | reading-update-sheet | reading-join-ask | reading-home | reading-demo | reading-series | reading-downloads | eink-grey | eink-kaleido-unsure) ;; player-*) ;; *) usage ;; esac
  if [[ "$shot" == player-start-* ]]; then continue; fi
  if [[ "$shot" == player-* && ! " ${players[*]} " == *" $shot "* ]]; then usage; fi
done

# The starting screen's shot player-start-<state>-<seconds>[-details][-choose] as state, seconds
# and the options, one per line.
startparts() {
  local rest="${1#player-start-}" details=0 choose=0
  while :; do
    case "$rest" in
      *-details) details=1; rest="${rest%-details}" ;;
      *-choose) choose=1; rest="${rest%-choose}" ;;
      *) break ;;
    esac
  done
  printf '%s\n' "${rest%-*}" "${rest##*-}" "$details" "$choose"
}

# The launch arguments for a shot: the scene, and the screen it names.
arguments() {
  local shot="$1" scene="${1%%:*}" screens="" screen
  if [[ "$shot" == player-start-* ]]; then
    local parts
    parts=()
    while IFS= read -r line; do parts+=("$line"); done < <(startparts "$shot")
    printf '%s\n' -KaeriScene playing -KaeriPlayerShot start-fixture -KaeriStartState "${parts[0]}" -KaeriStartAt "${parts[1]}"
    [[ "${parts[2]}" != 1 ]] || printf '%s\n' -KaeriStartDetails 1
    [[ "${parts[3]}" != 1 ]] || printf '%s\n' -KaeriStartChoose 1
    return
  fi
  if [[ "$shot" == player-* ]]; then
    printf '%s\n' -KaeriScene playing -KaeriPlayerShot "${shot#player-}"
    [[ -z "${KAERI_POINTED:-}" ]] || printf '%s\n' -KaeriPlayerPointer "$KAERI_POINTED"
    return
  fi
  [[ "$shot" == *:* ]] && screens="${shot#*:}"
  # The reading scenes are written with dashes (reader-manga-rtl); the launch names have none.
  case "$scene" in reader-* | reading-* | eink-*) scene="${scene//-/}" ;; esac
  printf '%s\n' -KaeriScene "$scene"
  local IFS=+
  for screen in $screens; do
    case "$screen" in
      "") ;;
      title=*) printf '%s\n' -KaeriTitle "${screen#title=}" ;;
      select=*) printf '%s\n' -KaeriSelect "${screen#select=}" ;;
      query=*) printf '%s\n' -KaeriSection search -KaeriQuery "${screen#query=}" ;;
      genre=*) printf '%s\n' -KaeriSection search -KaeriGenre "${screen#genre=}" ;;
      available=*) printf '%s\n' -KaeriSection search -KaeriAvailable "${screen#available=}" ;;
      filters=*) printf '%s\n' -KaeriSection search -KaeriFilters "${screen#filters=}" -KaeriCatalogue ;;
      scroll=*) printf '%s\n' -KaeriScroll "${screen#scroll=}" ;;
      palette=*) printf '%s\n' -KaeriPalette "${screen#palette=}" ;;
      pane=*) printf '%s\n' -KaeriPane "${screen#pane=}" ;;
      skippause=*) printf '%s\n' -KaeriSkipPause "${screen#skippause=}" ;;
      notice=*) printf '%s\n' -KaeriNotice "${screen#notice=}" ;;
      sheet=*) printf '%s\n' -KaeriSheet "${screen#sheet=}" ;;
      tv=*) printf '%s\n' -KaeriTV "${screen#tv=}" ;;
      sync=*) printf '%s\n' -KaeriSync "${screen#sync=}" ;;
      prelaunch=*) printf '%s\n' -KaeriPrelaunch "${screen#prelaunch=}" ;;
      cast=*) printf '%s\n' -KaeriCast "${screen#cast=}" ;;
      reader=*) printf '%s\n' -KaeriReader "${screen#reader=}" ;;
      medium=*) printf '%s\n' -KaeriMedium "${screen#medium=}" ;;
      work=*) printf '%s\n' -KaeriWork "${screen#work=}" ;;
      reading=*) printf '%s\n' -KaeriReading "${screen#reading=}" ;;
      send=*) printf '%s\n' -KaeriSend "${screen#send=}" ;;
      feed=*) printf '%s\n' -KaeriFeed "${screen#feed=}" ;;
      party=*) printf '%s\n' -KaeriParty "${screen#party=}" ;;
      together=*) printf '%s\n' -KaeriSection together -KaeriTogether "${screen#together=}" ;;
      showcase=*) printf '%s\n' -KaeriShowcase "${screen#showcase=}" ;;
      watched=*) printf '%s\n' -KaeriWatched "${screen#watched=}" ;;
      source=*) printf '%s\n' -KaeriSource "${screen#source=}" ;;
      fold=*) printf '%s\n' -KaeriFold "${screen#fold=}" ;;
      day=*) printf '%s\n' -KaeriDay "${screen#day=}" ;;
      mine=*) printf '%s\n' -KaeriCalendarMine "${screen#mine=}" ;;
      continuefrom=*) printf '%s\n' -KaeriContinueFrom "${screen#continuefrom=}" ;;
      world=*) printf '%s\n' -KaeriHomeWorld "${screen#world=}" ;;
      offline=*) printf '%s\n' -KaeriGoOffline "${screen#offline=}" ;;
      *) printf '%s\n' -KaeriSection "$screen" ;;
    esac
  done
}

# A shot's file name.
name() {
  local name="${1//:/-}"
  name="${name//+/-}"
  echo "${name//=/-}"
}

apple="$(cd "$(dirname "$0")/.." && pwd)"
out="$apple/.shots"
mkdir -p "$out"
for shot in "${shots[@]}"; do
  if [[ "$shot" == player-* || "$shot" == playing* ]]; then "$apple/scripts/player-media.sh" >/dev/null; break; fi
done

if [[ "$device" == mac ]]; then
  "$apple/scripts/engine.sh" testing macos
  for shot in "${shots[@]}"; do
    if [[ "$shot" == player-start-* ]]; then
      parts=()
      while IFS= read -r line; do parts+=("$line"); done < <(startparts "$shot")
      (cd "$apple/Packages/KaeriDev" &&
        KAERI_START_STATE="${parts[0]}" KAERI_START_AT="${parts[1]}" KAERI_START_DETAILS="${parts[2]}" \
        KAERI_START_CHOOSE="${parts[3]}" swift run KaeriDev --scene playing --player-shot start-fixture \
          --snapshot "$out/mac-$shot.png")
      continue
    fi
    if [[ "$shot" == player-* ]]; then
      pointed=()
      [[ -z "${KAERI_POINTED:-}" ]] || pointed=(-KaeriPlayerPointer "$KAERI_POINTED")
      (cd "$apple/Packages/KaeriDev" &&
        swift run KaeriDev --scene playing --player-shot "${shot#player-}" ${pointed[@]+"${pointed[@]}"} \
          --snapshot "$out/mac-$shot.png")
      continue
    fi
    args=()
    while IFS= read -r arg; do args+=("$arg"); done < <(arguments "$shot")
    # The harness takes the scene as --scene, and passes the rest to the app.
    (cd "$apple/Packages/KaeriDev" &&
      swift run KaeriDev --scene "${args[1]}" "${args[@]:2}" --snapshot "$out/mac-$(name "$shot").png")
  done
  exit 0
fi

# The Simulator device, made from the newest iOS runtime that has it when it is not there yet.
case "$device" in
  iphone) name="Kaeri iPhone" type="com.apple.CoreSimulator.SimDeviceType.iPhone-17" ;;
  ipad) name="Kaeri iPad" type="com.apple.CoreSimulator.SimDeviceType.iPad-Pro-11-inch-M5-12GB" ;;
esac
name="${KAERI_SIMULATOR:-$name}"
udid="$(xcrun simctl list devices -j | /usr/bin/python3 -I -c '
import json, sys
name = sys.argv[1]
for runtime, devices in json.load(sys.stdin)["devices"].items():
    for d in devices:
        if d["name"] == name and d["isAvailable"]:
            print(d["udid"]); raise SystemExit
' "$name")"
if [[ -z "$udid" ]]; then
  runtime="$(xcrun simctl list runtimes -j | /usr/bin/python3 -I -c '
import json, sys
kind = sys.argv[1]
ios = [
    r for r in json.load(sys.stdin)["runtimes"]
    if r["platform"] == "iOS" and r["isAvailable"]
    and any(t["identifier"] == kind for t in r.get("supportedDeviceTypes", []))
]
ios.sort(key=lambda r: [int(p) for p in r["version"].split(".")])
print(ios[-1]["identifier"] if ios else "")
' "$type")"
  [[ -n "$runtime" ]] || {
    echo "shots.sh: no iOS Simulator runtime has $type; xcodebuild -downloadPlatform iOS" >&2
    exit 1
  }
  udid="$(xcrun simctl create "$name" "$type" "$runtime")"
fi

derived="${KAERI_DERIVED_DATA:-$apple/.derived}"
(cd "$apple" && xcodegen generate --quiet)
xcodebuild -project "$apple/Kaeri.xcodeproj" -scheme "Kaeri Fixtures" \
  -destination "id=$udid" -derivedDataPath "$derived" build -quiet
app="$derived/Build/Products/Debug-fixtures-iphonesimulator/Kaeri Fixtures.app"

xcrun simctl boot "$udid" 2>/dev/null || true
xcrun simctl bootstatus "$udid" -b >/dev/null
# The iPad's apps full screen: neither Stage Manager nor windowed apps, which put the app in a
# window of its own size and its name in the status bar. SpringBoard reads them as it starts.
if [[ "$device" == ipad ]]; then
  windowing() { xcrun simctl spawn "$udid" defaults read com.apple.springboard "$1" 2>/dev/null || true; }
  if [[ "$(windowing SBChamoisWindowingEnabled)" != 0 || "$(windowing SBMedusaMultitaskingEnabled)" != 0 ]]; then
    xcrun simctl spawn "$udid" defaults write com.apple.springboard SBChamoisWindowingEnabled -bool NO
    xcrun simctl spawn "$udid" defaults write com.apple.springboard SBMedusaMultitaskingEnabled -bool NO
    xcrun simctl shutdown "$udid"
    xcrun simctl boot "$udid"
    xcrun simctl bootstatus "$udid" -b >/dev/null
  fi
fi
xcrun simctl ui "$udid" appearance "${KAERI_APPEARANCE:-dark}"
# Apple's clean status bar: 9:41, full Wi-Fi and battery, no carrier.
xcrun simctl status_bar "$udid" override --time 9:41 --dataNetwork wifi --wifiMode active \
  --wifiBars 3 --cellularMode active --cellularBars 4 --operatorName '' \
  --batteryState discharging --batteryLevel 100
xcrun simctl install "$udid" "$app"
# Held as the shots ask: the device stays as the last run left it otherwise.
case "$orientation" in landscape) turn=landscapeLeft ;; *) turn=portrait ;; esac
if ! xcrun devicectl --quiet device orientation set --device "$udid" "$turn" >/dev/null 2>&1; then
  [[ "$orientation" == portrait ]] || {
    echo "shots.sh: devicectl could not turn the Simulator to landscape" >&2
    exit 1
  }
fi

prefix="$device"
[[ "$orientation" == portrait ]] || prefix="$device-$orientation"
for shot in "${shots[@]}"; do
  log="$(mktemp -t kaeri-shot)"
  args=()
  while IFS= read -r arg; do args+=("$arg"); done < <(arguments "$shot")
  # shellcheck disable=SC2206
  [[ -z "${KAERI_LAUNCH_ARGS:-}" ]] || args+=(${KAERI_LAUNCH_ARGS})
  xcrun simctl terminate "$udid" app.kaeri.fixtures 2>/dev/null || true
  # Attached to the app's console, which ends when the app is terminated: the app's output
  # reaches a file this way, where --stdout leaves it empty.
  # --terminate-running-process: a launch that finds the app still running, as it is while the
  # last one is still going away, only brings that process forward, with its own arguments and
  # no console here. The shot would then be of the wrong scene.
  xcrun simctl launch --console-pty --terminate-running-process "$udid" app.kaeri.fixtures \
    "${args[@]}" >"$log" 2>&1 &
  ready=0
  for _ in $(seq 1 120); do
    if grep -q KAERI_READY "$log"; then ready=1; break; fi
    sleep 0.5
  done
  if (( ! ready )); then
    # No shot of a screen that never said it was ready: the console is kept to say why.
    kept="$out/$prefix-$(name "$shot").log"
    cp "$log" "$kept"
    echo "shots.sh: the app did not say it was ready for $shot; its console is in $kept" >&2
    tail -n 20 "$kept" >&2
    xcrun simctl terminate "$udid" app.kaeri.fixtures 2>/dev/null || true
    wait
    rm -f "$log"
    exit 1
  fi
  # The art's and the preview's fade-ins, and KAERI_SHOT_WAIT seconds more for a trailer.
  sleep "$(echo "0.5 + ${KAERI_SHOT_WAIT:-0}" | bc)"
  xcrun simctl io "$udid" screenshot --type=png "$out/$prefix-$(name "$shot").png" >/dev/null
  echo "shots.sh: wrote $out/$prefix-$(name "$shot").png"
  xcrun simctl terminate "$udid" app.kaeri.fixtures 2>/dev/null || true
  wait
  rm -f "$log"
done

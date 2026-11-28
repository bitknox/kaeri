#!/usr/bin/env bash
# slot's own build caches on the GitHub lane: the Android engine and libmpv that an earlier slot
# job built, kept as run artifacts named by a hash of what they are built from, never by branch.
# GitHub's Actions cache cannot do this: a cache saved on one slot/ branch is invisible to every
# other slot/ branch, and slot.yml saves none so main's caches stay main's. Without this, every
# gh android job built libmpv (9 min) and the engine (about 36 min) again: three jobs on 24
# November built the same engine hash a1bb3922 three times.
#
#   cache.sh restore   before the job: fetch what an earlier job left, if anything
#   cache.sh pack      after the job: tar what this job built that no artifact has yet
#
# The engine's and libmpv's own stamps decide whether a restored build is current (engine.sh
# and mpv.sh rebuild when it is not), so a wrong or stale artifact costs a build, never a wrong
# binary. Needs GH_TOKEN (the run's read-only token with actions: read), GITHUB_REPOSITORY,
# GITHUB_OUTPUT, RUNNER_TEMP, NDK_VERSION and ENGINE_HASH. Run from the repository's root.
set -uo pipefail
mode=${1:?restore or pack}
repo=$GITHUB_REPOSITORY
ecache=$HOME/.cache/kaeri-engine/android
work=$RUNNER_TEMP/slot/cache
mkdir -p "$work"
say() { echo "slot cache: $*"; }

# libmpv's key: mpv.sh's own sources hash (mpv.sh and android/mpv), and the NDK.
mpv_hash=$(git ls-files -co --exclude-standard -- android/scripts/mpv.sh android/mpv | LC_ALL=C sort -u |
  while IFS= read -r f; do [ -f "$f" ] && printf '%s\n' "$f"; done | git hash-object --stdin-paths | shasum -a 256 | cut -c1-40)
pin=$(sed -n 's/^pin=//p' android/scripts/mpv.sh | head -1)
ename=slot-engine-android-ndk${NDK_VERSION}-${ENGINE_HASH}
mname=slot-mpv-android-ndk${NDK_VERSION}-${mpv_hash}

engine_entries() { find "$ecache" -mindepth 4 -maxdepth 4 -type d -name "$ENGINE_HASH" 2>/dev/null; }

newest_run() { # <artifact name>: the run id of its newest unexpired copy
  gh api "repos/$repo/actions/artifacts?name=$1&per_page=10" \
    --jq '[.artifacts[] | select(.expired | not)] | sort_by(.created_at) | last | .workflow_run.id // empty' 2>/dev/null
}
fetch() { # <artifact name> <dir to untar in>
  local run t0=$SECONDS d
  run=$(newest_run "$1")
  [ -n "$run" ] || { say "no $1 yet"; return 1; }
  d=$(mktemp -d "$work/dl.XXXX")
  if timeout 600 gh run download "$run" -R "$repo" -n "$1" -D "$d" >/dev/null 2>"$d.err" && [ -f "$d/cache.tar" ]; then
    mkdir -p "$2" && tar -xf "$d/cache.tar" -C "$2" && rm -rf "$d" &&
      { say "restored $1 from run $run in $((SECONDS - t0)) s"; return 0; }
  fi
  say "fetching $1 from run $run failed: $(tail -1 "$d.err" 2>/dev/null)"; rm -rf "$d"; return 1
}

case "$mode" in
  restore)
    eh=0 mh=0
    if [ -n "$(engine_entries)" ]; then say "the engine $ENGINE_HASH came from main's cache"; eh=1
    elif fetch "$ename" "$ecache"; then eh=1; fi
    if fetch "$mname" android/.mpv; then
      mh=1; ln -sfn "$pin" android/.mpv/current
    fi
    { echo "engine=$eh"; echo "mpv=$mh"; echo "ename=$ename"; echo "mname=$mname"; } >> "$GITHUB_OUTPUT"
    ;;
  pack)
    # Only what was missing at the start and is complete now: an engine entry is renamed into
    # place whole, and libmpv's stamp is written last.
    if [ "${RESTORED_ENGINE:-1}" = 0 ] && [ -n "$(engine_entries)" ]; then
      mkdir -p "$work/engine"
      engine_entries | sed "s|^$ecache/||" | tar -cf "$work/engine/cache.tar" -C "$ecache" -T -
      [ -s "$work/engine/cache.tar" ] && { echo "engine=1" >> "$GITHUB_OUTPUT"; say "packed the engine $ENGINE_HASH ($(du -sh "$work/engine/cache.tar" | cut -f1))"; }
    fi
    if [ "${RESTORED_MPV:-1}" = 0 ] && [ -n "$pin" ] && [ -f "android/.mpv/$pin/stamp" ] && [ -d "android/.mpv/$pin/jniLibs" ]; then
      mkdir -p "$work/mpv" && tar -cf "$work/mpv/cache.tar" -C android/.mpv "$pin" &&
        { echo "mpv=1" >> "$GITHUB_OUTPUT"; say "packed libmpv $pin ($(du -sh "$work/mpv/cache.tar" | cut -f1))"; }
    fi
    ;;
  *) echo "usage: cache.sh restore|pack" >&2; exit 64 ;;
esac
exit 0

#!/bin/sh
# The .app's entry point.
#
# Finder launches a bundle with no controlling terminal, so running the TUI directly here would
# get a program with nowhere to draw. This re-launches the real binary inside a terminal instead.
#
# `open -a` is used rather than exec'ing the terminal directly because it goes through Launch
# Services, which is what raises and focuses an already-running terminal instead of starting a
# second copy.

set -eu

HERE="$(cd "$(dirname "$0")" && pwd)"
BIN="$HERE/kaeri"

if [ ! -x "$BIN" ]; then
	osascript -e 'display alert "kaeri" message "The Kaeri binary is missing from this application bundle."'
	exit 1
fi

# Honour the terminal the user actually lives in, falling back to the one every Mac has.
for candidate in "${KAERI_TERMINAL:-}" Ghostty kitty WezTerm iTerm Terminal; do
	[ -n "$candidate" ] || continue
	if open -a "$candidate" "$BIN" 2>/dev/null; then
		exit 0
	fi
done

osascript -e 'display alert "kaeri" message "Could not open a terminal to run Kaeri in."'
exit 1

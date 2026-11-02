## Torrents: switching them on, the optional proxy, its check, and opening your own links.
## Engine notices about the proxy check (torrent-proxy-waiting and so on) are in engine*.ftl.

# Title of the settings group and the screen.
torrents-title = Torrents

# Label and short value for the switch.
torrents-on = Torrents are on

# Line under the switch while torrents are off.
torrents-off-detail = Off by default. Torrents connect directly unless you route them through a proxy.

# Line about privacy in the torrent sheet, before torrents start.
torrents-risk = Other people in a torrent can see your IP address. To keep it private, route torrent traffic through a proxy.

# Line in the Apple sources screen: torrents belong to the app, not to a plugin.
torrents-not-a-plugin = Torrents are part of { -app-name }, not a { -plugin }. The proxy and seeding are set in Settings, under { -source-cap-pl }.

## Proxy

# Switch: send torrent traffic through a SOCKS5 proxy. Off by default.
torrents-proxy-route = Route torrent traffic through a proxy

# Line under the switch.
torrents-proxy-route-detail = Torrent traffic goes through a SOCKS5 proxy you run or rent. While this is off, torrents connect directly.

# Summary: torrents go through a proxy. Host and port are already formatted.
# @arg host: text
# @arg port: text
torrents-summary-proxy-at = Through a SOCKS5 proxy at { $host }:{ $port }.

# Summary: torrents go through a proxy, address not shown.
torrents-summary-proxy = Through a SOCKS5 proxy.

# Summary: no proxy is set.
torrents-summary-direct = Connecting directly, without a proxy.

## Proxy form

# Label of the proxy host field.
torrents-proxy-host = Proxy host

# Label of the proxy port field.
torrents-proxy-port = Proxy port

# Label of the proxy user name field.
torrents-proxy-user = User name

# Label of the proxy password field.
torrents-proxy-password = Password

# Line under the password field.
torrents-password-keep = Leave the password empty to keep the saved one.

# Line under the proxy form: what happens while the new proxy is checked.
# @allow proceed
torrents-proxy-keeps-saved = Torrents keep using the saved proxy until the new one passes the proxy check.

# Line when the host or port is wrong.
torrents-proxy-invalid = The proxy needs a host and a port from 1 to 65535.

# Line while the proxy is checked.
torrents-checking-proxy = Checking the proxy…

# Notice after the proxy was saved.
torrents-proxy-saved = Proxy saved

# Why a save was refused: the proxy check failed. A sentence fragment.
torrents-check-not-made-proxy = The proxy check failed

# Notice when a proxy was not saved. The reason is a sentence fragment.
# @arg why: cause
torrents-proxy-not-saved = Not saved. { $why }, so the saved proxy stays.

# Notice when torrents stayed off. The reason is a sentence fragment.
# @arg why: cause
torrents-still-off = Torrents are still off. { $why }.

## Torrent index

# Label of the index address field.
torrents-index = { -torrent-index-cap }

# Line under the index field.
torrents-index-detail = The RSS search address { -app-name } asks for torrents.

# Hint inside the index field.
torrents-index-hint = https://…/?q={ "{query}" }

# Line when no index address is set.
torrents-index-needed = Torrents need an index address.

# Line when the index address has the wrong scheme.
torrents-index-scheme = Use an http or https address.

# Notice after the index was saved.
torrents-index-saved = { -torrent-index-cap } saved

## The proxy check

# Under the proxy form: what the app checks.
torrents-check-note = { -app-name } checks that it can reach the proxy before torrents start. Torrents wait while that check fails.

# Label of the proxy check's result.
torrents-last-check = Proxy check

# Check passed.
torrents-check-passed = Passed

# Check running: the short word.
torrents-check-running = Checking

# Check failed: the line.
torrents-check-failed-line = The proxy check failed. Torrents start once it passes.

# Short word: the check failed.
torrents-check-failed = Failed

# Line: no check yet.
torrents-check-waiting-line = Not checked yet

# Short word: waiting for the first check.
torrents-check-waiting = Waiting

# Short word: no check.
torrents-not-checked = Not checked

# Button that runs the check.
torrents-check-now = Check now

## Pages on the TV that a phone fills in

# Title of the page on the phone for the torrent setup.
torrents-for-app = Torrents for { -app-name }

# Title of the page on the phone for signing in. The tracker is a name.
# @arg tracker: text
torrents-page-sign-in = Sign in to { $tracker }

# Line on that page: the form was not complete.
torrents-page-fill-form = Fill in the form, then send it again.

# Line on that page after sending.
# @arg next: text
torrents-page-look-at-tv = Look at the TV: { $next }. You can close this page.

# Title when the page address is wrong.
torrents-page-not-found = Not found

# Title when the page is not allowed.
torrents-page-not-allowed = Not allowed

# Line when the address lacks its key.
torrents-page-missing-key = This address is missing its key. Scan the QR code on the TV again, and open the whole address it gives.

# Line when the TV could not read what was sent.
torrents-page-unreadable = The TV couldn't read this. Scan the QR code on the TV again.

# Line when the page needs script.
torrents-page-needs-script = This page needs JavaScript to send the form to the TV safely.

## Opening your own magnet link or torrent file

# Menu item and button: open a magnet link or a torrent file the viewer has.
# @max 32
opened-open = Open link or file

# Title of the sheet where the viewer pastes a magnet link.
opened-paste-title = Open a magnet link or torrent file

# Placeholder in the field for the link.
opened-paste-placeholder = Magnet link or info hash

# Button: choose a torrent file from the device.
opened-choose-file = Choose a torrent file

# The question once the link's name matched a title and an episode.
# @arg title: text
# @arg episode: text
opened-confirm = Is this { $title }, episode { $episode }?

# The question when the name matched a title but states no episode.
# @arg title: text
opened-confirm-title = Is this { $title }? Choose the episode.

# Line when the name matched nothing, or the link has no name.
opened-no-guess = { -app-name } couldn't tell what this is. Choose the title it is, or play it without tracking.

# Button: play it as the title and episode asked about.
# @max 24
opened-play = Play

# Button: pick another title or episode.
# @max 24
opened-change = Change

# Button: play with nothing recorded.
# @max 32
opened-play-untracked = Play without tracking

# Line under Play without tracking.
opened-untracked-detail = No history, resume point, { -tracker } progress or sync.

# Button: download it for offline as the title and episode asked about.
# @max 24
opened-download = Download

# Heading over the titles the link could be, under Change.
opened-change-title = Which title is it?

# Placeholder in the search field under Change.
opened-change-search = Search for the title

# Label of the episode field under Change.
opened-episode = Episode

# Shown in place of Play while torrent playback is off.
# @max 40
opened-turn-on = Turn on torrent playback

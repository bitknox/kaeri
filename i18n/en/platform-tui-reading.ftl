# Reading's Home and Search in the terminal, drawn as text. The terminal has no reader: a
# series found here opens in the apps.

## Search

# Heading over AniList's results in reading's search, with how many it found. Shown once every
# result is listed.
# @arg n: int
# @only tui
tui-read-anilist-count = { $n ->
    [one] AniList · 1 series
   *[other] AniList · { $n } series
 }

# Heading over AniList's results while more pages are to come, or while AniList is asked.
# @only tui
tui-read-anilist = AniList

# The line under the search field when no filter is set. Lower case, as the terminal's metadata
# lines are.
# @only tui
tui-read-filters-none = filters none

# The line under the search field, with the filters set. The filters are already joined, such as
# "Manga · Japan".
# @arg set: text
# @only tui
tui-read-filters = filters { $set }

# On the line under the search field: the switch that hides AniList's results. "on" or "off"
# follows it in bold.
# @only tui
tui-read-sources-only = your { -source-pl } only

# The state of that switch, after it.
# @only tui
tui-read-on = on

# The state of that switch, after it.
# @only tui
tui-read-off = off

# Beside a source's name in the search results: the source is a book server.
# @only tui
tui-read-kind-server = server

# Beside a source's name in the search results: the source is the folders on this computer.
# @only tui
tui-read-kind-folder = folder

# Beside a source's name in the search results: the source is a { -plugin }.
# @only tui
tui-read-kind-plugin = { -plugin }

# Beside an AniList result that one of the person's sources has. The source is its name. Lower
# case, as the terminal's metadata lines are.
# @arg source: text
# @only tui
tui-read-in-source = in { $source }

# Beside an AniList result that more than one of the person's sources has. Always 2 or more.
# @arg n: int
# @only tui
tui-read-in-sources = { $n ->
    [one] in { $n } of your { -source-pl }
   *[other] in { $n } of your { -source-pl }
 }

# After a source that didn't answer, beside the key that asks it again. Lower case.
# @only tui
tui-read-try-again = try again

# Toast when "Your sources only" turns on.
# @only tui
tui-read-sources-only-on = Your { -source-pl } only. AniList's results are hidden.

# Toast when "Your sources only" turns off.
# @only tui
tui-read-sources-only-off = AniList's results show again.

## Status line and keys

# Left of the status line on reading's Home and Search: how many sources are asked.
# @arg n: int
# @only tui
tui-read-status = { $n ->
    [one] Reading · 1 { -source }
   *[other] Reading · { $n } { -source-pl }
 }

# Left of the status line on reading's Home and Search, when the terminal is narrow.
# @max 12
# @only tui
tui-read-status-short = Reading

# Hint in the status line on reading's Search: Tab moves between AniList's results and the
# sources'. Shown in lower case.
# @max 20
# @only tui
tui-read-hint-next-section = Next section

# The same hint when the terminal is narrow.
# @max 12
# @only tui
tui-read-hint-section = Section

# Hint in the status line on reading's Search: the key that turns "Your sources only" on and off.
# Shown in lower case.
# @max 24
# @only tui
tui-read-hint-sources-only = Your { -source-pl } only

# The same hint when the terminal is narrow.
# @max 16
# @only tui
tui-read-hint-sources-only-short = { -source-cap-pl } only

# Heading of the reading keys in the key reference.
# @only tui
tui-read-keys-heading = Reading

# A row of the key reference: move between AniList's results and the sources'.
# @only tui
tui-read-keys-next-section = Next section of Search

# A row of the key reference: turn "Your sources only" on or off.
# @only tui
tui-read-keys-sources-only = Your { -source-pl } only, on or off

# A row of the key reference: ask a source that didn't answer again.
# @only tui
tui-read-keys-try-again = Ask a { -source } again

# A row of the key reference: the filters of reading's search.
# @only tui
tui-read-keys-filters = Search filters

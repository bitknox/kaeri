# Reading's Home and Search in the terminal, drawn as text. The terminal has no reader: a
# series found here opens in the apps.

## Search

# Heading over AniList's results in reading's search, with how many it found. Shown once the
# count is known, which is when every result is listed.
# @arg n: int
# @only tui
tui-read-anilist-count = { $n ->
    [one] AniList · 1 series
   *[other] AniList · { $n } series
 }

# AniList's name, in the line that says it is still being asked.
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

# Hint in the status line on the shows' Home and Search while Kaeri is for shows and reading:
# the key that turns Home and Search to reading's. Shown in lower case.
# @max 16
# @only tui
tui-read-hint-to-reading = Reading

# Hint in the status line on reading's Home and Search while Kaeri is for shows and reading: the
# key that turns Home and Search back to the shows'. Shown in lower case.
# @max 16
# @only tui
tui-read-hint-to-shows = Shows

# Hint in the status line on a reading title the Planning list can take. Shown in lower case.
# @max 16
# @only tui
tui-read-hint-planning = Planning

# Left of the status line on the shows' Home and Search while Kaeri is for shows and reading,
# before what the line says there anyway, such as "Demo · Subbed · 1080p".
# @arg state: text
# @only tui
tui-read-status-shows = Shows · { $state }

# Action label in the key reference and the command palette: turns Home and Search between the
# shows' and reading's, while Kaeri is for both.
# @only tui
tui-read-action-switch = Shows or reading on Home and Search

# Ends the toast that says a command does nothing here: it follows the command's name and
# "works", and names where it does.
# @only tui
tui-read-where-switch = on Home and Search while Kaeri is for shows and reading

# In the key reference's reading keys: puts the title under the cursor on the Planning list.
# Works on Home's discovery rows and AniList's results.
# @only tui
tui-read-keys-planning = Add to Planning

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

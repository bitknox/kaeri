# Reading's Home and Search in the terminal, drawn as text. The terminal has no reader: a
# series found here opens in the apps.

## Search

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

# After a source that didn't answer, beside the key that asks it again. Lower case.
# @only tui
tui-read-try-again = try again

# Toast when "Your sources only" is pressed with nothing typed in reading's search: the sources
# are asked for words, never browsed.
# @only tui
tui-read-sources-only-words = Type a title to search your { -source-pl }.

# Search with "Your sources only" on, when none of the sources found anything. A line saying to
# try other words follows.
# @only tui
tui-read-sources-nothing = Nothing found in your { -source-pl }

# A source in reading's search that could not be asked because the computer is offline. The
# source is its name. Folders still answer offline; servers and plugins need a connection.
# @arg source: text
# @only tui
tui-read-source-offline = { $source } needs a connection.

# Names in a sentence, the last joined with "and": "Home library and Northwind", "A, B and C".
# The names before the last are already joined with commas. Used in "Nothing found in …".
# @arg names: text
# @arg last: text
# @only tui
tui-read-names-and = { $names } and { $last }

# Toast after Enter on a series in reading's Home or Search. The terminal has no reader.
# @max 40
# @only tui
tui-read-opens-in-apps = Opens in the { -app-name } apps.

# On reading's Home when no book source is set up. The terminal cannot add one; the apps can.
# @only tui
tui-read-sources-in-apps = Folders, servers and { -plugin-store-pl } are added in the { -app-name } apps.

# Toast when "Your sources only" turns on.
# @only tui
tui-read-sources-only-on = Your { -source-pl } only. AniList's results are hidden.

# Toast when "Your sources only" turns off.
# @only tui
tui-read-sources-only-off = AniList's results show again.

## Status line and keys

# Header chip while Kaeri is for reading alone: how many of the person's book sources are on. The
# mark is a dot, filled when one or more is on. Lower case, as the header's other chips are.
# @arg mark: text
# @arg n: int
# @only tui
tui-read-header-sources = { -source-pl } { $mark } { $n }

# Left of the status line on reading's Home and Search: how many of the person's sources are on.
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
# the key that goes to Read from Home, and turns Search to reading's. Shown in lower case.
# @max 16
# @only tui
tui-read-hint-to-reading = Reading

# Hint in the status line on Read and reading's Search while Kaeri is for shows and reading: the
# key that goes back to the shows' Home, or turns Search back to the shows'. Shown in lower case.
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

# Action label in the key reference and the command palette, while Kaeri is for both: from the
# shows' Home to Read and back, and Search turned between the shows' and reading's.
# @only tui
tui-read-action-switch = Shows or reading

# Ends the toast that says a command does nothing here: it follows the command's name and
# "works", and names where it does.
# @only tui
tui-read-where-switch = on Home, Read and Search while { -app-name } is for shows and reading

# In the key reference's reading keys: puts the title under the cursor on the Planning list.
# Works on Home's discovery rows and AniList's results.
# @only tui
tui-read-keys-planning = Add to Planning

# Heading of the reading keys in the key reference.
# @only tui
tui-read-keys-heading = Reading

# A row of the key reference: move to the next part of the screen, AniList's results or the
# sources' on Search, and the next row on Home.
# @only tui
tui-read-keys-next-section = Next section

# A row of the key reference: turn "Your sources only" on or off.
# @only tui
tui-read-keys-sources-only = Your { -source-pl } only, on or off

# A row of the key reference: ask a source or AniList again on Search, or load Home again.
# @only tui
tui-read-keys-try-again = Ask again, or load again

# A row of the key reference: the filters of reading's search.
# @only tui
tui-read-keys-filters = Search filters

# Reading in the terminal past Home and Search: a series' page with its chapters, and the reader
# it opens. The terminal draws them as text.

## Series page

# Under a series' title on the terminal's series page: the source its chapters are read from.
# @arg source: text
# @only tui
tui-read-series-from = Reading from { $source }

# After the source on the series page: the list the series is on and the trackers that keep that
# list, such as "Reading on AniList". The list is its name, such as "Reading" or "Planning"; the
# trackers are already joined.
# @arg list: text
# @arg trackers: text
# @only tui
tui-read-series-list-on = { $list } on { $trackers }

# After the source on the series page, when no tracker keeps the list: "On your Reading list".
# @arg list: text
# @only tui
tui-read-series-list = On your { $list } list

# Beside the progress meter on the series page: how many of the chapters listed are read.
# @arg read: int
# @arg total: int
# @only tui
tui-read-series-read = { $total ->
    [one] { $read } of 1 chapter read
   *[other] { $read } of { $total } chapters read
 }

# At the top of the series page while the computer is offline: the chapters listed are the ones
# kept from before, and only the downloaded ones open.
# @only tui
tui-read-series-offline = Offline. Downloaded chapters still open.

# Left of the status line on the series page while the computer is offline.
# @max 24
# @only tui
tui-read-status-offline = Reading · offline

# Header chip on the series page while the computer is offline, in place of how many sources are
# on. The mark is a cross. Lower case, as the header's other chips are.
# @arg mark: text
# @only tui
tui-read-header-offline = { -source-pl } { $mark }

## Status line and keys

# Hint in the status line on the series page: the key that marks the chapter under the cursor
# read, or unread when it is read. Shown in lower case. Also a row of the key reference.
# @max 20
# @only tui
tui-read-read-unread = Read or unread

# Hint in the status line on the series page: the key that picks the list the series is on.
# Shown in lower case.
# @max 12
# @only tui
tui-read-hint-status = Status

# Hint in the status line on reading's Home, on a series in Continue: the key that opens the
# chapter where reading stopped. Shown in lower case.
# @max 12
# @only tui
tui-read-hint-continue = { -continue-reading-cap }

# Heading of the series page's keys in the key reference.
# @only tui
tui-read-keys-series = Series

# A row of the key reference: open the chapter under the cursor on the series page.
# @only tui
tui-read-keys-read = Read the chapter

# A row of the key reference: download the chapter under the cursor on the series page.
# @only tui
tui-read-keys-download = Download this chapter

# A row of the key reference: pick the list the series is on, such as Reading or Planning.
# @only tui
tui-read-keys-status = Set the list status

# A row of the key reference on reading's Home: open a series' page with its chapters, from a
# series in Continue, where Enter opens the chapter itself.
# @only tui
tui-read-keys-chapters = Open the series page

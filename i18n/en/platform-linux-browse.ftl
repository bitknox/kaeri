# Strings only the Linux app shows, for its browsing screens: the title page, torrents,
# downloads, search, calendar, library and the source picker.

## Torrents

# Title of the dialog that edits the proxy.
# @only linux
# @max 40
torrents-linux-edit-proxy-title = Edit proxy

## Downloads

# Seeding row: time left until the goal, in a span already worded ("18h", "1d 6h").
# @only linux
# @arg span: text
# @max 24
downloads-linux-seed-left = { $span } left

# Seeding row: less than an hour left until the goal.
# @only linux
# @max 30
downloads-linux-seed-left-soon = Less than an hour left

# Storage row: free space, the disk's size and what downloads take. All three are sizes already formatted.
# @only linux
# @arg free: bytes
# @arg total: bytes
# @arg taken: bytes
# @max 80
downloads-linux-space-free = { $free } of { $total } free · downloads take { $taken }

## Search

# Placeholder of the search field. It also finds settings.
# @only linux
# @max 40
search-linux-placeholder = Search titles and settings

# Tooltip of the button that clears the recent searches.
# @only linux
# @max 40
search-linux-clear-recent = Clear recent searches

# Title of the state when a browse with no filters lists nothing.
# @only linux
# @max 40
search-linux-nothing-found = Nothing found

# Title of the state when a typed search waits for Enter.
# @only linux
# @max 40
search-linux-press-enter = Press Enter to search

# Line under the Enter title: why results do not come as you type. The tracker is a name.
# @only linux
# @arg tracker: text
# @max 120
search-linux-paused = Searching as you type is paused for a moment, to stay within { $tracker }'s limits.

# Menu title over the sort choices of the filter row.
# @only linux
# @max 24
filter-linux-sort-by = Sort by

# Sort choice: what is rising now.
# @only linux
# @max 24
filter-linux-sort-trending = Trending

# Sort choice: the most recent first.
# @only linux
# @max 24
filter-linux-sort-newest = Newest

# Sort choice: the earliest first.
# @only linux
# @max 24
filter-linux-sort-oldest = Oldest

# Sort choice: the most favourited first.
# @only linux
# @max 24
filter-linux-sort-favourites = Favourites

# Genre filter button when several genres are set.
# @only linux
# @arg count: int
# @max 24
filter-linux-genres-count = { $count ->
    [one] { $count } genre
   *[other] { $count } genres
}

# Format filter button when several formats are set.
# @only linux
# @arg count: int
# @max 24
filter-linux-formats-count = { $count ->
    [one] { $count } format
   *[other] { $count } formats
}

# Status filter button when several statuses are set.
# @only linux
# @arg count: int
# @max 24
filter-linux-statuses-count = { $count ->
    [one] { $count } status
   *[other] { $count } statuses
}

# Search filter button, before a choice is made, and the first choice in its menu.
# @only linux
# @max 20
filter-linux-available = Available

# Choice in the Available menu that sets no filter.
# @only linux
# @max 24
filter-linux-available-any = Anywhere

# Choice in the Available menu, and the chip once set: titles the viewer's own sources have.
# @only linux
# @max 24
filter-linux-available-here = Plays here

# Choice in the Available menu, and the chip once set: titles on a licensed service the viewer
# added.
# @only linux
# @max 28
filter-linux-available-services = On your services

# Chip for a genre that is left out of the results. The genre is already named.
# @only linux
# @arg genre: text
# @max 40
filter-linux-not = Not { $genre }

# Button that takes off every filter chip.
# @only linux
# @max 24
filter-linux-clear-all = Clear all

# Tooltip of the arrow that scrolls the filter row back.
# @only linux
# @max 30
filter-linux-earlier = Earlier filters

# Tooltip of the arrow that scrolls the filter row on.
# @only linux
# @max 30
filter-linux-more = More filters

## Calendar

# What a screen reader says for a broadcast whose time is an estimate. The time is already formatted.
# @only linux
# @arg time: text
# @max 40
calendar-linux-about = about { $time }

# What a screen reader says for a broadcast that is the dub's.
# @only linux
# @max 24
calendar-linux-dubbed = { -dubbed }

## Library

# Title of the menu, and tooltip of its button, that orders a list.
# @only linux
# @max 20
library-linux-order = Order

# A list's tab: its name and how many titles it holds.
# @only linux
# @arg list: text
# @arg count: text
# @max 30
library-linux-list-count = { $list } { $count }

## Title page

# Description read by a screen reader on the button that plays the shown episode on a TV.
# @only linux
# @max 80
title-linux-play-on-tv-help = Play this episode on a TV with { -app-name }

# Description read by a screen reader on an episode's watched check box.
# @only linux
# @arg episode: int
# @max 40
title-linux-episode-watched = Episode { $episode } watched

# Line in a day's column when nothing airs that day.
# @only linux
calendar-linux-day-empty = Nothing airs

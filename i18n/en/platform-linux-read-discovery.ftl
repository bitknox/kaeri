# Strings only the Linux app shows, for Read's discovery rows and the sources' part of Search.

## Read's Home rows

# Screen reader name of a discovery row's View all button: the row's heading follows, so a
# list of links makes sense on its own.
# @only linux
# @arg row: text
read-linux-view-all-row = View all, { $row }

# Screen reader name of a discovery row's heading while its covers are on their way.
# @only linux
# @arg row: text
read-linux-row-loading = { $row }, loading

## Search, from your sources

# Said once by screen readers when Your sources only turns on.
# @only linux
read-linux-anilist-hidden = AniList's results are hidden.

# Said once by screen readers when every part of a search has answered. Sources that failed
# follow, each in its own sentence.
# @only linux
# @arg anilist: int
# @arg sources: int
read-linux-search-done = { $anilist } from AniList, { $sources } from your { -source-pl }.

# Said once by screen readers when every source has answered, with Your sources only on.
# @only linux
# @arg sources: int
read-linux-search-done-sources = { $sources } from your { -source-pl }.

# Screen reader name of a source's row heading while the source is being asked. The source's
# name and kind come first: "Northwind, Plugin".
# @only linux
# @arg source: text
read-linux-source-searching = { $source }, searching

# Screen reader name of the reason under a source that gave no answer in time, where the screen
# shows "No answer in 15 s."
# @only linux
# @arg seconds: int
read-linux-source-timeout = { $seconds ->
    [one] No answer in 1 second.
   *[other] No answer in { $seconds } seconds.
 }

# Screen reader name of Try again under a source that didn't answer, with the source's name, so
# each of several reads on its own.
# @only linux
# @arg source: text
read-linux-try-again-source = Try again, { $source }

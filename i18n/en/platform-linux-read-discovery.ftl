# Strings only the Linux app shows, for Read's discovery rows and the sources' part of Search.

## Read's Home rows

# Screen reader name of a discovery row's View all button: the row's heading follows, so a
# list of links makes sense on its own.
# @only linux
# @arg row: text
read-linux-view-all-row = View all, { $row }

# Screen reader name of See all on a row of reading's Search start: the row's heading follows,
# as for Read's rows.
# @only linux
# @arg row: text
read-linux-see-all-row = See all, { $row }

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

## Joining words

# Two facts on the line under a cover: "Manga · Releasing", "Japan · since March". Change the
# separator if the language sets such pairs another way.
# @only linux
# @arg first: text
# @arg second: text
# @max 60
read-linux-caption-pair = { $first } · { $second }

# A screen reader's list of a cover's facts, one more added to the ones before: "Manga, Japan",
# then "Manga, Japan, Releasing".
# @only linux
# @arg facts: text
# @arg fact: text
read-linux-spoken-facts = { $facts }, { $fact }

# What a screen reader says for a cover: one more sentence after the ones before. "Lantern
# Street Notes" and "Manga, Japan, Releasing" make "Lantern Street Notes. Manga, Japan,
# Releasing".
# @only linux
# @arg said: text
# @arg more: text
read-linux-spoken-then = { $said }. { $more }

# The end of what a screen reader says for a cover: the full stop after the last sentence.
# @only linux
# @arg said: text
read-linux-spoken-end = { $said }.

# A list of names before its last one, which "{ $first } and { $last }" adds: "Folders,
# Northwind" in "Folders, Northwind and Demo".
# @only linux
# @arg names: text
# @arg name: text
read-linux-names-more = { $names }, { $name }

# The filters and order a browse shows, one more added to the ones before: "Manga, Japan",
# then "Manga, Japan, Trending".
# @only linux
# @arg filters: text
# @arg filter: text
read-linux-browse-more = { $filters }, { $filter }

# The line over Search's results with AniList's count after it: "Webtoons, trending · 1,920
# series from AniList".
# @only linux
# @arg line: text
# @arg total: text
read-linux-line-total = { $line } · { $total }

# Said once by screen readers when a search has settled: the next sentence after the ones
# before, such as a source that didn't answer.
# @only linux
# @arg said: text
# @arg more: text
read-linux-said-then = { $said } { $more }

# A source's name with its kind, as a screen reader names the source's row: "Northwind,
# Plugin".
# @only linux
# @arg source: text
# @arg kind: text
read-linux-source-with-kind = { $source }, { $kind }

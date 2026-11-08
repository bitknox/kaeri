## Windows: reading's places. Home, Updates, Library, Search, Downloads and the book sources.

# Line under the title of the Downloads page for reading when no chapter is kept on this PC.
# @only windows
# @max 80
windows-read-downloads-empty-detail = Downloaded chapters open without a connection.

# Said once by Narrator when a search with words has heard from AniList and every source.
# Failed sources follow as their own sentences.
# @only windows
# @arg anilist: int
# @arg sources: int
windows-read-search-finished = { $anilist } from AniList, { $sources } from your { -source-pl }.

# Said once by Narrator when a search with Your sources only on has heard from every source.
# @only windows
# @arg sources: int
windows-read-search-finished-sources = { $sources } from your { -source-pl }.

# Said once by Narrator when Your sources only is turned on in Search.
# @only windows
windows-read-search-anilist-hidden = AniList's results are hidden.

# Narrator's words for a source that ran out of time in Search, read with its Try again.
# @only windows
# @arg seconds: int
windows-read-search-source-timeout-said = No answer in { $seconds ->
    [one] { $seconds } second
   *[other] { $seconds } seconds
}.

# Narrator's help on Search's filters while Your sources only is on.
# @only windows
windows-read-search-filters-dimmed = The filters apply to AniList's results only.

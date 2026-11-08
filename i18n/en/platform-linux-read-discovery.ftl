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

# Beside a source's name over its row of covers: what kind of source it is.
# @only linux
# @max 16
read-linux-source-folder = Folder

# Beside a source's name over its row of covers: a book server.
# @only linux
# @max 16
read-linux-source-server = Server

# Beside a source's name over its row of covers: a plugin.
# @only linux
# @max 16
read-linux-source-plugin = { -plugin-cap }

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

## Words only the Android app shows on reading's Home, Updates, and the reading side of Library
## and Search.

# Line under "You're offline" on reading's Home when it has never loaded, so nothing is kept to show.
# It has an Open Downloads button.
# @only android
read-home-offline-line = Reading's Home needs a connection the first time it opens. Downloaded chapters still open.

# Screen readers: a source group in Search while its source is still being asked. The source is
# its name.
# @arg source: text
# @only android
read-search-source-busy = { $source }, searching

# The same, with the kind of source it is (Server, Folder or the plugin word).
# @arg source: text
# @arg kind: text
# @only android
read-search-source-busy-kind = { $source }, { $kind }, searching

# Screen readers: a source group's heading in Search, its name and the kind of source it is.
# @arg source: text
# @arg kind: text
# @only android
read-search-source-named = { $source }, { $kind }

# Screen readers: the reason under a source group whose source gave no answer in time.
# @arg seconds: int
# @only android
read-search-source-timeout-spoken = { $seconds ->
    [one] No answer in { $seconds } second.
   *[other] No answer in { $seconds } seconds.
 }

# Screen readers: the Try again button of one source group in Search. The source is its name.
# @arg source: text
# @only android
read-search-try-again-source = Try again, { $source }

# The line under "Nothing found in your sources" when Your sources only is on.
# @only android
read-search-sources-only-nothing-line = Try other words.

# Screen readers: a reading cover with no line under its title.
# @arg title: text
# @only android
read-tile-spoken-title = { $title }.

# Screen readers: a reading cover, its title and the line under it.
# @arg title: text
# @arg line: text
# @only android
read-tile-spoken = { $title }. { $line }.

# Screen readers: a reading cover, its title, the line under it and where the person has it.
# @arg title: text
# @arg line: text
# @arg where: text
# @only android
read-tile-spoken-in = { $title }. { $line }. { $where }.

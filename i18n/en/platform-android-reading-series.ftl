## Words only the Android app shows on a reading series page, its sheets, Send to e-reader and
## the reading part of Downloads.

# On a series page whose source failed, under the cause: the chapters downloaded before still open.
# @only android
# @arg n: int
read-series-downloaded-open = { $n ->
    [one] One downloaded chapter still opens.
   *[other] { $n } downloaded chapters still open.
}

# Downloads' empty line for a person who only reads, under "No downloads".
# @only android
read-downloads-empty-detail = Downloaded chapters open without a connection.

# Downloads' empty line for a person who watches and reads, under "No downloads".
# @only android
read-downloads-empty-both = Downloaded episodes and chapters open without a connection.

# The head of the group menu on a large screen, over its choices: the source the groups are in.
# @only android
# @arg source: text
# @max 40
group-menu-in = Group in { $source }

## Words only the Android app shows on a reading series page, its sheets, Send to e-reader and
## the reading part of Downloads.

# On a series page whose source failed, under the cause: the chapters downloaded before still open.
# @only android
# @arg n: int
read-series-downloaded-open = { $n ->
    [one] One downloaded chapter still opens.
   *[other] { $n } downloaded chapters still open.
}

# The head of the group menu on a large screen, over its choices: the source the groups are in.
# @only android
# @arg source: text
# @max 40
group-menu-in = Group in { $source }

# What TalkBack says the Group row does, after "Double-tap to": an imperative, as Android's
# click labels are.
# @only android
# @max 24
series-group-action = Pick a group

# E-ink series page on a large panel: the text button beside the chapter order that starts
# choosing chapters.
# @only android
# @max 16
series-select = Select

# E-ink series page on a large panel: the label over the person's own score, shown as "8 / 10".
# @only android
# @max 20
series-fact-score = Your score

# E-ink series page on a large panel: the label over the series' status, "Releasing".
# @only android
# @max 20
series-fact-status = Status

# E-ink series page on a large panel: the label over how often chapters come out, "Usually
# Fridays".
# @only android
# @max 20
series-fact-releases = New chapters

# E-ink series page on a large panel: the state at the end of an unread chapter row that is
# downloaded here.
# @only android
# @max 24
series-row-on-device = On this { -device }

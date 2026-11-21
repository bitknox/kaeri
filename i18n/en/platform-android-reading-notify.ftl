## Text only the Android phone and tablet apps show in their reading notifications and the
## system's notification channels for them.

# Name of the notification channel for new chapters, shown in the system's settings.
# @only android
# @max 40
channel-chapters-name = New chapters

# What the new chapters channel is for, shown in the system's settings.
# @only android
channel-chapters-description = When a series on your Reading list has a new chapter.

# Name of the notification channel that says chapter downloads are done, shown in the system's settings.
# @only android
# @max 40
channel-downloads-finished-name = Downloads finished

# What the downloads finished channel is for, shown in the system's settings.
# @only android
channel-downloads-finished-description = When the chapters you asked to download are all done.

# The download notification while only chapters download.
# @only android
# @arg count: int
downloads-notice-chapters = { $count ->
    [one] Downloading one chapter
   *[other] Downloading { $count } chapters
 }

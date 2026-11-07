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

# Notification, new chapter: the line under the series' title. The chapter is the source's own
# words for it, such as "Chapter 12".
# @only android
# @arg chapter: text
notif-chapter-new = { $chapter } is new.

# Notification, new chapters: the line under the series' title when one series has several.
# @only android
# @arg count: int
notif-chapters-new = { $count ->
    [one] One new chapter
   *[other] { $count } new chapters
 }

# Notification, new chapter: what the lock screen shows when previews are hidden.
# @only android
notif-chapter-hidden = New chapter

# Notification, new chapter: the action that opens the reader on the chapter.
# @only android
# @max 16
notif-chapter-read = Read

# The download notification while only chapters download.
# @only android
# @arg count: int
downloads-notice-chapters = { $count ->
    [one] Downloading one chapter
   *[other] Downloading { $count } chapters
 }

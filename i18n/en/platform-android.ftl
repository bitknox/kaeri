# Name of the notification channel for download progress, shown in the system's settings.
channel-downloads-name = Downloads

# Under a release in the source picker: how many people share it right now.
# @only android
# @arg count: int
# @max 24
release-seeders = { $count ->
    [one] One seeder
   *[other] { $count } seeders
 }

# Heading in the trailer sheet when the video did not start within a few seconds or the player refused it.
# @only android
# @max 40
trailer-failed = Couldn't open the trailer

# Heading on the screen that covers the app when its engine did not start or has stopped.
# @only android
# @max 40
engine-down-title = { -app-name } stopped

# Line under it when the engine stopped while the app was open, so lists would sit empty.
# @only android
# @max 120
engine-down-stopped = Nothing new will load until it starts again. Try again, or quit and open the app once more.

# Button on that screen that closes the app.
# @only android
# @max 16
engine-down-quit = Quit

# Words in place of the Picture in picture when leaving the app switch, when Android's own per-app Picture in picture permission is off. An Open settings button sits beside it.
# @only android
# @max 32
settings-picture-in-picture-system-off = Off in Android settings

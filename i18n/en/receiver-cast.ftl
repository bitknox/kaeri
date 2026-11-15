## Kaeri's Cast receiver: the page a TV with Chromecast built in shows while connected. It is
## built into a JSON file per language on the website, and the sender names the language. Every
## line is read from across a room at TV size, so keep them short. Lines that name the device
## that casts come in one message per kind of device: iPhone, iPad, Mac, phone and tablet (Android)
## and computer (Windows and Linux).

# The TV while connected with nothing playing, under the app's wordmark. Shown for up to ten minutes.
# @max 60
receiver-ready-iphone = Ready. Choose an episode on the iPhone.

# The TV while connected with nothing playing, under the app's wordmark. Shown for up to ten minutes.
# @max 60
receiver-ready-ipad = Ready. Choose an episode on the iPad.

# The TV while connected with nothing playing, under the app's wordmark. Shown for up to ten minutes.
# @max 60
receiver-ready-mac = Ready. Choose an episode on the Mac.

# The TV while connected with nothing playing, under the app's wordmark. Shown for up to ten minutes.
# @max 60
receiver-ready-phone = Ready. Choose an episode on the phone.

# The TV while connected with nothing playing, under the app's wordmark. Shown for up to ten minutes.
# @max 60
receiver-ready-tablet = Ready. Choose an episode on the tablet.

# The TV while connected with nothing playing, under the app's wordmark. Shown for up to ten minutes.
# @max 60
receiver-ready-computer = Ready. Choose an episode on the computer.

# Beside a spinner while an episode starts, under the show and episode.
# @max 24
receiver-starting = Starting

# At the top of the paused picture.
# @max 24
receiver-paused = Paused

# At the bottom of the picture while the stream stalls; the count goes up each second.
# @arg seconds: int
# @max 44
receiver-waiting = Waiting for the stream · { $seconds } s

# Title of a card on the paused picture when the device that sends the episode stopped answering.
# @max 36
receiver-sender-gone-iphone = Waiting for the iPhone

# Title of a card on the paused picture when the device that sends the episode stopped answering.
# @max 36
receiver-sender-gone-ipad = Waiting for the iPad

# Title of a card on the paused picture when the device that sends the episode stopped answering.
# @max 36
receiver-sender-gone-mac = Waiting for the Mac

# Title of a card on the paused picture when the device that sends the episode stopped answering.
# @max 36
receiver-sender-gone-phone = Waiting for the phone

# Title of a card on the paused picture when the device that sends the episode stopped answering.
# @max 36
receiver-sender-gone-tablet = Waiting for the tablet

# Title of a card on the paused picture when the device that sends the episode stopped answering.
# @max 36
receiver-sender-gone-computer = Waiting for the computer

# Under the waiting card's title.
# @max 60
receiver-sender-gone-hint-iphone = Open { -app-name } on the iPhone to keep watching.

# Under the waiting card's title.
# @max 60
receiver-sender-gone-hint-ipad = Open { -app-name } on the iPad to keep watching.

# Under the waiting card's title.
# @max 60
receiver-sender-gone-hint-mac = Open { -app-name } on the Mac to keep watching.

# Under the waiting card's title.
# @max 60
receiver-sender-gone-hint-phone = Open { -app-name } on the phone to keep watching.

# Under the waiting card's title.
# @max 60
receiver-sender-gone-hint-tablet = Open { -app-name } on the tablet to keep watching.

# Under the waiting card's title.
# @max 60
receiver-sender-gone-hint-computer = Open { -app-name } on the computer to keep watching.

# Title of the card when an episode can't play. The show's name is under it.
# @arg episode: plain
# @max 40
receiver-error = Couldn't play episode { $episode }

# Title of the card when an episode without a number can't play.
# @max 40
receiver-error-unnumbered = Couldn't play this episode

# Last line of the card when an episode can't play.
# @max 60
receiver-error-hint-iphone = Choose another { -source } on the iPhone.

# Last line of the card when an episode can't play.
# @max 60
receiver-error-hint-ipad = Choose another { -source } on the iPad.

# Last line of the card when an episode can't play.
# @max 60
receiver-error-hint-mac = Choose another { -source } on the Mac.

# Last line of the card when an episode can't play.
# @max 60
receiver-error-hint-phone = Choose another { -source } on the phone.

# Last line of the card when an episode can't play.
# @max 60
receiver-error-hint-tablet = Choose another { -source } on the tablet.

# Last line of the card when an episode can't play.
# @max 60
receiver-error-hint-computer = Choose another { -source } on the computer.

# Under the show's name after the last episode played, or after a Stop at the very end.
# @arg episode: plain
# @max 40
receiver-finished = Finished episode { $episode }

# Under the show's name after an episode without a number played to the end.
# @max 40
receiver-finished-unnumbered = Finished

# Shown once for four seconds when the TV can't keep up with typeset subtitles and changes to
# plain ones.
# @max 60
receiver-plain-subs = Subtitles show as plain text on this TV.

# On the card that shows the next episode while it counts down. The seconds are whole.
# @arg seconds: int
# @max 36
receiver-up-next = { -up-next-cap } · plays in { $seconds } s

# The next episode on the Up next card, under the count.
# @arg episode: plain
# @arg title: text
receiver-up-next-titled = Episode { $episode } · { $title }

# The next episode on the Up next card when it has no title of its own.
# @arg episode: plain
# @max 30
receiver-up-next-numbered = Episode { $episode }

# A note at the top for three seconds after the opening was skipped.
# @max 30
receiver-skipped-opening = Skipped opening

# A note at the top for three seconds after the ending was skipped.
# @max 30
receiver-skipped-ending = Skipped ending

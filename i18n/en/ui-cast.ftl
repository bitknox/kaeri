## Casting to a TV with Chromecast built in, from the Play on TV picker and its now-playing card.
## The engine's notices about a cast are in engine-cast.ftl.

# Heading of the picker's group of TVs with Chromecast built in, and the line under such a TV
# when it fetches the stream by itself. A product name.
cast-chromecast = Chromecast

# Line added to the picker's empty state, under what to check.
cast-none-found-extra = TVs with Chromecast built in show here too.

# The picker's one row while the app may not look on the local network, with a button that
# opens the system's settings.
cast-local-network = Allow { -app-name } to find { -device-pl } on your local network

# Settings row: whether the TV picker lists TVs with Chromecast built in.
cast-tvs-label = Show TVs with Chromecast built in

# Settings row: whether a cast to a TV plays from torrents, which this device serves meanwhile.
cast-torrents-label = Torrents while casting

# Note under the Settings row that shows TVs with Chromecast built in, on a Mac.
cast-tvs-note-mac = They appear under Chromecast in Play on TV. This Mac sends them the episode.

# Note under the Settings row that shows TVs with Chromecast built in, on an iPhone.
cast-tvs-note-iphone = They appear under Chromecast in Play on TV. This iPhone sends them the episode.

# Note under the Settings row that shows TVs with Chromecast built in, on an iPad.
cast-tvs-note-ipad = They appear under Chromecast in Play on TV. This iPad sends them the episode.

# Note under the Settings row that lets a cast play torrents, on a Mac.
cast-torrents-note-mac = A TV with Chromecast built in can then play torrents. This Mac sends the episode for as long as it plays.

# Note under the Settings row that lets a cast play torrents, on an iPhone.
cast-torrents-note-iphone = A TV with Chromecast built in can then play torrents. This iPhone sends the episode for as long as it plays.

# Note under the Settings row that lets a cast play torrents, on an iPad.
cast-torrents-note-ipad = A TV with Chromecast built in can then play torrents. This iPad sends the episode for as long as it plays.

# Line on the casting card while the next episode counts down. The seconds are whole.
# @arg seconds: int
cast-up-next-plays-in = { -up-next-cap } · plays in { $seconds } s

# The card's line when a cast never started. The name is the TV's.
# @arg name: text
cast-couldnt-start = Couldn't start on { $name }

# The card's line when a cast that played has stopped.
cast-stopped = Casting stopped

# Button that stops the cast and plays the episode on this device, where the TV was.
# @max 20
cast-play-here = Play here

# Button that stops the cast. The TV goes back to its own screen.
# @max 24
cast-stop = Stop casting

# Line under TV volume on the card, for a TV whose volume only its own remote sets.
cast-volume-fixed = Set on the TV

## The cast button, its dialogs, the mini player over the navigation bar and the full controller.

# The cast button's label and tooltip while it connects to a TV. The name is the TV's own, such
# as Living room.
# @arg name: text
cast-button-connecting = Connecting to { $name }

# The cast button's label and tooltip while a TV is connected, said once as it connects.
# @arg name: text
cast-button-connected = Connected to { $name }

# Under the main action of a title page while a TV is connected: where the episode plays.
# @arg name: text
cast-plays-on = Plays on { $name }

# The connected TV's dialog while the TV shows the app and nothing is loaded.
cast-idle = Nothing playing yet

# The controller's status line while the episode is paused on the TV.
# @arg name: text
cast-status-paused = Paused on { $name }

# Screen reader hint and tooltip on the show's name and cover in the controller, which open
# the show's page.
# @arg title: text
cast-open-show = Open { $title }

# The chevron that puts the full controller back into the mini player.
# @max 20
cast-controller-close = Close

# Screen reader hint on the mini player over the navigation bar.
cast-mini-opens = Opens the controls

# Long-press menu of an episode while a TV plays: queues the episode after the one playing.
# @arg name: text
cast-play-next-on = Play next on { $name }

# The system's media notification and Now Playing: the line naming the TV that plays.
# @arg name: text
cast-notification-line = Casting to { $name }

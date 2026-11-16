## Apple only: casting lines that name the kind of device this is (Mac, iPhone or iPad). The
## shared casting words are in ui-cast.ftl.

# Line under a TV with Chromecast built in, in the picker: this device serves the episode to it
# while it plays. The device is the product's name, such as Mac or iPhone.
# @arg device: text
cast-row-through = Plays through this { $device }

# Title of the Mac's confirmation when the app quits while it casts. The name is the TV's.
# @arg name: text
cast-quit-title = Stop casting to { $name }?

# Line in that confirmation: the Mac serves the episode, so quitting ends it on the TV.
# @arg name: text
cast-quit-body = Quitting { -app-name } stops the episode on { $name }.

# The confirmation's button that stops casting and quits.
# @max 28
cast-quit-action = Stop and quit

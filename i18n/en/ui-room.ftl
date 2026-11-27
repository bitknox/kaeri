## The room: watching together over relays. One room per show and episode, with a line that
## says what is happening, a face and a state word per person, the chat, and the invite. Names
## are what each person calls themselves. Positions in the episode (`at`) come already formatted,
## such as 12:04. Lists of names come already joined, such as "Mika and Ada".

## The room's line: a bold first line and a quieter second line under it, in the room column,
## the room on a phone, the TV rail and the bar a phone folds the room into.

# First line while the room plays and nobody holds it.
# @max 40
room-line-watching = Watching together

# Second line while the room plays: the room's episode and its place in it.
# @arg episode: text
# @arg at: text
room-line-watching-detail = Episode { $episode } · { $at }

# First line while the room waits for people whose player is opening the episode or stalled. `names` is one name, two joined, or room-names-people.
# @arg names: text
# @max 40
room-line-paused-for = Paused for { $names }

# First line on your own device while the room waits for you.
# @max 40
room-line-paused-for-you = Paused for you

# Second line while the room waits for you: who waits, and for how many seconds so far.
# @arg names: text
# @arg count: int
# @arg seconds: int
room-line-waiting-for-you = { $count ->
    [one] { $names } is waiting · { $seconds } s
   *[other] { $names } are waiting · { $seconds } s
}

# First line once the room has waited as long as it will (20 seconds), when it waits for anyone opening the episode, or for people with different reasons (room-line-still-seeking and room-line-still-buffering are the others). `count` is how many people it waits for.
# @arg names: text
# @arg count: int
# @max 40
room-line-still-loading = { $count ->
    [one] { $names } is still loading
   *[other] { $names } are still loading
}

# First line once the room has waited as long as it will, when everyone it waits for is still moving to the room's new place after a seek. `count` is how many people it waits for.
# @arg names: text
# @arg count: int
# @max 40
room-line-still-seeking = { $count ->
    [one] { $names } is still seeking
   *[other] { $names } are still seeking
}

# First line once the room has waited as long as it will, when everyone it waits for stalled mid-episode. `count` is how many people it waits for.
# @arg names: text
# @arg count: int
# @max 40
room-line-still-buffering = { $count ->
    [one] { $names } is still buffering
   *[other] { $names } are still buffering
}

# Second line once the room has waited as long as it will.
# @arg seconds: int
room-line-waited = The room has waited { $seconds } s

# Second line after someone pressed Carry on, when the room waited for one person.
# @arg carrier: text
# @arg name: text
room-line-carried-on-one = { $carrier } carried on · { $name } joins when ready

# Second line after someone pressed Carry on, when the room waited for more than one person.
# @arg carrier: text
room-line-carried-on-many = { $carrier } carried on · they join when ready

# First line while the room is paused by someone.
# @arg name: text
# @max 40
room-line-paused-by = { $name } paused

# First line while the room is paused by you.
# @max 40
room-line-you-paused = You paused

# Second line while the room is paused: the place it paused at.
# @arg at: text
room-line-paused-at = at { $at }

# Second line while one of the room's relays is not answering.
# @arg count: int
# @arg total: int
room-line-relays-down = { $count ->
    [one] { $count } of { $total } relays isn't answering · the room carries on
   *[other] { $count } of { $total } relays aren't answering · the room carries on
}

# First line when your own device lost every relay.
# @max 40
room-line-reconnecting = Reconnecting to the room

# Second line when your own device lost every relay.
room-line-reconnecting-detail = No relay is answering · your episode keeps playing

# Second line when your own player is behind the room.
# @arg seconds: int
room-line-you-behind = You're { $seconds } s behind · catching up

# The room's line in the full screen strip, with the seconds the room has waited. `line` is one of the room's first lines.
# @arg line: text
# @arg seconds: int
room-line-with-seconds = { $line } · { $seconds } s

# Stands for the people the room waits for when there are more than two, inside "Paused for …".
# @arg count: int
room-names-people = { $count ->
    [one] { $count } person
   *[other] { $count } people
}

## People and their states. Each person has a face with a ring and one short word under or
## beside it. The words show in the room column, on phones under the faces, on the TV rail and
## in the place's room card.

# State word: in step with the room and playing.
# @max 20
room-state-watching = Watching

# State word before play: this person's first frame is ready.
# @max 20
room-state-ready = Ready

# State word: the room is paused and this person paused it.
# @max 20
room-state-paused-it = Paused it

# State word: the room is paused; this person was watching.
# @max 20
room-state-paused = Paused

# State word: the slowest player, holding the room back, with the gap in seconds.
# @arg seconds: int
# @max 24
room-state-behind = Behind · { $seconds } s

# State word: the player is opening the episode and the room waits, with the seconds so far. Also the room's second line while it waits for one loading person.
# @arg seconds: int
# @max 24
room-state-loading = Loading · { $seconds } s

# State word: the player stalled mid-episode, with the seconds so far. Also the room's second line while it waits for one buffering person.
# @arg seconds: int
# @max 24
room-state-buffering = Buffering · { $seconds } s

# State word: this person plays another episode than the room's.
# @arg episode: text
# @max 24
room-state-other-episode = On episode { $episode }

# State word: this person is picking which source to play from.
# @max 24
room-state-choosing = Choosing a { -source }

# State word: none of this person's sources lists the room's episode.
# @arg episode: text
# @max 36
room-state-no-source = No { -source } for episode { $episode }

# State word: this person is in the room with the player closed.
# @max 20
room-state-player-closed = Player closed

# State word: this person's device stopped answering.
# @max 20
room-state-reconnecting = Reconnecting

# State word: this person left the room. The row stays dimmed for a moment.
# @max 20
room-state-left = Left

# State word: you muted this person. Shown alone or after the state word with room-state-and-muted.
# @max 20
room-state-muted = Muted

# A state word followed by Muted, for a person you muted.
# @arg state: text
room-state-and-muted = { $state } · Muted

# Marker after your own name in the people list.
# @max 12
room-you-marker = (you)

# Your own name under your face on a phone and in the place's room card.
# @max 12
room-you = You

# You, inside a list of names mid-sentence: "Paused for Ren and you", "Mika carried on without
# you". Lowercase where the language writes it so.
# @max 12
room-you-in-list = you

# Accessibility label of a face in the full screen strip: the name and the state word.
# @arg name: text
# @arg state: text
room-face-label = { $name }, { $state }

# The last face in a row of faces, standing for the people not shown.
# @arg count: int
# @max 8
room-more-faces = +{ $count }

# Accessibility label of the last face, which opens the full people list.
# @arg count: int
room-more-faces-label = { $count ->
    [one] { $count } more person
   *[other] { $count } more people
}

# Name under the last face on a phone and the TV: it opens everyone.
# @max 12
room-more = More

# State under the last face: how many of the hidden people are not watching.
# @arg count: int
# @max 24
room-more-not-watching = { $count } not watching

# State under the last face when everyone hidden is watching.
# @max 20
room-more-all-watching = All watching

## The wait and Carry on. The room waits for someone opening the episode or stalled, up to 20
## seconds, then offers to carry on without them. In a capsule over the video, in the room's
## line and in the full screen strip.

# Reason in the waiting capsule, after Paused for, lower case: their player is opening the episode.
# @arg seconds: int
room-wait-loading = loading · { $seconds } s

# Reason in the waiting capsule, after Paused for, lower case: their player stalled.
# @arg seconds: int
room-wait-buffering = buffering · { $seconds } s

# Button once the room has waited 20 seconds for one person.
# @arg name: text
# @max 28
room-carry-on-without = Carry on without { $name }

# Button once the room has waited 20 seconds for more than one person.
# @max 28
room-carry-on-without-them = Carry on without them

# Capsule over the video after someone pressed Carry on.
# @arg names: text
room-carrying-on-without = Carrying on without { $names }

# Second part of the capsule after Carry on, when the room waited for one person.
# @arg name: text
room-joins-at-place = { $name } joins at the room's place

# Second part of the capsule after Carry on, when the room waited for more than one person.
room-join-at-place-many = They join at the room's place

# Line on your own starting screen while the room waits for you, with the seconds so far.
# @arg seconds: int
room-paused-until-start = The room is paused until this starts · { $seconds } s

# Button on your own starting screen that lets the room carry on without you.
# @max 28
room-dont-wait = Don't wait for me

## Before play. The room is open and the episode shows in the video area until someone presses
## Play for everyone. Ready is automatic.

# First line before anyone pressed Play.
# @max 40
room-line-not-started = Not started yet

# Second line before anyone pressed Play.
room-line-play-starts = Play starts it for everyone

# First line after Play, while the room waits for people still loading.
# @arg names: text
# @arg count: int
# @max 40
room-line-starting-when = { $count ->
    [one] Starting when { $names } is ready
   *[other] Starting when { $names } are ready
}

# First line counting down once everyone is ready.
# @arg seconds: int
# @max 40
room-line-starting-in = Starting in { $seconds }

# Second line when everyone is ready.
room-line-everyone-ready = Everyone is ready

# Button once the room has waited 20 seconds before play, for one person.
# @arg name: text
# @max 28
room-start-without = Start without { $name }

# Button once the room has waited 20 seconds before play, for more than one person.
# @max 28
room-start-without-them = Start without them

# Small heading over the episode in the video area before play.
room-up-in-room = Up in the room

# Small heading over the episode in the video area when you opened an invite.
room-room-is-watching = The room is watching

# Line under the show's title in the video area: episode number, episode title and length. The length is already formatted, such as 24 min.
# @arg episode: text
# @arg title: text
# @arg length: text
room-banner-episode = Episode { $episode } · { $title } · { $length }

# Line under the episode before play: one of your sources has it.
room-plays-from-sources = Plays from your { -source-pl }

# Button that starts the episode for everyone in the room.
# @max 28
room-play-for-everyone = Play for everyone

# Line beside Play for everyone.
room-anyone-can-start = Anyone in the room can start it

# Label of the Play for everyone button after it was pressed.
# @max 28
room-playing = Playing…

# Line beside the button after Play was pressed, on a desktop.
room-waiting-first-frames = Waiting for everyone's first frame

# Line under the play button after Play was pressed, on a phone.
# @max 32
room-waiting-first-frames-short = Waiting for first frames

# First line of the room when an invite link opened it.
# @arg name: text
# @max 40
room-invited-by = { $name } invited you

# Second line when an invite link opened the room: the show and the episode.
# @arg title: text
# @arg episode: text
room-invited-detail = { $title } · Episode { $episode }

# Second line when an invite link opened a room with 10 or more people in it.
# @arg count: int
room-invited-over = { $count } people are here already · rooms work best with 10 or fewer

# Button that joins the room from an invite.
# @max 24
room-join = Join the room

# Button that joins a room with 10 or more people in it.
# @max 24
room-join-anyway = Join anyway

## Your own player has nothing to show: choosing a source, or none has the episode. In the
## video area.

# Heading over the list of sources for the room's episode.
# @arg episode: text
room-choose-source = Choose a { -source } for episode { $episode }

# Line under it.
room-choose-source-line = The room keeps watching; you join at its place.

# Button that plays the chosen source at the room's place.
# @max 24
room-play-in-room = Play in the room

# Heading when none of your sources has the room's episode.
# @arg episode: text
room-no-source-title = None of your { -source-pl } has episode { $episode }

# Line under it on a desktop: how many sources were looked in.
# @arg count: int
room-no-source-line = { $count ->
    [one] { -app-name } looked in your one { -source }. You stay in the room and the chat while the others watch.
   *[other] { -app-name } looked in your { $count } { -source-pl }. You stay in the room and the chat while the others watch.
}

# Line under it on a phone.
room-no-source-line-short = You stay in the room and the chat.

# Button that opens your sources.
# @max 24
room-open-sources = Open { -source-pl }

# Button that searches titles in your sources.
# @max 24
room-search-titles = Search titles

## The room's header and buttons.

# Line under the show's title on a phone and in the Linux header bar: the episode and how many people are in the room.
# @arg episode: text
# @arg count: int
room-header-line = Episode { $episode } · { $count } in the room

# Button and accessibility label that opens the invite.
# @max 16
room-invite = Invite

# Button and accessibility label that opens the room's details.
# @max 16
room-details = Details

# Accessibility label of the More button on a phone.
# @max 16
room-more-actions = More

# Accessibility label of the player's button that hides the room column.
room-hide = Hide room

# Accessibility label of the chevron on a phone that folds the room into the bar over the tabs.
room-fold = Close the room to the bar

# Accessibility label of the player's button that opens the chat in full screen.
# @max 16
room-chat = Chat

## Invite and New invite.

# Line in the invite: anyone with the link can join.
room-invite-anyone = Anyone with this link can join.

# Line after it on a desktop: the link carries the key.
room-invite-key = The link carries the room's key, so only people who have it can read the room.

# Line after it on a phone.
room-invite-key-short = Only people with the link can read the room.

# Accessibility label of the invite's QR code.
room-invite-qr = Invite as a QR code

# Line beside the QR code.
room-invite-qr-line = Someone beside you can scan this with their camera.

# Button that copies the invite link.
# @max 24
room-copy-link = Copy link

# Button that opens the system's share sheet with the invite link.
# @max 24
room-share = Share…

# Menu item on a phone that copies the invite link.
# @max 28
room-copy-invite-link = Copy invite link

# Button that makes a new invite link and stops the old one.
# @max 24
room-new-invite = New invite

# Line under New invite on a desktop.
room-new-invite-line = New invite stops the old link working. Everyone in the room moves to the new one.

# Short line beside New invite on a phone.
# @max 28
room-new-invite-short = Stops the old link

## Move to a new room: for a stranger already in the room. Anyone starts it and ticks who
## comes along; each person ticked chooses to go.

# Button in the invite and menu item on a phone that opens the move.
# @max 28
room-move-open = Move to a new room…

# Line under the button in the invite.
room-move-open-line = For someone already here: start a new room and pick who comes along.

# Title of the move dialog and sheet.
room-move-title = Move to a new room

# Line under the title.
room-move-body = Everyone you tick is asked to go along. Nobody moves without saying yes, and anyone not ticked stays in this room.

# Note under your own row: you always go.
room-move-always-goes = Always goes

# Note under a person you muted, who starts unticked.
room-move-muted = Muted, so not ticked

# Line under the list.
# @arg at: text
room-move-foot = The new room picks up at { $at } with a new key. Its link reaches only the people ticked.

# Button that moves, counting the others ticked.
# @arg count: int
# @max 28
room-move-with = { $count ->
    [one] Move with { $count } other
   *[other] Move with { $count } others
}

# The offer to someone ticked, in a notice or alert, as one sentence pair.
# @arg name: text
room-move-offer = { $name } moved the room. Go along?

# First line of the room while the offer waits for an answer.
# @arg name: text
# @max 40
room-line-moved = { $name } moved the room

# Second line with the offer: who else is asked, and the place the episode carries on from.
# @arg names: text
# @arg count: int
# @arg at: text
room-line-moved-detail = { $count ->
    [one] Go along? { $names } is asked too. The episode carries on from { $at }.
   *[other] Go along? { $names } are asked too. The episode carries on from { $at }.
}

# Second line with the offer when nobody else is asked.
# @arg at: text
room-line-moved-detail-alone = Go along? The episode carries on from { $at }.

# Button that goes along to the new room.
# @max 24
room-go-along = Go along

# Short button that stays in this room, in a notice.
# @max 16
room-stay = Stay

# First line of the old room for someone who stayed.
# @arg name: text
# @max 40
room-line-moved-away = { $name } moved to a new room

# Second line of the old room: who went along.
# @arg names: text
room-line-went-along = { $names } went along · this room carries on

## Details: the room's encryption and relays.

# Label of the encryption row.
room-details-encryption = Encryption

# Value of the encryption row.
room-details-end-to-end = End to end

# Label of the relays row, and the title of the relays page in Settings.
room-relays = Relays

# Value of the relays row when all answer.
# @arg count: int
room-relays-answering = { $count } answering

# Value of the relays row when some don't answer.
# @arg count: int
# @arg total: int
room-relays-answering-of = { $count } of { $total } answering

# State of a relay that doesn't answer.
# @max 20
room-relay-not-answering = Not answering

# Round trip time of a relay.
# @arg ms: int
room-relay-latency = { $ms } ms

# Line under the relays in Details.
room-details-relays-line = Relays pass the room's messages along and can't read them. These are public relays run by others; { -app-name } doesn't run any of them. If one stops answering, the room swaps in a spare.

# Button in Details that edits the room's relays.
# @max 24
room-edit-relays = Edit relays…

# Title of the room's relay editor.
room-edit-relays-title = Edit relays

# Line under it.
room-edit-relays-line = Changes this room's relays for everyone in it. New invites carry the list.

# Accessibility label of the button that removes a relay. `relay` is its address.
# @arg relay: text
room-relay-remove = Remove { $relay }

# Button that adds the address typed beside it.
# @max 16
room-relay-add = Add

# Line under the room's relay editor.
room-edit-relays-settings = Settings › Playback › Relays sets the list for new rooms.

## Settings › Playback › Watch together, and its Relays page.

# Heading of the group in Settings › Playback.
room-settings-group = { -watch-together-cap }

# Switch that hides chat in every room on this device.
room-settings-hide-chat = Hide chat in rooms

# Line under it.
room-settings-hide-chat-line = Chat lines, reactions and their ticks on the seek bar stay off the video, and the chat field is gone. The people list still shows who is in the room.

# Line under the Relays row: how many relays new rooms use.
# @arg count: int
room-settings-relays-count = { $count ->
    [one] { $count } public relay run by others
   *[other] { $count } public relays run by others
}

# Button at the end of the Relays row.
# @max 16
room-settings-relays-edit = Edit

# Line under the group.
room-settings-group-line = Each of you plays the episode from your own { -source-pl }; relays only carry the room's messages, sealed so they can't read them.

# Line at the top of the Relays page on a desktop.
room-relays-intro = Rooms you start use these relays, and your invites carry them. Relays pass the room's messages along and can't read them. These are public relays run by others; { -app-name } doesn't run any of them. A room's own list is in its Details.

# Line at the top of the Relays page on a phone.
room-relays-intro-short = Rooms you start use these relays, and your invites carry them. They can't read the room. These are public relays run by others.

# Heading of the list of relays for new rooms.
room-relays-new-rooms = For new rooms

# Line under the list for new rooms.
room-relays-new-rooms-line = { -app-name } sends every room event to all of them. Plain ws:// works only for localhost and .local addresses.

# Heading of the list of spare relays.
room-relays-spares = Spares

# Line under the spares on a desktop.
room-relays-spares-line = When a room has fewer than two relays answering, it swaps in the first spare that answers.

# Line under the spares on a phone.
room-relays-spares-line-short = Used when a room has fewer than two relays answering.

# Button that puts the default relays and spares back.
# @max 28
room-relays-restore = Restore the defaults

## Leave and the confirmations.

# Button in the room column's header that leaves, on the place's room card, with the dialog after it.
# @max 16
room-leave-ellipsis = Leave…

# Menu item on an iPhone that leaves, asking first.
# @max 28
room-leave-menu = Leave the room…

# Menu item on Android that leaves, asking first.
# @max 28
room-leave-menu-plain = Leave the room

# Title of the confirmation before leaving.
room-leave-title = Leave the room?

# Line under it.
room-leave-line = The others keep watching.

# Line under it when closing the window on a Mac, which offers Keep the room.
room-leave-line-keep = The others keep watching. Keep the room closes the player and keeps your place.

# Button in the confirmation on closing the window: close the player and stay in the room.
# @max 24
room-keep = Keep the room

# Button in the TV's confirmation that goes back to the room.
# @max 24
room-keep-watching = Keep watching

## Up next and Skip opening are the room's: they move everyone.

# Small heading of the up next card.
room-up-next = { -up-next-cap } for the room

# Line on the card naming the next episode.
# @arg episode: text
# @arg title: text
room-up-next-episode = Episode { $episode } · { $title }

# Line on the card naming the next episode when it has no title.
# @arg episode: text
room-up-next-episode-plain = Episode { $episode }

# Line on the card counting down for everyone.
# @arg seconds: int
room-up-next-count = Plays for everyone in { $seconds } s

# Line on the card while your file ended sooner and the room waits for someone's longer file.
# @arg name: text
room-up-next-waiting = Waiting for { $name } to finish

# Button on the card that moves the room on at once.
# @max 24
room-play-next-now = Play next now

# First line of the room while up next counts down.
# @arg seconds: int
# @max 40
room-line-up-next = { -up-next-cap } in { $seconds } s

# Second line while up next counts down.
# @arg episode: text
room-line-up-next-detail = Episode { $episode } plays for everyone

## Chat in the room: the conversation, the room's own lines in it, and reactions.

# Placeholder of the chat field.
# @max 24
room-chat-field = Say something

# Button and accessibility label that sends a chat line.
# @max 16
room-chat-send = Send

# Accessibility label of the button beside the chat field that opens the reactions.
# @max 16
room-react = React

# Line in the chat of a room opened from an invite, before joining.
room-chat-opens = The chat opens when you join

# Line under a chat line that didn't reach any relay.
# @max 24
room-chat-not-sent = Couldn't send

# Chat line: someone started the room.
# @arg name: text
# @arg episode: text
room-chat-started = { $name } started the room · Episode { $episode }

# Chat line: you started the room.
# @arg episode: text
room-chat-you-started = You started the room · Episode { $episode }

# Chat line: someone paused, at a place in the episode.
# @arg name: text
# @arg at: text
room-chat-paused = { $name } paused at { $at }

# Chat line: you paused.
# @arg at: text
room-chat-you-paused = You paused at { $at }

# Chat line: someone played after a pause.
# @arg name: text
room-chat-resumed = { $name } resumed

# Chat line: the room waits for people whose player is opening the episode.
# @arg names: text
room-chat-paused-for-loading = Paused for { $names } · loading

# Chat line: the room waits for people whose player stalled.
# @arg names: text
room-chat-paused-for-buffering = Paused for { $names } · buffering

# Chat line on your own device: the room waits for you.
room-chat-paused-for-you = Paused for you · loading

# Chat line: someone pressed Carry on.
# @arg carrier: text
# @arg names: text
room-chat-carried-on = { $carrier } carried on without { $names }

# Chat line: someone's first frame landed and they joined at the room's place.
# @arg name: text
# @arg at: text
room-chat-joined-at = { $name } joined at { $at }

# Chat line: someone left the room.
# @arg name: text
room-chat-left = { $name } left

# Chat line: someone moved to a new room.
# @arg name: text
room-chat-moved = { $name } moved to a new room

# Chat line in the old room: someone went along to the new room.
# @arg name: text
room-chat-went = { $name } went to the new room

# Chat line: up next moved the room on.
# @arg episode: text
room-chat-up-next = { -up-next-cap }: episode { $episode } for everyone

# Chat line: someone skipped the opening for the room.
# @arg name: text
room-chat-skipped-opening = { $name } skipped the opening

# Label of the laugh reaction button.
# @max 16
room-react-laugh = Laugh

# Label of the gasp reaction button.
# @max 16
room-react-gasp = Gasp

# Label of the clap reaction button.
# @max 16
room-react-clap = Clap

# Label of the cry reaction button.
# @max 16
room-react-cry = Cry

# Chat line: someone laughed, at a place in the episode.
# @arg name: text
# @arg at: text
room-reacted-laugh = { $name } laughed at { $at }

# Chat line: someone gasped, at a place in the episode.
# @arg name: text
# @arg at: text
room-reacted-gasp = { $name } gasped at { $at }

# Chat line: someone clapped, at a place in the episode.
# @arg name: text
# @arg at: text
room-reacted-clap = { $name } clapped at { $at }

# Chat line: someone cried, at a place in the episode.
# @arg name: text
# @arg at: text
room-reacted-cry = { $name } cried at { $at }

## Full screen strip: the faces in a capsule at the top of the full screen picture. It shows the
## room's line (room-line-with-seconds) and Carry on; its faces use room-face-label.

## TV: the full picture with the strip, and the rail beside a smaller video.

# Line under the video on the TV rail: what the remote's buttons do.
room-tv-keys = OK for the controls · Back closes the room

# Line under the chat on the TV rail: chat is read-only there.
room-tv-write-from-phone = Write from your phone

## The Watch together place: the room you are in, joining with a link, and recent rooms.

# Line under the place's heading.
room-place-line = Each of you plays the episode from your own { -source-pl }; the room keeps you in step. Start a room from any episode with { -watch-together-cap }, in the player or on its page.

# Small heading of the card for the room you are in.
room-place-yours = Your room

# Button on the card that opens the room's player.
# @max 24
room-open-player = Open the player

# Heading of the field that joins a room.
room-place-join = Join a room

# Placeholder of the field that takes an invite link.
# @max 24
room-place-link = Invite link

# Button that pastes the clipboard into the field.
# @max 16
room-place-paste = Paste

# Heading of the list of recent rooms.
room-recent = Recent rooms

# Line beside the heading.
room-recent-note = The last ten, kept on this { -device }

# Line of a recent room: the episode, who was there, and when, already formatted, such as Tuesday or 19:25.
# @arg episode: text
# @arg names: text
# @arg when: text
room-recent-row = Episode { $episode } · with { $names } · { $when }

# Line of a recent room where you were alone.
# @arg episode: text
# @arg when: text
room-recent-row-alone = Episode { $episode } · { $when }

# Button on a recent room that joins it again.
# @max 16
room-rejoin = Rejoin

## Errors: joining that fails, the cap and old invites.

# Title of the alert when none of an invite's relays answers, and the bold start of the line in the place.
room-error-unreachable-title = Couldn't reach this room's relays

# Line under it, and the next sentence in the place.
room-error-ask-new-invite = Ask for a new invite.

# The same error in one line, where there is no room for a title.
room-error-unreachable = Couldn't reach this room's relays. Ask for a new invite.

# Line after it in the place: what was tried.
# @arg seconds: int
room-error-tried = { -app-name } tried the link's relays and your spares, { $seconds } s each.

# Title of the alert when a recent room has nobody answering, such as after a new invite.
room-error-nobody-answered = Nobody answered in this room

# Line under it.
room-error-nobody-answered-line = If someone made a new invite, the old one stopped working. Ask them for the new link.

# Notice when the room already has 10 people and you can't join.
room-error-full = This room is full

# Notice when an invite link or code is from an older version.
room-error-old-invite = This invite is from an older version of { -app-name }. Ask for a new one.

# Chat line: someone came into the room.
# @arg name: text
room-chat-joined = { $name } joined

# Chat line: someone moved the room to another place in the episode.
# @arg name: text
# @arg from: text
# @arg to: text
room-chat-jumped = { $name } jumped from { $from } to { $to }

# Chat line: the room waits for people catching up after a jump.
# @arg names: text
room-chat-paused-for-seeking = Paused for { $names } · catching up

# Chat line: this device's player was moved to stay with the room.
room-chat-corrected = Moved to stay with the room

# Chat line: someone put on another episode for the room.
# @arg name: text
# @arg episode: text
room-chat-episode-changed = { $name } put on episode { $episode }

# Chat line: someone who was behind caught up with the room.
# @arg name: text
room-chat-caught-up = { $name } caught up with the room

# Chat line on a device whose Play lost to someone's Pause pressed at the same moment.
# @arg name: text
room-chat-already-paused = { $name } already paused

# Chat line: someone made a new invite; the old link stopped working.
# @arg name: text
room-chat-new-invite = { $name } made a new invite

# Chat line: someone cancelled Up next for everyone.
# @arg name: text
room-chat-up-next-cancelled = { $name } cancelled { -up-next }

# Chat line on your own device: your name was taken, so the room knows you by this one.
# @arg name: text
room-chat-renamed = You're { $name } in this room

## Added for the apps' room UI.

# Button that shows the room's column again after Hide room.
# @max 24
room-show = Show room

# The room's Skip opening button on the player: skips for everyone.
# @max 24
room-skip-opening = Skip opening

# Confirmation after Copy invite link.
room-link-copied = Invite link copied

# The starting screen's line on the device opening the room's episode.
# @arg episode: text
room-starting-episode = Starting episode { $episode }

# A short length in minutes, on the banner before play.
# @arg minutes: int
room-minutes = { $minutes } min

# Joins the last name to the ones before it: "Mika, Ren and Sol". `names` is the ones before,
# already joined with room-names-comma.
# @arg names: text
# @arg last: text
room-names-and = { $names } and { $last }

# Joins two names in a longer list: "Mika, Ren".
# @arg first: text
# @arg rest: text
room-names-comma = { $first }, { $rest }

## Asked for by the Windows room (26 Nov)

# Person menu item and accessibility action that hides someone's lines and reactions in this room.
# @arg name: text
# @max 32
room-mute = Mute { $name }

# Person menu item and accessibility action that shows a muted person's lines again.
# @arg name: text
# @max 32
room-unmute = Unmute { $name }

# Line in the conversation where a muted person's lines were.
# @arg name: text
room-chat-muted = { $name } is muted

# Line after the newest chat lines over the video, which opens the conversation.
# @arg count: int
# @max 24
room-chat-more = { $count } more in chat

# Spoken by a screen reader when a chat line arrives.
# @arg name: text
# @arg message: text
room-chat-announce = { $name } says { $message }

# End of the chat field while sending waits, after three messages in five seconds.
# @arg seconds: int
# @max 16
room-chat-wait = Wait { $seconds } s

# Spoken once when sending waits, for screen readers.
# @arg seconds: int
room-chat-wait-spoken = { $seconds ->
    [one] Wait one second
   *[other] Wait { $seconds } seconds
}

# Spoken by a screen reader once, when 20 characters are left in the chat field.
# @arg count: int
room-chat-characters-left = { $count ->
    [one] { $count } character left
   *[other] { $count } characters left
}

# A reaction line with the moment in the episode it was made at, such as "Ren and Mika laughed at 12:04". The time is minutes and seconds.
# @arg line: text
# @arg time: text
room-reacted-at = { $line } at { $time }

# Label on the seek bar when pointing near a reaction tick: the moment and who reacted.
# @arg time: text
# @arg names: text
room-tick-label = { $time } · { $names }

# Two people named together, for a reaction made by both: "Ren and Mika".
# @arg first: text
# @arg second: text
room-names-two = { $first } and { $second }

# Three people named together.
# @arg first: text
# @arg second: text
# @arg third: text
room-names-three = { $first }, { $second } and { $third }

# More than three people named together: two names and how many others.
# @arg first: text
# @arg second: text
# @arg count: int
room-names-others = { $first }, { $second } and { $count } others

# Player panel, under the subtitled or dubbed switch in a room.
room-version-own = Only your player switches. The room keeps playing.

# The banner over the room when the episode has no name of its own: the episode and its length.
# @arg episode: text
# @arg length: text
room-banner-episode-plain = Episode { $episode } · { $length }

# Menu item dimmed in a room, with this reason under it.
room-mark-unavailable = Mark skip times after you leave the room.

## Asked for by the apps (26 Nov)

# Heading of the people list in the room, and its accessibility label.
# @max 32
room-people = People in the room

# State word: the player is moving to the room's new place after someone sought, with the seconds so far. Also the room's second line while it waits for one seeking person.
# @arg seconds: int
# @max 24
room-state-seeking = Seeking · { $seconds } s

# Bold start of the failure line in the room's place, followed by the hint (room-error-ask-new-invite). With its full stop: room-error-unreachable-title is the same words as an alert's title.
room-error-unreachable-lead = Couldn't reach this room's relays.

# Bold start of the failure line in the room's place, followed by the hint (room-error-nobody-answered-line). With its full stop: room-error-nobody-answered is the same words as an alert's title.
room-error-nobody-answered-lead = Nobody answered in this room.


# Line on a phone when its paired TV is in the room. The device is its name, such as Living room TV.
# @arg device: text
room-device-in-room = { $device } is in this room

# Button on the phone's remote screen that leaves the room on the paired TV.
# @arg device: text
# @max 40
room-leave-on-device = Leave the room on { $device }

# Line on the TV: joining someone's room goes through a paired phone, as the TV can't open links.
room-tv-join-from-phone = To join someone's room, open their invite on a paired phone and choose this TV.

# Line above the list of sources that have the room's episode, in the Choose a source dialog in a room.
# @arg count: int
# @max 90
room-sources-found-line = { $count ->
    [one] One { -source } has it. It plays for everyone in the room.
   *[other] { $count } { -source-pl } have it. Your pick plays for everyone in the room.
}

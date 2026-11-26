# What the terminal says beside a notice that has something to do about it. The key is the one
# bound to stopping playback, as the key map draws it.
# @arg key: text
# @only tui
hint-stop-playback = Press { $key } to stop playback.

# What the terminal says beside a notice about a { -plugin } update: where to go.
# @only tui
hint-open-stores = Go to { -source-cap-pl } and press Enter to open the { -plugin-store-pl }.

# What the terminal says beside the notice that Kitsu signs in from the Accounts screen.
# @only tui
hint-kitsu-credentials = Press Enter on its row to enter your credentials.

# Status line while the { -source-pl } are rebuilt after a change.
# @only tui
status-reloading-sources = Reloading { -source-pl }…

# Toast when the { -source } pinned for a title can't be used and the others are tried instead.
# @arg name: text
# @only tui
pin-unavailable = { $name } is pinned for this title but isn't available. Trying the other { -source-pl }.

# Name of the Together section in the terminal's rail, on the key 0.
# @max 16
# @only tui
tui-together-section = Together

# Prompt of the Join line in the Together section: what to paste there.
# @max 24
# @only tui
tui-together-join-prompt = invite link

# Line under "Chat is hidden" in the Together section's conversation, while Hide chat is on.
# @only tui
tui-together-chat-hidden-line = Lines and reactions stay off the player in every room.

# Second line of a recent room nobody else was in. The time is already formatted, such as Tuesday.
# @arg episode: text
# @arg when: text
# @only tui
tui-together-recent-alone = Episode { $episode } · { $when }

# Last line of reading's Home: the key that opens the choice of what { -app-name } is for. The key is the one bound to it, as the key map draws it.
# @arg key: text
# @only tui
tui-medium-read-change = Press { $key } to change what { -app-name } is for.

# Hint beside the Kaeri is for line on the Settings screen. The key is the one bound to it, as the key map draws it.
# @arg key: text
# @max 24
# @only tui
tui-medium-settings-hint = { $key } to change

# Row in the terminal's Report a bug band, under Open on GitHub: Enter writes the player log file
# beside the terminal's own logs. Lower-cased, the same words are the key hint for it.
# @only tui
report-player-log-save-here = Save player log

# That row once the file is written, and the notice in the status line. Path is where the file is,
# such as ~/.cache/kaeri/logs/kaeri-player-log-2026-11-15-1432.txt.
# @arg path: text
# @only tui
report-player-log-saved-to = Player log saved to { $path }

# That row when the file could not be written. Folder is where it was to go, such as
# ~/.cache/kaeri/logs.
# @arg folder: text
# @only tui
report-player-log-failed-here = Couldn't save the log to { $folder }.

## The room in the terminal's Together section: the room this terminal is in, a line to join
## another by its invite link, and the last rooms.

# Toast when this terminal is in a room, said away from the Together section. The key is the one that goes to Together, as the key map draws it.
# @arg key: text
# @only tui
tui-room-joined = You're in the room. Press { $key } to see it.

# Toast when what was pasted in the Together section's Join line isn't a room's invite link.
# @only tui
tui-room-not-a-link = That isn't a room's invite link.

# Toast when the room behind an invite already has 10 people or more. The key is Enter's, as the key map draws it; the Join line then reads Join anyway.
# @arg key: text
# @only tui
tui-room-full = This room is full: rooms work best with 10 or fewer. Press { $key } on the Join line to join anyway.

# Line in the field that takes an invite link.
# @only tui
tui-room-join-line = Paste the invite link someone sent you.

# Line under the Together section's conversation and in the field for saying something. States who can read the chat.
# @only tui
tui-room-chat-privacy = Messages are sealed end to end. Relays pass them along and can't read them.

# The conversation's only line while Hide chat is on, and a toast when the field is asked for then.
# @only tui
tui-room-chat-hidden = Chat is hidden

# Toast when a message was sent too soon after the last ones.
# @arg seconds: int
# @only tui
tui-room-chat-wait = Wait { $seconds } s to send again.

# A muted person's line in the conversation, in place of what they said.
# @arg name: text
# @only tui
tui-room-muted-line = { $name } is muted

# Line in the Together section when this terminal isn't in a room. The key is the one that starts a room, as the key map draws it.
# @arg key: text
# @only tui
tui-room-start-line = Each of you plays the episode from your own { -source-pl }; the room keeps you in step. Press { $key } on an episode to start a room.

# Line in place of the recent rooms when none are kept yet.
# @only tui
tui-room-recent-empty = The last ten rooms you're in show here.

# The room's connection, under its title: the room's messages are sealed end to end, and how many of its relays answer.
# @arg count: int
# @arg total: int
# @only tui
tui-room-relays-line = End to end · { $count } of { $total } relays answering

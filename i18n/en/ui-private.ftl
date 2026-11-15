## Private watching and reading

# The private mode switch: under the account card on phones, first in Settings › Accounts on the
# desktops and the TV, and a check item in the Linux main menu.
private-mode = Private mode

# The switch's description: the footer on Apple, supporting text on Android and the TV, the
# SettingsCard description on Windows, the AdwSwitchRow subtitle on Linux.
private-mode-detail = Nothing you watch or read is saved or synced until you turn it off or quit { -app-name }.

# Second line under the switch. Private mode only stops what the app keeps; it hides nothing from
# anyone else.
private-mode-limits = Your { -source-pl } and { -watch-party-pl } still see what you open.

# Notice after the menu item or the shortcut turns private mode on, and the words of the badge's
# popover, flyout and TV dialog. Also the badge's screen reader hint.
private-mode-on-notice = Private mode is on. Nothing is saved or synced until { -app-name } quits.

# Button in the notice, the badge's popover and flyout, and the TV dialog: turns private mode off.
action-turn-off = Turn off

# Under the switch on the desktops. The keys are the platform's own: ⇧⌘N on the Mac, Ctrl+Shift+N
# on Windows and Linux.
# @arg keys: text
private-mode-shortcut-hint = Shortcut: { $keys }

# The badge while private mode is on, the player label, and the head of the title page line.
# @max 12
private-badge = Private

# Screen reader label of the badge.
private-badge-label = Private mode is on

# Line under a title or series page's main action, after "Private" or "Always private".
private-line-not-saved = Not saved or synced

# Line under a title or series page's main action when a place is held in memory, after "Private".
private-line-kept = Your place is kept until { -app-name } quits

# A title kept private on this device: the More menu item with a check, the switch in the show's
# settings, and the head of the title page line.
title-always-private = Always private

# Under the Always private switch in a show's settings.
show-always-private-detail = Nothing from this show is saved or synced on this { -device }. What is already saved stays.

# Under the Always private switch in a series page's settings.
series-always-private-detail = Nothing from this series is saved or synced on this { -device }. What is already saved stays.

# What the system's media controls, the lock screen, the media card and the player window show
# while private: no show, episode, artist or artwork.
now-playing-private = Playing in { -app-name }

# Android's required download notification for a download started while private. No title or art.
# @only android
private-download-line = Downloading in { -app-name }

# Announced to screen readers once when private mode turns on.
private-mode-on-announcement = Private mode on

# Announced to screen readers once when private mode turns off.
private-mode-off-announcement = Private mode off

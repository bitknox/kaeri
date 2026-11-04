## Sync words the computer apps share, Windows and Linux. The rest of sync is in ui-sync.ftl.

# Windows and Linux dialog line while the browser is open for a cloud sign-in.
# @arg service: text
cloud-browser-line = Your browser opened { $service }’s page. Allow access there, and { -app-name } carries on by itself.

# Windows and Linux dialog title while the browser is open for a cloud sign-in.
# @arg service: text
cloud-browser-title = Sign in to { $service } in your browser

# Word for this device in sentences such as “Turn off sync on this computer”.
device-kind-computer = computer

# Button on the Devices page that opens the box for a code another device shows.
devices-enter-code = Enter a code

# Button on the Devices page that shows the recovery key behind the computer’s own lock.
devices-show-key = Show

# Button in the alert that turns sync off on this computer.
devices-turn-off-action = Turn off

# Label of the field for the other device’s address in the Enter a code dialog.
pair-address-label = Address

# Placeholder of the pairing code field.
pair-code-placeholder = 000 000

# Label of the field for a recovery key in the restore dialog.
rkey-field-label = Recovery key

# Label of the user name field for WebDAV and a Nextcloud app password.
cloud-user-label = User name

# Label of the password field for WebDAV and a Nextcloud app password.
cloud-password-label = Password

# Label of the WebDAV server address field.
cloud-webdav-address-label = Server address

# Line under the pairing code while a device is being paired.
pair-working-line = Pairing

# Name for the device whose pairing code did not match, when no name is known yet.
pair-other-device = your other { -device }

# Note under the Enter a code dialog's fields: the address can be left empty.
pair-address-optional = Leave the address empty and { -app-name } looks for the { -device } on this network.

# Device page footer when the device has no Remote switch.
devices-sync-foot = Sync shares where you stopped, your history and your settings.

# A device's line on the Devices page with Remote after it, when this device drives it as a remote. The word is the Remote switch's.
# @arg line: text
devices-line-remote = { $line } · Remote

# Menu item on a profile's row on the Profiles pane, and the edit dialog's title on Linux.
profiles-edit-action = Edit

# Label of the name field in the add and edit profile dialogs.
profiles-name-field = Name

# Accessible name and tooltip of the menu button on a profile's row.
# @arg name: text
profiles-more-for = More for { $name }

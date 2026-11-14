## The terminal's { -device-cap-pl } screen, reached from Settings: sync with the person's other
## { -device-pl }, and their profiles when there are two or more. The shared words are in ui-sync.

# The word for this kind of { -device } inside sentences such as "Turn off sync on this computer". Lower case.
# @only tui
sync-tui-kind = computer

# A row under Your { -device-pl }: pair by typing the code another { -device } shows.
# @only tui
sync-tui-enter-code = Enter a code

# The Cloud folder row while sync uses no cloud folder.
# @only tui
sync-tui-no-cloud = No cloud folder

# Beside a { -device } in Your { -device-pl } that has not synced with this one yet.
# @only tui
sync-tui-device-never = Not synced yet

# Beside a profile: the one in use now. Column value.
# @only tui
sync-tui-profile-in-use = In use

# The wait while the browser shows a cloud service's sign-in page. The service is a name such as Dropbox.
# @arg service: text
# @only tui
sync-tui-browser-line = Your browser opened { $service }'s page. Allow access there, and { -app-name } carries on by itself.

# Under Open the page again: the page's address follows on the next line, for a browser that did not open.
# @only tui
sync-tui-browser-address = Or go to this address yourself:

# Title of the waiting dialog while a cloud sign-in runs in the browser.
# @only tui
sync-tui-signing-in = Signing in

# Title of the field for a WebDAV server's address.
# @only tui
sync-tui-webdav-address = WebDAV address

# Under the WebDAV address field.
# @only tui
sync-tui-webdav-address-note = The address of the folder on your server, starting with https://.

# Title of the field for a folder's path on this computer.
# @only tui
sync-tui-folder-path = Folder

# Under the folder path field.
# @only tui
sync-tui-folder-path-note = The full path of a folder another app keeps in step, such as a synced drive.

# Title of the field for the user name a WebDAV or Nextcloud server signs in with.
# @only tui
sync-tui-user = User name

# Under the user name field, for Nextcloud's app password.
# @only tui
sync-tui-nextcloud-user-note = The name you sign in to Nextcloud with.

# Title of the field for a WebDAV password. It is drawn as dots.
# @only tui
sync-tui-password = Password

# Title of the field for a Nextcloud app password. It is drawn as dots.
# @only tui
sync-tui-app-password = App password

# Label before the six-digit pairing code this { -device } shows.
# @only tui
sync-tui-code = Code

# Label before this { -device }'s network address and port, which the other { -device } enters with the code.
# @only tui
sync-tui-address = Address

# While this { -device } is getting a pairing code ready to show.
# @only tui
sync-tui-pair-making = Making a code

# Title of the field for the other { -device }'s address, the second step of entering a code.
# @only tui
sync-tui-pair-address = Address of the other { -device }

# Under the address field when entering a pairing code.
# @only tui
sync-tui-pair-address-note = The address the other { -device } shows under its code, such as 192.168.1.20:47811.

# While this { -device } pairs with the one whose code was entered.
# @only tui
sync-tui-pairing = Pairing

# After a recovery key is shown: where to see it again in the terminal.
# @only tui
sync-tui-rkey-again = You can see it again in Settings, { -device-cap-pl }.

# A choice when adding a profile: add it as a kids profile.
# @only tui
sync-tui-add-kids = Add as a kids profile

# Said when the terminal closes to start again on another profile, and printed when it cannot start again by itself.
# @arg name: text
# @only tui
sync-tui-profile-restart = Start { -app-name } again to use { $name }'s profile.

# A sync problem: the folder holds nothing to join.
# @only tui
sync-tui-err-no-group = This folder holds no history to join. Choose another folder.

# A sync problem: a cloud service that this build can't sign in to. The service is a name such as Dropbox.
# @arg service: text
# @only tui
sync-tui-err-unavailable = { $service } isn't available on this { -device }.

# Status line hint: Enter removes the highlighted { -device } from sync, after a question.
# @max 12
# @only tui
sync-tui-hint-remove = Remove

# Status line hint: Enter shows the recovery key.
# @max 12
# @only tui
sync-tui-hint-show = Show

# Status line hint: Enter turns sync off on this { -device }, after a question.
# @max 12
# @only tui
sync-tui-hint-turn-off = Turn off

# Status line hint: a key that copies the recovery key.
# @max 12
# @only tui
sync-tui-hint-copy = Copy

# In place of a QR code when the window is too short to draw it whole.
# @only tui
sync-tui-qr-no-room = Make the window taller to see the QR code.

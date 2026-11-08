## Sync, devices and profiles: the cloud folder, pairing, the recovery key and profiles, on every platform.

# Settings row and page title, every platform.
devices-title = { -device-cap-pl }

# Devices row value while sync is off.
devices-row-off = Sync off

# Devices row value; “Synced 2 min ago”, “Synced just now”.
# @arg ago: text
devices-row-synced = Synced { $ago }

# Devices row value with the error glyph.
devices-row-error = Couldn’t sync

# Group heading over the name.
devices-this-device = This { -device }

# Row for this device’s name.
devices-name = Name

# Group heading while off.
sync-group = Sync

# Off state.
sync-off-title = Sync is off

# Off state.
sync-off-line = Turn it on to keep where you stopped, your history and your settings the same on your { -device-pl }.

# Button; the Mac adds an ellipsis.
sync-turn-on = Turn on sync

# Road sheet title.
sync-road-title = How should your { -device-pl } share?

# Road sheet row.
sync-road-nearby = With a { -device } nearby

# Road sheet row line.
sync-road-nearby-line = Pair with another { -device } on the same Wi-Fi. Nothing is stored anywhere else.

# Road sheet row.
sync-road-cloud = Through a cloud folder

# Road sheet row line. Names no service, since a build may hide some (decision 14).
sync-road-cloud-line = A cloud drive you already use, or your own server. It works while your { -device-pl } are apart and keeps a backup.

# Road sheet footer.
sync-road-later = You can add the other way later. Most people use both.

# Group heading.
devices-yours = Your { -device-pl }

# Empty Your devices.
devices-only-this = Only this { -device }

# Empty Your devices.
devices-only-this-line = Pair another { -device } to share where you stopped, your history and your settings.

# Row and sheet title.
devices-add = Add a { -device }

# Device row line past 30 days.
# @arg days: int
devices-not-seen = Not seen for { $days } days

# A device paired only for Play on TV.
devices-remote-only = Remote only

# Button on a remote-only TV row.
# @arg device: text
devices-also-sync = Also sync with { $device }

# Footer while sync is off.
devices-paired-tvs-foot = Paired for Play on TV. A TV paired as a remote never gets your history.

# Switch on a device.
devices-sync-switch = Sync

# Switch on a TV.
devices-remote-switch = Remote

# Device page footer when the device has a Remote switch. The kind is this device's word, such as computer or PC.
# @arg kind: text
# @arg device: text
devices-switches-foot = Sync shares where you stopped, your history and your settings. Remote lets this { $kind } drive { $device }.

# Device page, destructive.
devices-remove = Remove this { -device }

# Alert title.
# @arg device: text
devices-remove-alert = Remove { $device }?

# Alert message.
# @arg device: text
devices-remove-alert-line = { $device } keeps what it has, but gets nothing new from your other { -device-pl }.

# Last row; “this phone”, “this Mac”.
# @arg kind: text
sync-turn-off = Turn off sync on this { $kind }

# Alert title.
# @arg device: text
sync-turn-off-alert = Turn off sync on { $device }?

# Alert message.
# @arg kind: text
sync-turn-off-alert-line = This { $kind } stops sharing with your other { -device-pl }. Everything it has stays here.

# Group heading.
cloud-group = Cloud folder

# Group footer.
cloud-foot = The folder keeps your { -device-pl } in step while they are apart, and holds a sealed backup. Only your { -device-pl } can read it.

# Row when only the nearby road is set.
cloud-add = Add a cloud folder

# Picker title.
cloud-pick-title = Choose a cloud folder

# Picker row line.
cloud-google-line = A hidden { -app-name } folder in your Drive

# Picker row line.
cloud-nextcloud-line = Your own or a hosted Nextcloud server

# Picker row line, Apple and Windows.
cloud-icloud-line = A folder you choose in iCloud Drive

# Picker group heading.
cloud-other = Other

# Picker row.
cloud-webdav = WebDAV server

# Picker row line.
cloud-webdav-line = Any server that speaks WebDAV

# Picker row line.
cloud-folder-line = For a folder another app keeps in step

# Picker footer.
cloud-pick-foot = Your { -device-pl } keep a sealed copy of their changes in this folder. Only your { -device-pl } can read it, and { -app-name } runs no server of its own.

# Row line while the first write runs.
cloud-setting-up = Setting up the folder

# Field label.
cloud-nextcloud-address = Server address

# Field note.
cloud-nextcloud-address-note = The address you open Nextcloud at in a browser.

# Login flow wait.
# @arg host: text
cloud-nextcloud-wait-title = Finish signing in on { $host }

# Login flow wait.
cloud-nextcloud-wait-line = Your Nextcloud page is open in the browser. Allow access there, and this sheet carries on by itself.

# Wait row.
# @arg service: text
cloud-waiting = Waiting for { $service }

# Button.
cloud-open-again = Open the page again

# Text button.
cloud-app-password = Use an app password instead

# Button.
cloud-connect = Connect

# Error.
# @arg service: text
cloud-err-offline = Couldn’t reach { $service }. Check your connection.

# Inline error.
# @arg host: text
cloud-err-no-server = Couldn’t find a Nextcloud server at { $host }. Check the address.

# Inline error.
cloud-err-app-password = Nextcloud didn’t accept that app password. Make a new one in Nextcloud under Settings, Security.

# Inline error.
cloud-err-webdav = The server refused that user name or password.

# Error, with Open the page again.
cloud-err-timeout = The sign-in page closed before access was allowed.

# Error.
cloud-err-other-group = This folder belongs to another set of { -device-pl }. Enter its recovery key to join them, or choose another folder.

# Status error, with Sign in again.
# @arg service: text
# @arg kind: text
cloud-err-signed-out = { $service } signed { -app-name } out. Your changes wait on this { $kind } until you sign in again.

# Status error.
# @arg service: text
cloud-err-full = { $service } is full. Free some space there, then try again.

# Status error, with Choose folder.
cloud-err-folder-gone = { -app-name } can’t open the folder any more. Choose it again.

# Status error title.
# @arg service: text
cloud-err-title = Couldn’t sync with { $service }

# Picker row line.
cloud-dropbox-line = { -app-name }’s own folder in your Dropbox, Apps/{ -app-name }

# Picker row line.
cloud-onedrive-line = { -app-name }’s own folder in your OneDrive, Apps/{ -app-name }

# Cloud group button.
cloud-sync-now = Sync now

# Cloud group button; at once, no question, except on the TV.
cloud-sign-out = Sign out

# Cloud group button after signing out.
cloud-sign-in = Sign in

# Cloud row after Sign out; hollow haze dot.
# @arg kind: text
cloud-signed-out-line = Signed out. Changes wait on this { $kind } until you sign in again.

# Picker row line after `SignInDenied`.
# @arg service: text
cloud-err-denied = { $service } didn’t give { -app-name } access. Choose it again to retry.

# TV picker footer (decision 17).
tv-cloud-pick-foot = Choose the one your other { -device-pl } use. A new cloud folder is added on a phone or computer, where its recovery key can be saved.

# TV picker row line for Dropbox.
tv-cloud-dropbox-line = Signs in on your phone

# TV, Dropbox.
tv-cloud-needs-phone-title = Dropbox signs in on your phone

# TV, Dropbox, a paired phone has it.
# @arg device: text
tv-cloud-needs-phone-has = { $device } is signed in to Dropbox. This TV can use that sign-in over your paired link.

# TV amber button; also for Nextcloud and WebDAV.
# @arg device: text
tv-use-phone-signin = Use { $device }’s sign-in

# TV, Dropbox, no phone has it.
tv-cloud-needs-phone-none = Dropbox has no sign-in a TV can do. On your phone, open { -app-name }, then Settings, { -device-cap-pl }, Cloud folder, and sign in to Dropbox there. Then choose Dropbox here again.

# TV code screen after the poll succeeds.
tv-cloud-finishing = Signed in. Setting up the folder

# TV cloud row.
# @arg service: text
tv-cloud-revoked-line = { $service } signed { -app-name } out. Select to sign in again.

# TV cloud row.
tv-cloud-signed-out-line = Signed out. Select to sign in.

# TV ConfirmDialog.
# @arg service: text
tv-cloud-sign-out-title = Sign out of { $service }?

# TV ConfirmDialog, with Cancel focused and Sign out.
# @arg service: text
tv-cloud-sign-out-line = This TV stops syncing through { $service }. Changes wait here until you sign in again.

# Status line while offline.
# @arg kind: text
cloud-offline = Waiting for a connection. Changes are kept on this { $kind }.

# TV device flow.
tv-google-title = Sign in to Google Drive

# TV device flow.
tv-qr-open-page = Or scan to open the page

# TV Nextcloud.
tv-nextcloud-line = Scan this with your phone and allow access on your Nextcloud page. The TV carries on by itself.

# Sheet title.
rkey-title = Your recovery key

# Sheet line.
rkey-line = Keep this somewhere safe. It restores your history if you lose every { -device }, and lets a new { -device } join from your cloud folder.

# Button.
rkey-copy = Copy

# Text button.
rkey-qr = Show as QR code

# Sheet footnote.
# @arg kind: text
rkey-again = You can see it again in Settings, { -device-cap-pl }, after unlocking this { $kind }.

# Devices row.
rkey-row = Recovery key

# Devices row value.
# @arg date: text
rkey-made = Made { $date }

# Row.
rkey-new = Make a new key

# Footer.
rkey-new-foot = The old key stops working, and your { -device-pl } switch to the new one the next time they sync.

# Restore.
rkey-enter-title = Enter your recovery key

# Restore.
# @arg service: text
# @arg count: int
rkey-enter-line =
    { $service } holds history from { $count ->
        [one] { $count } { -device }
       *[other] { $count } { -device-pl }
    }. The key you saved when sync was turned on opens it.

# Field note.
rkey-enter-note = 24 letters and digits, in groups of four. Spaces and case don’t matter.

# Inline error.
rkey-wrong = That key doesn’t open this folder. Check each group of four and try again.

# Button.
rkey-restore = Restore

# Add a device.
pair-enter-line = On the other { -device }, open { -app-name } and choose Set up from another { -device }, or Settings, { -device-cap-pl }, Show a code. Then enter the code here.

# Button, phones.
pair-scan = Scan the code

# Note.
pair-same-wifi = Both { -device-pl } need to be on the same Wi-Fi.

# Button.
pair-button = Pair

# Text button.
pair-show-instead = Show a code here instead

# Code screen.
pair-show-title = Enter this code on your other { -device }

# Code screen.
pair-show-scan = Or scan it from { -app-name } on your phone.

# Code screen.
# @arg left: text
pair-good-for = Good for 2 minutes · { $left }

# TV code screen.
pair-tv-line = On your phone, open { -app-name }, then Settings, { -device-cap-pl }, Add a { -device }, and scan this code or enter it.

# Success.
# @arg device: text
pair-ok = Paired with { $device }

# Success.
pair-ok-line = Bringing over your history. You can keep using { -app-name } meanwhile.

# Success switch.
# @arg device: text
pair-copy-signins = Copy sign-ins from { $device }

# Success switch line.
# @arg trackers: text
pair-copy-signins-line = { $trackers }. Each { -device } gets its own copy over this link.

# Error.
# @arg device: text
pair-err-code = That code didn’t match. Check the code on { $device } and try again.

# Error.
pair-err-not-found = Couldn’t find the { -device } showing this code. Both { -device-pl } need to be on the same Wi-Fi.

# Error on the showing side.
pair-err-expired = This code ran out. Show a new one.

# Profile question.
# @arg device: text
pair-which-profile = Which profile on { $device } is yours?

# Profile question.
# @arg kind: text
# @arg device: text
pair-which-profile-line = This { $kind } syncs with that profile only. The others stay on { $device }.

# Footer.
# @arg device: text
pair-also-sync-foot = { $device } already knows this phone from Play on TV, so syncing with it needs no new code.

# Continue, for a day.
# @arg n: int
# @arg time: text
# @arg device: text
continue-from-device = Episode { $n } · stopped at { $time } on { $device }

# Player, 6 s, with Start over.
# @arg time: text
# @arg device: text
player-resumed-from = Resumed at { $time } from { $device }

# Row line, Mac toolbar status.
sync-syncing = Syncing

# Mac jobs popover.
# @arg device: text
sync-syncing-with = Syncing with { $device }

# Device page group.
sync-notes = Sync notes

# Sync note.
# @arg n: int
# @arg title: text
# @arg other: text
# @arg device: text
sync-note-kept-watched = Episode { $n } of { $title } stays watched. { $other } marked it unwatched without knowing { $device } had finished it.

# Sync note action.
sync-note-undo = Mark unwatched

# Footer.
sync-notes-foot = Notes stay for 7 days.

# Settings row and page.
profiles-title = Profiles

# Profiles row value with one profile.
profiles-only-you = Only you

# Profiles page with one.
profiles-one-foot = Profiles keep separate history, { -continue-cap } and accounts for people who share this { -device }. With one profile, { -app-name } has no picker and no names.

# Row, sheet title, picker tile.
profiles-add = Add a profile

# Sheet line.
profiles-add-line = Each profile keeps its own history, { -continue-cap } and accounts. { -source-cap-pl } and downloads stay shared on this { -device }.

# Field label.
profiles-new = New profile

# Field label.
profiles-yours = Your profile

# Field note when signed in.
profiles-yours-note = Filled in from AniList. Until now this { -device } had one profile with no name.

# Switch.
profiles-kids = Kids profile

# Switch footer.
profiles-kids-line = Adult titles stay off, and { -source-pl }, { -device-pl } and torrent settings are hidden in it.

# Settings foot in a kids profile.
profiles-kids-hidden = { -source-cap-pl } and { -device-cap-pl } are hidden in a kids profile, and adult titles stay off.

# Line under a kids profile.
profiles-kids-tag = Kids

# Button.
profiles-add-button = Add profile

# Profiles page footer.
profiles-shared-foot = { -source-cap-pl }, downloads and this { -device }’s settings are shared by every profile.

# Profile page row.
profiles-syncs-with = Syncs with

# Profile page, destructive.
profiles-delete = Delete profile

# Alert title.
# @arg name: text
profiles-delete-alert = Delete { $name }’s profile?

# Alert message.
# @arg name: text
profiles-delete-alert-line = { $name }’s history and sign-ins on this { -device } are removed. { -device-cap-pl } that sync with { $name } keep theirs.

# TV, and iPad when asked.
profiles-picker = Who’s watching?

# iPad switch.
profiles-ask-at-launch = Ask who’s watching at launch

# iPad footer.
profiles-ask-foot = Off by default. Turn it on for an iPad the whole household shares. You can switch profiles from the account row at any time.

# TV Settings.
profiles-switch = Switch profile

# Account sheet and menus.
profiles-manage = Manage profiles

# Welcome, text button; TV says “Set up from your phone”.
first-setup-other = Set up from another { -device }

# Code screen from first launch.
first-setup-line = On a { -device } that already has { -app-name }, open Settings, { -device-cap-pl }, Add a { -device }, and enter this code or scan it.

# Text button.
first-restore = Restore from a cloud folder instead

# Success from first launch.
first-ok-line = Bringing over your history, settings and { -plugin-store }s. You can start using { -app-name } now.

# Success switch.
# @arg store: text
first-join-store = Join the { -plugin-store } { $store }

# Success switch line.
# @arg device: text
# @arg kind: text
first-join-store-line = { $device } makes this { $kind } its own invite

# Picker row on a phone or computer.
# @arg kind: text
cloud-folder = A folder on this { $kind }

# Field note under the app password, on Android.
# @arg kind: text
cloud-app-password-note = Make one in Nextcloud under Settings, Security. It is kept on this { $kind }.

# Devices row value while sync is off and TVs are paired for Play on TV. The row says "Sync off", then this.
# @arg count: int
devices-tvs-for-play = { $count ->
    [one] One TV for Play on TV
   *[other] { $count } TVs for Play on TV
}

# Line under a TV paired for Play on TV today.
devices-paired-today = Paired today

# Line under Add a device on the TV's Devices page.
tv-devices-add-line = Shows a code to enter on your phone

# TV Profiles page with one profile.
tv-profiles-only-you = Only you. Add a profile to keep separate history, { -continue-cap } and accounts on this TV.

# TV Settings, Profiles: the row that opens the picker.
tv-profiles-opens-picker = Opens Who’s watching?

# TV Profiles page heading over the profiles.
tv-profiles-on-this-tv = Profiles on this TV

# Android recovery key sheet, third button.
rkey-save-file = Save as a file

# Android recovery key sheet, button that shows the key as a code.
rkey-qr-short = QR code

# Button on the Devices page and the add a device sheet that shows a pairing code instead of entering one.
pair-show-code = Show a code

# Cloud row button when the service signed Kaeri out.
cloud-sign-in-again = Sign in again

# Cloud row button when the folder can't be opened.
cloud-choose-folder = Choose folder

# Spoken label of a profile's avatar on Home.
# @arg name: text
profiles-avatar-desc = { $name }, switch profile

# Heading over the pairing code the other device shows, over the field.
pair-code-field = Pairing code

## Words the sentences above take for this device: "this phone", "this tablet", "this TV".
device-kind-phone = phone
device-kind-tablet = tablet
device-kind-tv = TV

# TV welcome, text button.
first-setup-phone = Set up from your phone

# Error naming a device that is not known by name.
devices-other-device = the other { -device }

# Wait line on iPhone and iPad, where the sign-in page is a web sheet, not the browser.
# @arg service: text
cloud-sheet-line = { $service }’s page is open. Allow access there, and { -app-name } carries on by itself.

# Sheet or dialog title after the browser came back with access allowed.
# @arg service: text
cloud-connecting = Connecting to { $service }

# Status line beside the indicator while connecting.
cloud-connecting-line = Access allowed. Finishing the sign-in.

# Status line under Setting up the folder, whose title is cloud-setting-up.
# @arg service: text
cloud-setting-up-line = Making { -app-name }’s folder in { $service } and sealing a first copy of your history.

# Error title in the sign-in sheet or dialog.
# @arg service: text
cloud-err-signin-title = Couldn’t sign in to { $service }

# Error title when the sign-in worked and the folder could not be set up.
cloud-err-setup-title = Couldn’t set up the folder

# Error line beside Try again, after access was refused on the service's page.
# @arg service: text
cloud-err-denied-short = { $service } didn’t give { -app-name } access.

# Error line for any other failure during the sign-in; the details go to Messages.
# @arg service: text
cloud-err-signin-other = { $service } didn’t finish the sign-in. Try again in a moment.

# Error line for any other failure while making the folder after the sign-in; the details go to Messages.
# @arg service: text
cloud-err-setup-other = { -app-name } couldn’t make its folder in { $service }. Try again in a moment.

# More menu item, and a row on phones; the Mac adds an ellipsis.
cloud-use-another = Use another service

# More menu item in red, and a button in the TV's dialog.
cloud-remove = Remove cloud folder

# Confirmation title before removing the cloud folder.
# @arg service: text
cloud-remove-title = Remove { $service } from sync?

# Confirmation line before removing the cloud folder.
# @arg kind: text
# @arg service: text
cloud-remove-line = Everything on this { $kind } stays here, and paired { -device-pl } nearby keep syncing. The folder stays in { $service } with its sealed copy, and your other { -device-pl } use it until you remove it on them too.

# Destructive button in the remove confirmation.
cloud-remove-action = Remove

# Accessibility label of the More button on the cloud folder row.
# @arg service: text
cloud-more = More for { $service }

# Picker row line for the service in use, under Use another service.
cloud-in-use = In use

# Picker line under Use another service.
# @arg service: text
cloud-switch-foot = { $service } stays in use until the new folder is ready.

# Footer under the two ways in while sync is off.
sync-start-foot = Either one turns sync on, and you can use both.

# Aside on the Devices page on the Mac and iPad.
# @arg kind: text
devices-aside = Sync carries where you stopped, your history and your settings between your own { -device-pl }, nearby or through a cloud folder you choose. It is sealed before it leaves this { $kind }.

# TV Devices footer while sync is off, after devices-sync-foot.
tv-sync-start-foot = Pairing with your phone turns sync on. Then this TV can join your cloud folder.

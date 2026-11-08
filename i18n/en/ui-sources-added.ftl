## Adding a source and what came in: a book server, a folder or a plugin store, and the reason
## when nothing did. The engine says each with Event::CameIn.

# Label of the address field in the Add a server sheet.
book-server-address = Address

# Note under the empty address field.
book-server-address-hint = An address, or the whole OPDS link from Kavita.

# Note under the address field once a Kavita OPDS link is pasted.
book-server-kavita-key = This is a Kavita OPDS link. Its key signs you in, so no password is needed.

# Under the key chip. The store is where the system keeps secrets: Keychain, or Android Keystore.
# @arg store: text
book-server-key-kept = { -app-name } keeps the key in { $store } and shows only its last four characters.

# Chip inside the address field, showing the key's last four characters after four dots.
# @arg last: text
# @max 16
book-server-key = Key ••••{ $last }

# Result title once the server signed in.
# @allow library
# @arg series: int
# @arg libraries: int
book-server-found-title = { $libraries ->
    [one] Found { $series } series
   *[other] Found { $series } series in { $libraries } libraries
 }

# Line under the result title. The kind is Komga, Kavita or OPDS.
# @allow library
# @arg kind: text
# @arg host: text
book-server-found-line = { $kind } at { $host }. Pick the libraries to add.

# A library row's line in the result.
# @arg series: int
book-server-library-line = { $series } series

# Switch in the result, on to start with.
book-server-sync-progress = Send progress to the server

# Line under the switch.
# @arg host: text
book-server-sync-progress-line = Pages you read here go back to { $host }, and its progress comes here.

# Field label.
book-server-username = Username

# Field label.
book-server-password = Password

# Error title when the server could not be reached.
# @arg host: text
book-server-err-unreachable = Couldn't reach { $host }

# Line under it.
book-server-err-unreachable-line = Check the address, and that the server is on and on this network.

# Error title when the server turned the password down.
# @arg host: text
book-server-err-password = { $host } didn't accept that password

# Line under it. The kind is Komga, Kavita or OPDS.
# @arg kind: text
book-server-err-password-line = Use the username and password you use for { $kind } in a browser.

# Error title when Kavita turned the key in an OPDS link down.
# @arg host: text
book-server-err-key = { $host } didn't accept that key

# Line under it.
book-server-err-key-line = The key changes when it's reset in Kavita. Copy the OPDS link again from your account settings there.

# Title when the server lets the account in and shows it no series.
# @arg host: text
book-server-empty = { $host } shows this account no series

# Line under it.
book-server-empty-line = It lets you in, but the server may still be scanning, or this account may not have access to its books.

# Note under it.
book-server-empty-recheck = You can add it now. { -app-name } looks again every 30 minutes.

# Line under an error when the address is not a book server.
book-server-not-books-line = { -app-name } reads from Komga, Kavita and OPDS catalogues.

# Folder result title.
# @arg series: int
# @arg files: int
book-folder-found = { $files ->
    [one] Found { $series } series in one file
   *[other] Found { $series } series in { $files } files
 }

# Line under it: the folder's path, then how many series were matched by their own details.
# @arg path: text
# @arg matched: int
book-folder-found-line = { $path }. { -app-name } matched { $matched } of them by their details.

# Row in the folder result: files that are books in formats Kaeri can't open.
# @arg count: int
book-folder-unsupported = { $count ->
    [one] One file { -app-name } can't open
   *[other] { $count } files { -app-name } can't open
 }

# Line under it: the formats, already counted and joined, such as "14 MOBI, 6 AZW3".
# @arg kinds: text
book-folder-unsupported-line = { $kinds }. They stay in the folder as they are.

# One item of that list: EPUB files locked with DRM or a password.
# @arg count: int
book-folder-drm = { $count } EPUB locked with DRM

# Note under the folder result.
book-folder-opens = { -app-name } opens CBZ, CBR, EPUB and PDF files, and folders of images.

# Row action that lists the files skipped.
book-folder-show-files = Show files

# Primary action in the Add a folder sheet.
book-folder-add = Add folder

# Folder row action that picks another folder.
book-folder-change = Change

# Title when a folder holds nothing Kaeri can open.
# @arg folder: text
book-folder-empty = Nothing in { $folder } that { -app-name } can open

# Line under it.
# @arg files: int
book-folder-empty-line = { $files ->
    [one] It holds one file, which isn't CBZ, CBR, EPUB, PDF or an image.
   *[other] It holds { $files } files, none of them CBZ, CBR, EPUB, PDF or images.
 }

# Primary action after an empty folder.
book-folder-choose-other = Choose another folder

# Title of the Join a plugin store sheet.
store-join-title = Join a { -plugin-store }

# Label of the invite field.
store-invite-field = Invite

# Result title once a plugin store is joined.
# @arg store: text
store-joined-title = Joined { $store }

# Line under it. The owner is who runs the store.
# @arg owner: text
# @arg count: int
store-joined-line = { $count ->
    [one] { $owner } offers one { -plugin }. Joining installs nothing; install it here or later.
   *[other] { $owner } offers { $count } { -plugin-pl }. Joining installs nothing; pick the ones you want here or later.
 }

# Group header in the joined store's list.
store-group-reading = For reading

# Group header in the joined store's list.
store-group-watching = For watching

# Row action.
store-install = Install

# Row state once installed.
store-installed-row = Installed

# Primary action when two or more plugins are for reading.
store-install-all-reading = Install all for reading

# Result after installing.
# @arg count: int
store-installed = { $count ->
    [one] One { -plugin } installed. It's in Your { -source-pl }.
   *[other] { $count } { -plugin-pl } installed. They're in Your { -source-pl }.
 }

# Info title when every plugin in a store is for watching, and watching is off here.
# @arg store: text
store-other-medium = Every { -plugin } in { $store } is for watching

# Line under it.
store-other-medium-line = Watching is off on this { -device }, so none of them show in Your { -source-pl }.

# Secondary action under it.
store-turn-on-watching = Turn on watching

# Info title when a store has no plugins.
# @arg store: text
store-empty = { $store } has no { -plugin-pl } yet

# Line under it.
# @arg owner: text
store-empty-line = You're a member, so new { -plugin-pl } show in { -plugin-store-cap-pl } when { $owner } adds them.

# Error title when the store doesn't answer.
# @arg host: text
store-err-unreachable = Couldn't reach { $host }

# Line under it.
# @arg owner: text
store-err-unreachable-line = The invite looks right, but the { -store } isn't answering. Try again later, or ask { $owner } if it's running.

# Under the reading group: plugins for watching while watching is off here.
# @arg count: int
store-other-medium-more = { $count ->
    [one] One more is for watching, which is off on this { -device }.
   *[other] { $count } more are for watching, which is off on this { -device }.
 }

# On a TV, under the list: plugins for reading, which a TV doesn't do.
# @arg count: int
store-tv-reading-only = { $count ->
    [one] One more is for reading, which this TV doesn't do.
   *[other] { $count } more are for reading, which this TV doesn't do.
 }

# On a TV, info title when every plugin in a store is for reading.
# @arg store: text
store-tv-all-reading = Every { -plugin } in { $store } is for reading, which this TV doesn't do.

# Plugin row in an invite's result.
# @allow release
# @arg version: text
# @arg hosts: text
store-plugin-row = Version { $version } · may contact { $hosts }

# Notice after a server or folder is added, for 4 seconds.
# @arg name: text
# @arg series: int
source-added = Added { $name } · { $series } series

# Notice after Install all, for 4 seconds.
# @arg count: int
# @arg store: text
source-added-plugins = { $count ->
    [one] Installed one { -plugin } from { $store }
   *[other] Installed { $count } { -plugin-pl } from { $store }
 }

# A server's row in Your sources, just added. The kind is Komga, Kavita or OPDS; an OPDS catalogue is one library.
# @allow library
# @arg kind: text
# @arg series: int
# @arg libraries: int
source-row-server = { $libraries ->
    [one] { $kind } · { $series } series in { $libraries } library · added just now
   *[other] { $kind } · { $series } series in { $libraries } libraries · added just now
 }

# A server's row in Your sources once it has settled. The kind is Komga, Kavita or OPDS; an OPDS catalogue is one library.
# @allow library
# @arg kind: text
# @arg series: int
# @arg libraries: int
source-row-server-series = { $libraries ->
    [one] { $kind } · { $series } series in { $libraries } library
   *[other] { $kind } · { $series } series in { $libraries } libraries
 }

# A server's row in Your sources while it shows no series.
# @arg kind: text
source-row-server-empty = { $kind } · no series yet · looks again in 30 minutes

# A folder's row in Your sources.
# @arg path: text
# @arg series: int
# @arg skipped: int
source-row-folder = { $skipped ->
    [one] { $path } · { $series } series · one file skipped
   *[other] { $path } · { $series } series · { $skipped } files skipped
 }

# A plugin's row in Your sources. The medium is "Reading" or "Watching".
# @allow release
# @arg store: text
# @arg medium: text
# @arg version: text
source-row-plugin = From { $store } · { $medium } · version { $version }

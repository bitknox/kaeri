## The first run's "Already use Kaeri on another device?" page, and what joining brings. The
## engine says it with Event::SetupFrom.

# First run, page 2 of 5.
first-other-title = Already use { -app-name } on another { -device }?

# Under the title.
first-other-line = Pair with it and this { -device } gets your { -source-pl }, { -tracker-pl } and history. The steps after this are then done.

# Line under the pair row.
first-other-pair-line = Shows a code to enter on that { -device }. Both need the same Wi-Fi.

# Row that restores from a sync folder.
first-other-restore = Restore from a cloud folder

# Line under it.
first-other-restore-line = Uses the recovery key you saved

# Primary action: go on with the first run on this device alone.
first-other-no = No, start here

# Beside a spinner, after 600 ms.
pair-waiting = Waiting for your other { -device }

# After 30 seconds with nothing.
pair-nothing-yet = Nothing yet? Both { -device-pl } need to be on the same Wi-Fi.

# Group header for what the other device brings.
# @arg device: text
first-join-group = From { $device }

# Row while the rest arrives.
# @arg device: text
first-join-looking = Looking at what { $device } has…

# Row with a pop-up of Watch, Read and Both.
first-join-medium = { -app-name } is for

# Line under it when the other device reads.
# @arg device: text
first-join-medium-line = Reading, as on { $device }

# Line under the sign-ins switch: the trackers the other device is signed in to, already joined.
# @arg trackers: text
first-join-signins-line = { $trackers }

# Switch, one per book server.
# @arg host: text
first-join-server = Add { $host }

# Line under a server's switch. The kind is Komga, Kavita or OPDS; the store is Keychain or
# Android Keystore.
# @arg kind: text
# @arg device: text
# @arg store: text
first-join-server-line = { $kind } · signs in with the key from { $device }, kept in { $store }

# Note under the list.
# @arg device: text
first-join-folders = Folders stay on { $device }. Add this { -device }’s own folders in Your { -source-pl }.

# Note when the setup answered the remaining pages.
first-join-steps-done = Lists and { -source-pl } are set up, so first run ends here. You can change them in Settings.

# Row error when a plugin store refused this device.
# @arg store: text
# @arg owner: text
first-join-store-refused = { $store } didn’t let this { -device } in. { $owner } can send you an invite.

# Notice on Home, for 4 seconds.
# @arg device: text
first-join-syncing = Syncing with { $device }

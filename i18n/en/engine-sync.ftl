# What the engine writes itself for cross-device sync, where no app screen shows it.

# The page the browser shows after signing in to a cloud drive on a computer (Windows, Linux or
# the terminal), once Kaeri has caught the sign-in. The person goes back to the app by themselves.
cloud-loopback-done = You can close this tab and go back to { -app-name }.

# The error line under Sync on a device that another of the person's devices removed from sync.
# Joining again, with a code or the recovery key, or setting sync up again adds it as a new device.
sync-device-removed = This { -device } was removed from sync on another { -device }. Join again to sync it.

# Alert when the person tries to delete the profile in use, which is refused. Another profile
# has to be entered first.
# @arg name: text
notice-profile-in-use = { $name }'s profile is in use. Switch to another profile to delete it.

# Alert when a profile could not be deleted. The reason comes from the system.
# @arg name: text
# @arg reason: text
notice-profile-not-deleted = Couldn't delete { $name }'s profile. { $reason }

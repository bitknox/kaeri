## Windows only: sync on the Devices pane.

# Footnote on the recovery key dialog; the lock is Windows Hello.
# @only windows
rkey-lock-windows = You can see it again in Settings, { -device-cap-pl }, with Windows Hello.

# What Windows Hello says it is asking for before the recovery key shows again.
# @only windows
rkey-hello-reason = Show the { -app-name } recovery key

# Word for this device in sentences such as “Turn off sync on this PC”.
# @only windows
device-kind-pc = PC

# Accessible name of the Sync switch on a device's card, with the device's name.
# @only windows
# @arg device: text
devices-sync-switch-name = Sync with { $device }

# Accessible name of the button that opens a device's page from its card.
# @only windows
# @arg device: text
devices-open-name = Open { $device }

# Device page footer on a PC, which drives a TV as the remote.
# @only windows
# @arg device: text
devices-switches-foot-pc = Sync shares where you stopped, your history and your settings. Remote lets this PC drive { $device }.

# Device page footer on a PC when the device has no Remote switch.
# @only windows
devices-sync-foot = Sync shares where you stopped, your history and your settings.

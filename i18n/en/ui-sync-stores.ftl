## Joining a plugin store through a paired device, on the sync screens. The paired device makes an
## invite for this one, so this device gets its own token.

# Line on a store row when joining it through a paired device failed because the store or the
# paired device could not be reached. The person can try again later.
# @arg store: text
# @arg device: text
sync-store-err-offline = Couldn't reach { $store } or { $device }. Try again when both are online.

# Line on a store row when the store turned the invite down, for example because it was cancelled
# or used up.
# @arg store: text
sync-store-err-refused = { $store } didn't accept the invite. Ask its owner for a new one.

# Line on a store row when the paired device can't make invites to the store, because it isn't a
# named member there any more or joined through an open link.
# @arg store: text
# @arg device: text
sync-store-err-not-allowed = { $device } can't invite other { -device-pl } to { $store }. Ask the { -plugin-store }'s owner for an invite.

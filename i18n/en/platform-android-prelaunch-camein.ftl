## Text only the Android phone, tablet and TV apps show when a source is added: the parts of the
## server form, the store form and their rows that the shared catalogue leaves to each platform.

# Where Android keeps a Kavita key, filling { $store } in "Kaeri keeps the key in … and shows only
# its last four characters."
# @only android
book-server-keystore-android = Android’s keystore

# Chip inside the invite field once an invite link is pasted: the invite's code, hidden but for
# its last four characters.
# @only android
# @arg last: text
store-invite-chip = Invite ••••{ $last }

# How many hosts a plugin may contact, filling { $hosts } in "Version 1.4 · may contact 2 hosts".
# @only android
# @arg count: int
store-plugin-hosts = { $count ->
    [one] { $count } host
   *[other] { $count } hosts
 }

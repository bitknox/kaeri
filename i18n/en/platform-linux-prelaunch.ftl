## Linux: what the reading prelaunch pieces say that the shared catalogue leaves to each
## platform. The add forms, Your sources, and first run's join page.

# Chip inside the invite field once an invite link is pasted: the invite's code, hidden but for
# its last four characters.
# @only linux
# @arg last: text
linux-store-invite-chip = Invite ••••{ $last }

# How many hosts a plugin may contact, filling { $hosts } in "Version 1.4 · may contact 2 hosts".
# @only linux
# @arg count: int
linux-store-plugin-hosts = { $count ->
    [one] { $count } host
   *[other] { $count } hosts
 }

# Beside a spinner in the plugin store form while the invite is spent, after 600 ms.
# @only linux
linux-store-joining = Joining…

# The Install all button while the plugins install.
# @only linux
linux-store-installing = Installing…

# Third line of a book server's row in Your sources when its sign-in no longer works.
# @only linux
linux-book-server-signed-out = Signed out · sign in again from Edit

# Switch on a book server's page: its series show in Kaeri while it is on.
# @only linux
linux-book-server-use = Use this server

# Title of the dialog that asks before a book server is removed.
# @only linux
# @arg host: text
linux-book-server-remove-title = Remove { $host }?

# First run's join page, paired: the line under "Kaeri is for" when the other device's answer
# is Watch.
# @only linux
# @arg device: text
linux-join-medium-line-watch = Watching, as on { $device }

# First run's join page, paired: the line under "Kaeri is for" when the other device's answer
# is Both.
# @only linux
# @arg device: text
linux-join-medium-line-both = Watching and reading, as on { $device }

# Switch on a book plugin's page: its series show in Kaeri while it is on.
# @only linux
linux-book-plugin-use = Use this { -plugin }

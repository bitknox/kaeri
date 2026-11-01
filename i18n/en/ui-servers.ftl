## Media servers: adding a Jellyfin server, its page and its errors.

## Media servers: Jellyfin servers the viewer runs, and the steps that add one.

# Title of the dialog that adds a media server.
servers-add-title = Add a media server

# Heading of the servers found on the network.
servers-on-network = On your network

# Row shown while the app looks for servers.
servers-looking = Looking for servers…

# Row when the search ended with nothing found.
servers-none-found = No servers found on this network. Type the address instead.

# Label of the address field.
servers-address = Server address

# Placeholder of the address field.
servers-address-hint = jellyfin.home.example or 192.168.1.20:8096

# Line under the address field where discovery is not available.
servers-address-note = Type your server's address, the one you use to open Jellyfin in a browser.

# Button that checks the address.
# @max 16
servers-continue = { -proceed-cap }

# Title of the sign-in step.
# @arg server: text
servers-sign-in-title = Sign in to { $server }

# Label of the user name field.
servers-user = User name

# Label of the password field.
servers-password = Password

# Link that switches to Quick Connect.
servers-use-quick-connect = Use Quick Connect instead

# Link that switches back to a password.
servers-use-password = Sign in with a password instead

# Line under the sign-in fields. The store is where the token is kept, such as Credential Manager.
# @arg store: text
servers-token-note = Your password isn't saved. { -app-name } keeps a sign-in token in { $store }.

# Where Windows keeps the sign-in token, for the line above.
# @only windows
secure-store-windows = Credential Manager

# Where the sign-in token is kept on Linux, as a phrase in the line above.
# @only linux
secure-store-linux = your keyring

# Where the sign-in token is kept on this device, as the end of the line above.
# @only apple
secure-store-device = your Keychain

# Where Android keeps the sign-in token, as the end of the line above.
# @only android
secure-store-android = this { -device }'s secure storage

# Button that signs in.
# @max 16
servers-sign-in = Sign in

# Title of the Quick Connect step.
servers-qc-title = Enter this code in Jellyfin

# Steps under the Quick Connect code.
servers-qc-steps = On a { -device } that's signed in, open Jellyfin, go to your profile, choose Quick Connect and enter this code.

# Under the code while it waits.
servers-qc-waiting = Waiting for you to approve it

# Error when the code ran out.
servers-qc-expired = That code has run out.

# Button that asks for a new Quick Connect code.
# @max 24
servers-qc-new-code = Get a new code

# Title of the step that picks libraries.
servers-libraries-title = Choose libraries

# Line under it.
servers-libraries-body = Shows are matched to AniList so they join Home, your lists and Search.

# A library's line: what it holds, and how many series.
# @arg kind: text
# @arg count: int
servers-library-line = { $kind } · { $count } series

# Switch on the libraries step and the server's page.
servers-report = Report what you watch to Jellyfin

# Line under that switch.
servers-report-line = Resume points and watched marks follow what you play here.

# Button that finishes the dialog.
# @max 16
servers-add = Add server

# Line of a server's row while it is matched to AniList.
# @arg done: int
# @arg total: int
servers-row-matching = Matching · { $done } of { $total } series

# A server's line once it is read.
# @arg libraries: int
# @arg series: int
servers-row-summary = { $libraries ->
    [one] 1 { -library }
   *[other] { $libraries } libraries
} · { $series } series

# First part of a server's row line: how many libraries it has.
# @arg count: int
servers-row-libraries = { $count ->
    [one] 1 { -library }
   *[other] { $count } libraries
 }

# Second part of a server's row line: how many series it holds.
# @arg count: int
servers-row-series = { $count ->
    [one] 1 series
   *[other] { $count } series
 }

# Line of a server's row after the summary: series that need a look.
# @arg count: int
servers-row-needs-look = { $count } need a look

# Line at the top of a server's page.
# @arg address: text
# @arg version: text
# @arg user: text
servers-page-line = { $address } · Jellyfin { $version } · signed in as { $user }

# Button on a server's page.
# @max 16
servers-sign-out = Sign out

# Button on a server's page.
# @max 24
servers-remove = Remove server

# Error under the address field.
# @arg address: text
servers-error-unreachable = Can't reach { $address }. Check the address and that the server is on.

# Error under the address field.
servers-error-not-jellyfin = That address answers, but it isn't a Jellyfin server.

# Heading of the libraries list on a server's page.
servers-libraries = Libraries

# Line on a server's page saying what removing it does.
servers-remove-line = Nothing is deleted on the server.

# Notice after a server is removed. It holds Undo.
# @arg name: text
servers-removed = { $name } removed

# Error when the server is older than 10.8.
# @arg version: text
servers-error-too-old = This server runs Jellyfin { $version }. { -app-name } needs 10.8 or later.

# Error under the password field.
servers-error-credentials = Wrong user name or password.

# Error that replaces the Quick Connect link.
servers-error-qc-off = Quick Connect is off on this server. Sign in with a password, or turn it on in Jellyfin's dashboard.

# Error under the address field.
servers-error-plain-http = This address isn't on your network and doesn't use HTTPS, so your password would travel unprotected. Use an https:// address.

# Error under the address field when the address cannot be read.
servers-error-bad-address = That doesn't look like an address. Type it the way you open Jellyfin in a browser.

# Error when something else went wrong.
servers-error-other = Something went wrong. Try again.

# Title of the step that asks about a certificate.
servers-cert-title = Check this server

# Body of that step. The fingerprint is shown under it.
# @arg host: text
servers-cert-body = { $host } uses a certificate this { -device } doesn't trust. That's normal for a server at home. Allow it only if this fingerprint matches the one your server shows.

# Button on that step.
# @max 32
servers-cert-allow = Allow this certificate

# Line of a server row when the server signed the viewer out.
servers-row-signed-out = Signed out by the server

# Button on that row.
# @max 24
servers-sign-in-again = Sign in again

# Line of a server row that does not answer.
# @arg time: text
servers-row-not-answering = Not answering · last seen { $time }

# Line on a title page when its server does not answer.
# @arg server: text
# @allow proceed
title-server-not-answering = { $server } isn't answering. Episodes you downloaded still play.

# Kind of a Jellyfin library that holds shows, in a library's line.
servers-kind-shows = Shows

# Kind of a Jellyfin library that holds movies, in a library's line.
servers-kind-movies = Movies

# Kind of any other Jellyfin library, in a library's line.
servers-kind-other = Videos

# Link under the sign-in fields of a server that cannot use Quick Connect, and the name of the step it opens.
servers-quick-connect = Quick Connect

# Heading over a server's libraries.
servers-libraries-heading = Libraries

# Button on a server's page that matches its series to AniList again.
# @max 24
servers-refresh = Match again

# Line on a server's page: how many of its series are not matched for sure.
# @arg count: int
servers-needs-look = { $count } not matched to AniList

# What a Jellyfin library of films is called in a library's line.
servers-kind-films = Films

# Error under the address field when the server's certificate is not trusted.
# @arg host: text
servers-error-certificate = { $host } uses a certificate this { -device } doesn't trust. Use an https:// address with a certificate it trusts.

# Line on a server's row and page after Sign out.
servers-row-signed-out-here = Signed out

# Line under it: what signing out does.
servers-sign-out-line = Ends this { -device }'s sign-in on the server. The server and your downloads stay as they are.

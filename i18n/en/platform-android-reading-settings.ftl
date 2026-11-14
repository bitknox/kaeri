## Text only the Android phone and tablet apps show in Settings with reading on: the lines under
## the Settings list's rows, the book sources and Check matches.

# Line under Notifications in the Settings list: chapter notifications are on.
# @only android
# @max 40
settings-summary-chapters-on = New chapters on

# Line under Notifications in the Settings list: chapter notifications are off.
# @only android
# @max 40
settings-summary-chapters-off = New chapters off

# Part of the line under Sources in the Settings list: how many book folders there are.
# @only android
# @arg n: int
settings-summary-book-folders = { $n ->
    [one] 1 folder
   *[other] { $n } folders
 }

# Part of the line under Sources in the Settings list: how many book servers there are.
# @only android
# @arg n: int
settings-summary-book-servers = { $n ->
    [one] 1 server
   *[other] { $n } servers
 }

# Part of the line under Sources in the Settings list: how many plugins there are for books.
# @only android
# @arg n: int
settings-summary-book-plugins = { $n ->
    [one] 1 { -plugin }
   *[other] { $n } { -plugin-pl }
 }

# Row in Settings' Sources with both media on, and the title of its page: the sources for books.
# @only android
# @max 30
settings-book-sources = Book { -source-pl }

# Line over the Check matches list: how many series of the folders were matched, and how many need a look.
# @only android
# @arg matched: int
# @arg n: int
book-matches-line = { $n ->
    [one] { -app-name } matched { $matched } by their details. This one needs a look.
   *[other] { -app-name } matched { $matched } by their details. These { $n } need a look.
 }

# The folder picker could not be opened, or the folder could not be kept.
# @only android
# @max 80
book-folder-failed = That folder couldn't be added. Try another one.

# Under the sign-in fields of Add a server for books: progress goes back, and where the password is kept on Android.
# @only android
# @max 140
book-server-note-device = { -app-name } sends your reading progress back to this server. The password stays on this { -device }.

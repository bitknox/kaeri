## Words only the Android app shows on a reading series page, its sheets, Send to e-reader and
## the reading part of Downloads.

# Title of the sheet that prepares chapters for an e-reader and hands them to the share sheet. Also
# the menu item that opens it.
# @only android
# @max 30
send-ereader-title = Send to e-reader

# Heading over the e-readers to prepare the files for.
# @only android
# @max 20
send-ereader-for = For

# Heading over what is sent: which chapters.
# @only android
# @max 20
send-ereader-what = What

# The chapters sent, as a span of their numbers: "41 to 43".
# @only android
# @arg from: text
# @arg to: text
# @max 20
send-ereader-range = { $from } to { $to }

# An e-reader's screen size in pixels, beside the file format it gets: "1264 × 1680".
# @only android
# @arg width: int
# @arg height: int
# @max 20
send-ereader-screen = { $width } × { $height }

# A file format for e-readers: an EPUB whose pages keep a fixed layout, for comics.
# @only android
# @max 30
send-ereader-format-fixed-epub = Fixed-layout EPUB

# The EPUB format made for Kobo e-readers. Kobo is a product name: keep it as it is.
# @only android
# @max 20
send-ereader-format-kepub = Kobo EPUB

# The plain EPUB e-book format. Keep as it is unless your language writes it otherwise.
# @only android
# @max 20
send-ereader-format-epub = EPUB

# The comic book archive format. Keep as it is unless your language writes it otherwise.
# @only android
# @max 20
send-ereader-format-cbz = CBZ

# Line in the sheet before preparing: reading progress on the e-reader never comes back to the app.
# @only android
send-ereader-cant-see = { -app-name } can't see your place on this e-reader. When you come back, the series page asks where you stopped.

# The sheet's one main button: make the files, then open the share sheet with them.
# @only android
# @max 30
send-ereader-prepare = Prepare and share

# Beside the progress ring while the files are made.
# @only android
# @arg done: int
# @arg total: int
# @max 50
send-ereader-preparing = { $total ->
    [one] Preparing the chapter
   *[other] Preparing { $done } of { $total } chapters
}

# Under the progress while the files are made: they are fitted to the chosen e-reader's screen.
# @only android
# @arg device: text
# @max 50
send-ereader-sized = Pages are sized for the { $device } screen

# Heading when the files could not be made. The cause follows on its own line.
# @only android
# @max 50
send-ereader-failed = Couldn't prepare these chapters.

# Title of the sheet that asks how far the person read on the e-reader. Also the button that opens it.
# @only android
# @max 30
send-ereader-where-stop = Where did you stop?

# On the series page after a send: which e-reader the chapters went to and on which day.
# @only android
# @arg device: text
# @arg date: text
# @max 50
send-ereader-sent = Sent to { $device } on { $date }

# On the series page after a send, under where the chapters went.
# @only android
# @max 50
send-ereader-cant-see-there = { -app-name } can't see your place there.

# Line at the top of Where did you stop: how many chapters went to which e-reader, and when.
# @only android
# @arg n: int
# @arg device: text
# @arg date: text
send-ereader-went = { $n ->
    [one] One chapter went to { $device } on { $date }.
   *[other] { $n } chapters went to { $device } on { $date }.
}

# Switch in Where did you stop: the chosen chapter was read to its end. The chapter is the
# source's own label, such as "Chapter 43" or "Part 5".
# @only android
# @arg chapter: text
# @max 40
send-ereader-finished = I finished { $chapter }

# The main button of Where did you stop: move the place to the chosen chapter.
# @only android
# @max 30
send-ereader-set-place = Set my place

# Text button in Where did you stop that leaves the place as it is, naming it.
# @only android
# @arg n: text
# @arg page: int
# @max 40
send-ereader-keep-chapter = Keep chapter { $n } · page { $page }

# Text button in Where did you stop that leaves the place as it is.
# @only android
# @max 30
send-ereader-keep = Keep my place

# On a series page whose source failed, under the cause: the chapters downloaded before still open.
# @only android
# @arg n: int
read-series-downloaded-open = { $n ->
    [one] One downloaded chapter still opens.
   *[other] { $n } downloaded chapters still open.
}

# Downloads' empty line for a person who only reads, under "No downloads".
# @only android
read-downloads-empty-detail = Downloaded chapters open without a connection.

# Downloads' empty line for a person who watches and reads, under "No downloads".
# @only android
read-downloads-empty-both = Downloaded episodes and chapters open without a connection.

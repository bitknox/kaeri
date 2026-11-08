# Messages about reading manga and novels: the words on covers and series pages, the reader's
# notices, and why something could not be read. Each one is shown in place, unless it says
# otherwise. Reading is off in release builds until it launches.

# Under a cover: the chapter that is next to read. The chapter is its number, such as 44 or 10.5.
# @arg chapter: text
# @max 24
reading-next-chapter = Chapter { $chapter }

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-mondays = Usually Mondays

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-tuesdays = Usually Tuesdays

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-wednesdays = Usually Wednesdays

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-thursdays = Usually Thursdays

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-fridays = Usually Fridays

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-saturdays = Usually Saturdays

# Under a cover or on a series page: when new chapters usually come out.
# @max 24
reading-usually-sundays = Usually Sundays

# Under a cover or on a series page: how often new chapters come out, in weeks.
# @arg weeks: int
# @max 32
reading-about-every-weeks = { $weeks ->
    [one] About every week
   *[other] About every { $weeks } weeks
 }

# On a book's series page, when the same story is also a show. Shown only to someone who both
# watches and reads.
# @max 24
reading-also-a-show = Also a show

# On a show's page, when the same story is also a manga or a novel. Shown only to someone who
# both watches and reads.
# @max 24
reading-also-a-book = Also a book

# Under a tracker on a series page, when the furthest chapter read is a half chapter. Trackers
# count whole chapters only. The chapter is its number, such as 10.5; whole is the number the
# tracker gets, such as 10.
# @arg chapter: text
# @arg whole: text
reading-split-chapter = Chapter { $chapter } goes to { -tracker-pl } as { $whole }.

# A notice over the page when a chapter opens where it was left. It offers Start over.
# @arg page: plain
# @max 32
reader-resumed = Resumed at page { $page }

# A notice in the corner after jumping away with the scrubber or the page grid. Tapping it goes
# back to the page.
# @arg page: plain
# @max 24
reader-back-to-page = Back to page { $page }

# A notice when another device read further, so the reader moved on to where it stopped. It
# offers Go back. The chapter is its whole label, such as The Long Night or Episode 63, and the
# device is its name, such as Living room tablet. A chapter with only a number uses the next
# message.
# @arg chapter: text
# @arg page: plain
# @arg device: text
reader-moved = Moved to { $chapter }, page { $page }, where { $device } stopped

# The same notice for a chapter known only by its number, such as 14 or 14.5.
# @arg chapter: text
# @arg page: plain
# @arg device: text
reader-moved-number = Moved to chapter { $chapter }, page { $page }, where { $device } stopped

# A notice the first time a series opens with its reading direction guessed. It offers Change.
# @max 32
reader-detected-right-to-left = Looks like right to left

# A notice the first time a series opens with its reading direction guessed. It offers Change.
# @max 32
reader-detected-left-to-right = Looks like left to right

# A notice the first time a series opens as one long strip, guessed from its pages. It offers
# Change.
# @max 32
reader-detected-strip = Looks like a long strip

# Why a chapter or a list could not load: there is no connection.
reading-failed-offline = You're offline. Downloaded chapters still open.

# Why a chapter or a list could not load: the source did not answer. The source is its name.
# @arg source: text
reading-failed-source-down = { $source } isn't answering. Try again later.

# Why a chapter could not load: the source no longer has it.
reading-failed-not-found = This { -source } doesn't have it anymore. Try another { -source }.

# Why a chapter or a list could not load: the source refused. The reason comes from the source.
# @arg source: text
# @arg reason: text
reading-failed-blocked = { $source } refused the request. { $reason }

# Why a chapter or a list could not load: the source answered with something Kaeri could not read.
# @arg source: text
reading-failed-parse = { -app-name } couldn't read what { $source } sent. Its { -plugin } may need an update.

# Why one page did not load. The status is the server's HTTP status code, such as 404.
# @arg status: plain
reading-failed-page = This page didn't load (error { $status }). Try again.

# Why something could not load: a sign-in is needed. What is a server's or a tracker's name.
# @arg what: text
reading-failed-sign-in = Sign in to { $what } again to keep reading.

# Why a book could not open: its file or folder is gone, perhaps on a drive that is not
# connected. The path is the file's.
# @arg path: text
reading-failed-file-missing = { $path } isn't there anymore. If it's on a drive, connect it and try again.

# Why a book could not open: the file is damaged or not what its name says. The format is a
# short name such as CBZ or EPUB.
# @arg path: text
# @arg format: text
reading-failed-unreadable = { $path } couldn't be read as { $format }. The file may be damaged.

# Why a file was not added: Kaeri does not read its kind. The kind is a file extension such as .djvu.
# @arg kind: text
reading-failed-unsupported = { -app-name } can't read { $kind } files.

# Why a book could not open: it is copy-protected or encrypted.
reading-failed-protected = This book is copy-protected, so { -app-name } can't open it.

# Why a chapter could not be saved: the disk is full.
reading-failed-disk-full = There's no room left on this { -device }. Free some space and try again.

# Why a novel could not open: the system's web view is too old. The version is its major version
# number, such as 80.
# @arg version: plain
reading-failed-webview = Novels need a newer system WebView than version { $version }. Update Android System WebView and try again.

# Why a book plugin could not be used: it was built for a newer Kaeri. The world is the plugin
# interface's name and version.
# @arg world: text
reading-failed-needs-newer = This { -plugin } needs a newer { -app-name } ({ $world }). Update { -app-name } to use it.

# Why a reading request was refused: reading is not available in this build.
reading-failed-off = Reading isn't available in this version of { -app-name }.

# Why something about reading failed, for any other reason. The reason is as it came.
# @arg reason: text
reading-failed-other = Something went wrong. { $reason }

# Why Send to e-reader stopped before sending: the file it made is bigger than the e-reader or
# its service takes. File is the file's name, target the e-reader's name such as Kindle. Shown
# with Try again.
# @arg file: text
# @arg target: text
# @arg size: bytes
# @arg limit: bytes
reading-failed-send-too-large = { $file } is { $size }, and { $target } takes files up to { $limit }. Send fewer chapters at a time and try again.

# Why Send to e-reader stopped before sending: a part the book needs is not in the file Kaeri
# made. Part is a path inside the file, such as OEBPS/content.opf. Shown with Try again.
# @arg file: text
# @arg part: text
reading-failed-send-missing = { $file } is missing { $part }, so it wasn't sent. Try again.

# Why Send to e-reader stopped before sending: a part of the file Kaeri made does not declare or
# use the UTF-8 text encoding. Part is a path inside the file. Shown with Try again.
# @arg file: text
# @arg part: text
reading-failed-send-encoding = { $part } in { $file } has the wrong text encoding, so it wasn't sent. Try again.

# Why Send to e-reader stopped before sending: a part of the file Kaeri made is not laid out as
# the book format requires. Part is a path inside the file. Shown with Try again.
# @arg file: text
# @arg part: text
reading-failed-send-malformed = { $part } in { $file } isn't valid, so it wasn't sent. Try again.

# Under a cover or on a series page: what kind of book it is. Manga is drawn in Japan.
# @max 16
reading-kind-manga = Manga

# Under a cover or on a series page: what kind of book it is. Manhwa is drawn in Korea.
# @max 16
reading-kind-manhwa = Manhwa

# Under a cover or on a series page: what kind of book it is. Manhua is drawn in China or Taiwan.
# @max 16
reading-kind-manhua = Manhua

# Under a cover or on a series page: what kind of book it is. A webtoon is drawn as one long
# strip that scrolls down.
# @max 16
reading-kind-webtoon = Webtoon

# Under a cover or on a series page: what kind of book it is. A one-shot is a story told in a
# single chapter.
# @max 16
reading-kind-one-shot = One-shot

# Under a cover or on a series page: what kind of book it is. A light novel is a novel from Japan.
# @max 16
reading-kind-light-novel = Light novel

# Under a cover or on a series page: what kind of book it is, for a novel from anywhere else.
# @max 16
reading-kind-novel = Novel

# Under a cover or on a series page: new chapters are still coming out.
# @max 16
reading-status-releasing = Releasing

# Under a cover or on a series page: the story is complete.
# @max 16
reading-status-finished = Finished

# Under a cover or on a series page: new chapters have stopped for a while.
# @max 16
reading-status-hiatus = On hiatus

# Under a cover or on a series page: the story was stopped before it ended.
# @max 16
reading-status-cancelled = Cancelled

# Under a cover or on a series page: the first chapter is not out yet.
# @max 16
reading-status-not-yet = Not out yet

# Under a cover or on a series page: how many chapters the series has.
# @arg count: int
# @max 24
reading-chapters = { $count ->
    [one] 1 chapter
   *[other] { $count } chapters
 }

# Under a cover or on a series page: how many volumes the series has.
# @arg count: int
# @max 24
reading-volumes = { $count ->
    [one] 1 volume
   *[other] { $count } volumes
 }

# Under a cover: the series is on the person's Planning list.
# @max 32
reading-on-planning = On your Planning list

# Above the big picture at the top of reading's Home, saying why this series is there: a new
# chapter came out in a series the person reads.
# @max 48
reading-spotlight-new-chapter = New chapter in a series you follow

# Above the big picture at the top of reading's Home: the series the person was reading last.
# @max 40
reading-spotlight-continue = { -continue-cap } reading

# Above the big picture at the top of reading's Home: the first series on the person's Planning
# list, or one like what they read.
# @max 40
reading-spotlight-up-next = { -up-next-cap } for you

# Above the big picture at the top of reading's Home: a popular series that is still coming out,
# shown when the person has nothing of their own yet. It is not a pick for them.
# @max 40
reading-spotlight-popular = Popular, releasing now

# Under the big picture on reading's Home: a chapter that came out today. The chapter is its
# number, such as 88.
# @arg chapter: text
# @max 40
reading-out-today = Chapter { $chapter } is out today

# Under the big picture on reading's Home: a chapter that came out today, with its title.
# @arg chapter: text
# @arg title: text
reading-out-today-titled = Chapter { $chapter }, { $title }, out today

# Under a cover or the big picture: the page the person stopped on in a chapter.
# @arg chapter: text
# @arg page: plain
# @max 32
reading-chapter-page = Chapter { $chapter } · page { $page }

# Under a cover or the big picture: how far into a novel's chapter the person is.
# @arg chapter: text
# @arg percent: int
# @max 32
reading-chapter-percent = Chapter { $chapter } · { $percent }%

# Under a cover: a chapter came out that the person has not read. The chapter is its number.
# @arg chapter: text
# @max 32
reading-chapter-new = Chapter { $chapter } is new

# Under a cover: a light novel's part, next to read or in progress. Light novels read in parts.
# @arg part: text
# @max 24
reading-next-part = Part { $part }

# Under a cover or the big picture: how far into a light novel's part the person is.
# @arg part: text
# @arg percent: int
# @max 32
reading-part-percent = Part { $part } · { $percent }%

# Under a cover: a light novel's part came out that the person has not read.
# @arg part: text
# @max 32
reading-part-new = Part { $part } is new

# Under a cover or the big picture: how far into a light novel's part the person is, the part
# named in the source's own words ("Part 3", "Chapter 3"), as its row on the series page names it.
# @arg label: text
# @arg percent: int
# @max 40
reading-label-percent = { $label } · { $percent }%

# Under a cover: a light novel's part came out that the person has not read, named in the
# source's own words ("Part 4", "Chapter 4").
# @arg label: text
# @max 40
reading-label-new = { $label } is new

# Under a This week cell: a light novel's part that came out that day, with its own title.
# @arg part: text
# @arg title: text
# @max 40
reading-part-titled = Part { $part } · { $title }

# Under a This week cell: a light novel's part that came out that day, with its own title, the
# part named in the source's own words ("Part 4", "Chapter 4").
# @arg label: text
# @arg title: text
# @max 40
reading-label-titled = { $label } · { $title }

# Under the big picture on reading's Home: a light novel's part that came out today.
# @arg part: text
# @max 40
reading-part-out-today = Part { $part } is out today

# Under the big picture on reading's Home: a light novel's part that came out today, named in the
# source's own words ("Part 4", "Chapter 4").
# @arg label: text
# @max 40
reading-label-out-today = { $label } is out today

# Under the big picture on reading's Home: a light novel's part that came out today, with its
# title.
# @arg part: text
# @arg title: text
reading-part-out-today-titled = Part { $part }, { $title }, out today

# Under the big picture on reading's Home: a light novel's part that came out today, with its
# title, the part named in the source's own words ("Part 4", "Chapter 4").
# @arg label: text
# @arg title: text
reading-label-out-today-titled = { $label }, { $title }, out today

# Under a Library cover: the chapter the person is at, and the newest chapter there is.
# @arg chapter: text
# @arg of: text
# @max 32
reading-chapter-of = Chapter { $chapter } of { $of }

# Under a Library cover: the volume and part of a light novel the person is at.
# @arg volume: text
# @arg part: text
# @max 32
reading-volume-part = Volume { $volume }, part { $part }

# Under a Library cover: the volume of a light novel the person is at, and the part in the
# source's own words ("Part 5", "Chapter 5").
# @arg volume: text
# @arg label: text
# @max 40
reading-volume-label = Volume { $volume }, { $label }

# A search filter: books from Japan.
# @max 16
reading-origin-japan = Japan

# A search filter: books from Korea.
# @max 16
reading-origin-korea = Korea

# A search filter: books from China.
# @max 16
reading-origin-china = China

# A search filter: books from Taiwan.
# @max 16
reading-origin-taiwan = Taiwan

# Send to e-reader: a device class with no brand, for e-readers not listed by name. Opens CBZ and EPUB.
# @max 28
export-device-other = Other e-reader

# Send to e-reader: a device class, a grey e-ink screen of about 6 inches.
# @max 28
export-device-eink-6 = E-ink, 6 inch

# Send to e-reader: a device class, a grey e-ink screen of about 7 or 8 inches.
# @max 28
export-device-eink-7 = E-ink, 7 inch

# Send to e-reader: a device class, a colour e-ink screen of about 7 inches.
# @max 28
export-device-colour-eink = Colour e-ink, 7 inch

# Send to e-reader: a device class, a colour tablet.
# @max 28
export-device-tablet = Tablet

# A table of contents entry in a file sent to an e-reader. The chapter is its number, such as 44 or 10.5.
# @arg chapter: text
# @max 24
export-chapter = Chapter { $chapter }

# Send to e-reader: a send that waits because chapters must be fetched and downloads are kept to
# Wi-Fi. A send makes its files from every chapter at once, so all of it waits. n is how many
# chapters it still has to fetch.
# @arg n: int
export-waits-for-wifi = { $n ->
    [one] One chapter isn't downloaded yet, so this send waits for Wi-Fi and goes on by itself.
   *[other] { $n } chapters aren't downloaded yet, so this send waits for Wi-Fi and goes on by itself.
}
# Under a This week cell: the chapter that came out that day, with its own title.
# @arg chapter: text
# @arg title: text
# @max 40
reading-chapter-titled = Chapter { $chapter } · { $title }

## Chapter rows
## The second line of a chapter row is made of these parts, joined with " · ", such as
## "Read · 4 days ago · Downloaded" or "New · yesterday".

# A chapter row's state: the person read it.
# @max 16
reading-row-read = Read

# A chapter row's state: it came out since the person last read and is not read.
# @max 16
reading-row-new = New

# A chapter row's state: it is kept on this device to read offline.
# @max 20
reading-row-downloaded = Downloaded

# A chapter row's state: it is downloaded on this device, and the source no longer lists it. It
# still opens from the download.
# @max 28
reading-row-gone-from-source = Gone from the { -source }

# A chapter row's state: it is read from the source and nothing of it is on this device.
# @max 20
reading-row-not-downloaded = Not downloaded

# A chapter row's state: a file in one of the person's folders, or one already on the device.
# @max 20
reading-row-on-device = On this { -device }

# A chapter row's state: it waits its turn to download.
# @max 20
reading-row-queued = Queued

# A chapter row's state: it waits to download until the device is on Wi-Fi.
# @max 32
reading-row-waits-wifi = Queued · waits for Wi-Fi

# A chapter row's state while it downloads.
# @arg done: int
# @arg total: int
# @max 44
reading-row-downloading = Downloading · { $done } of { $total } pages

# A chapter row's state: the download failed. The reason follows as the next part.
# @max 24
reading-row-failed = Couldn't download

# A chapter row's state: the page the person stopped on, of how many.
# @arg page: plain
# @arg of: plain
# @max 24
reading-row-page-of = Page { $page } of { $of }

# A chapter row's state: the page the person stopped on, when the length isn't known.
# @arg page: plain
# @max 16
reading-row-page = Page { $page }

# A novel's chapter row: how far into it the person is.
# @arg percent: int
# @max 8
reading-row-percent = { $percent }%

# A novel's chapter row: the person opened it, and how far in is too little to say as a percent.
# @max 16
reading-row-opened = Opened

# A chapter row's state: the person bookmarked it. Bookmarked chapters are kept when read
# downloads are cleared.
# @max 16
reading-row-bookmarked = Bookmarked

# A chapter row: how long its pages are, for a file on the device.
# @arg n: int
# @max 16
reading-row-pages = { $n ->
    [one] 1 page
   *[other] { $n } pages
 }

# A chapter row: how many versions the person's sources have of it. The row's menu lists them.
# @arg n: int
# @max 20
reading-row-versions = { $n ->
    [one] 1 version
   *[other] { $n } versions
 }

# A chapter row: when it was read or came out, under a minute ago.
# @max 16
reading-ago-now = just now

# A chapter row: when it was read or came out, in minutes.
# @arg minutes: int
# @max 16
reading-ago-minutes = { $minutes } min ago

# A chapter row: when it was read or came out, earlier today.
# @max 16
reading-ago-today = today

# A chapter row: when it was read or came out, the day before today.
# @max 16
reading-ago-yesterday = yesterday

# A chapter row: when it was read or came out, in days, under a week.
# @arg days: int
# @max 20
reading-ago-days = { $days ->
    [one] 1 day ago
   *[other] { $days } days ago
 }

# A chapter row: when it was read or came out, in weeks, under two months.
# @arg weeks: int
# @max 20
reading-ago-weeks = { $weeks ->
    [one] 1 week ago
   *[other] { $weeks } weeks ago
 }

# A chapter row: when it was read or came out, in months, under a year.
# @arg months: int
# @max 20
reading-ago-months = { $months ->
    [one] 1 month ago
   *[other] { $months } months ago
 }

# A chapter row: when it was read or came out, in years.
# @arg years: int
# @max 20
reading-ago-years = { $years ->
    [one] 1 year ago
   *[other] { $years } years ago
 }

## Remove from lists
## One line per tracker after the person takes a series off their lists. A failed line is
## followed by its reason as the next part, joined with " · ".

# The tracker took the series off its list.
# @arg tracker: text
reading-removed-from = Removed from { $tracker }

# The tracker's list did not hold the series, so there was nothing to remove.
# @arg tracker: text
reading-removed-not-there = Wasn't on your { $tracker } list

# The app never learned which of the tracker's entries this series is, so it asked nothing.
# @arg tracker: text
reading-removed-unmatched = Not matched on { $tracker }, so nothing changed there

# The tracker could not take the series off. The reason follows.
# @arg tracker: text
reading-removed-failed = Still on your { $tracker } list

# Why a removal failed: the tracker turned the sign-in away.
# @arg tracker: text
reading-removed-sign-in = Sign in to { $tracker } again, then try once more.

## Not found in
## Read from lists the sources that were searched for a series without a match, each with why.

# The source answered the search and has no series by the series' names.
reading-miss-no-series = No series with this name

# The source has failed every ask since this time of day, in the person's zone.
# @arg time: text
reading-miss-down-since = Not answering since { $time }

# The source has failed every ask, since a time on another day or not known.
reading-miss-down = Not answering

## Downloads by series
## Each series on Downloads gets one line: "6 chapters · 214 MB · 2 read", or while one downloads
## "Chapter 42 · 12 of 19 pages · 2 more queued". The Reading section's total uses the first two
## parts: "38 chapters · 1.1 GB". Parts are joined with " · ".

# How many parts of a light novel are downloaded.
# @arg count: int
reading-dl-parts = { $count ->
    [one] 1 part
   *[other] { $count } parts
 }

# What the downloads take up on this device.
# @arg size: bytes
reading-dl-size = { $size }

# How many of the downloaded chapters are read, which Delete read clears.
# @arg count: int
reading-dl-read = { $count } read

# The chapter downloading now.
# @arg chapter: text
reading-dl-chapter = Chapter { $chapter }

# How far the chapter downloading now has come.
# @arg done: int
# @arg total: int
reading-dl-pages = { $done } of { $total } pages

# Chapters waiting their turn after the one downloading.
# @arg count: int
reading-dl-more-queued = { $count } more queued

# Chapters waiting their turn, none downloaded yet.
# @arg count: int
reading-dl-queued = { $count } queued

# Chapters that wait for Wi-Fi to download.
reading-dl-waits-wifi = waits for Wi-Fi

# A chapter that failed. The reason follows as the next part.
# @arg chapter: text
reading-dl-failed = Chapter { $chapter } couldn't download

## Download estimate
## Above the Download button when choosing chapters: "About 62 MB · over Wi-Fi".

# How much the chosen chapters will take, from their pages and the size of pages downloaded
# before.
# @arg size: bytes
reading-estimate-size = About { $size }

# The chapters download only on Wi-Fi, as the settings say.
reading-estimate-wifi = over Wi-Fi

# The device is on mobile data, and the settings let chapters download on it.
reading-estimate-mobile = over mobile data

# The device is on mobile data and the chapters wait until it's on Wi-Fi.
reading-estimate-waits-wifi = waits for Wi-Fi

## Downloads finished
## The notification a native app shows when the chapters it was downloading are all done, when
## the person turned it on.

# The notification's title.
reading-downloads-done-title = Downloads finished

# The notification's text: one series' chapters are downloaded.
# @arg title: text
# @arg count: int
reading-downloads-done-chapters = { $count ->
    [one] A chapter of { $title } is ready to read offline.
   *[other] { $count } chapters of { $title } are ready to read offline.
 }

# The notification's text: one light novel's parts are downloaded.
# @arg title: text
# @arg count: int
reading-downloads-done-parts = { $count ->
    [one] A part of { $title } is ready to read offline.
   *[other] { $count } parts of { $title } are ready to read offline.
 }

# The notification's text: chapters of several series are downloaded.
# @arg count: int
reading-downloads-done-series = Chapters of { $count } series are ready to read offline.

## Read's discovery rows
## The rows of reading's Home after the person's own sections, each a heading over a row of
## covers from AniList. Light novels, manga and webtoons each get their own rows.

# Row heading on reading's Home: manga that are trending on AniList now.
# @max 32
read-home-trending-manga = Trending manga

# Row heading on reading's Home: webtoons that are trending on AniList now.
# @max 32
read-home-trending-webtoons = Trending webtoons

# Row heading on reading's Home: light novels that are trending on AniList now.
# @max 32
read-home-trending-novels = Trending light novels

# Row heading on reading's Home: the most popular manga that started in the last 12 months.
# @max 32
read-home-new-manga = New manga

# Row heading on reading's Home: the most popular webtoons that started in the last 12 months.
# @max 32
read-home-new-webtoons = New webtoons

# Row heading on reading's Home: the most popular light novels that started in the last 12 months.
# @max 32
read-home-new-novels = New light novels

# Side text beside a New row's heading, in a quieter style: what the row covers.
# @max 24
read-home-new-side = Last 12 months

# Row heading on reading's Home: the most popular manga that have finished.
# @max 32
read-home-finished-manga = Finished manga

# Row heading on reading's Home: the most popular webtoons that have finished.
# @max 32
read-home-finished-webtoons = Finished webtoons

# Row heading on reading's Home: the most popular light novels that have finished.
# @max 32
read-home-finished-novels = Finished light novels

# Side text beside a Finished row's heading, in a quieter style: how the row is sorted.
# @max 24
read-home-finished-side = Most popular

# A choice of reading Search's Started filter: series that started at any time.
# @max 24
read-search-started-any = Any time

# A choice of reading Search's Started filter: series that started in the last 12 months.
# @max 24
read-search-started-year = Last 12 months

# A choice of reading Search's Started filter: series that started in the last 3 years.
# @max 24
read-search-started-3-years = Last 3 years

# Under a source's hit in Search: the chapters it holds, from the first.
# @arg last: plain
# @max 24
read-hit-chapters = Chapters 1 to { $last }

# Under a source's hit in Search: a web novel's episodes it holds, from the first.
# @arg last: plain
# @max 24
read-hit-episodes = Episodes 1 to { $last }

# Under a source's hit in Search: a light novel's parts it holds.
# @arg count: int
# @max 24
read-hit-parts = { $count ->
    [one] 1 part
   *[other] { $count } parts
 }

# Under a source's hit in Search: the book files it holds.
# @arg count: int
# @max 24
read-hit-files = { $count ->
    [one] 1 file
   *[other] { $count } files
 }

# Under a source's hit in Search: the book files it holds, all in one format ("CBZ").
# @arg count: int
# @arg format: text
# @max 24
read-hit-files-format = { $count ->
    [one] 1 file · { $format }
   *[other] { $count } files · { $format }
 }

# The name of the folders on this device, which read as one source: a Search group's heading,
# the source a series reads from, "In Folders" on a search result. It stands alone, with no
# word for its kind beside it.
# @max 24
read-source-folders = Folders

# A section of the feed that e-readers fetch from: the chapters downloaded on this device.
# @max 24
read-feed-downloads = Downloads

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since January".
# @max 20
reading-since-january = since January

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since February".
# @max 20
reading-since-february = since February

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since March".
# @max 20
reading-since-march = since March

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since April".
# @max 20
reading-since-april = since April

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since May".
# @max 20
reading-since-may = since May

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since June".
# @max 20
reading-since-june = since June

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since July".
# @max 20
reading-since-july = since July

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since August".
# @max 20
reading-since-august = since August

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since September".
# @max 20
reading-since-september = since September

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since October".
# @max 20
reading-since-october = since October

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since November".
# @max 20
reading-since-november = since November

# On a cover in a New row, after the country: the month the series started, within the last
# 12 months, as in "Japan · since December".
# @max 20
reading-since-december = since December

# The line over Search's results opened by View all on a Trending row: the format and the order. The count
# follows it: "Manga, trending · 1,920 series from AniList".
# @max 60
read-view-all-trending-manga = Manga, trending

# The line over Search's results opened by View all on a Trending row: the format and the order. The count
# follows it: "Webtoons, trending · 1,920 series from AniList".
# @max 60
read-view-all-trending-webtoons = Webtoons, trending

# The line over Search's results opened by View all on a Trending row: the format and the order. The count
# follows it: "Light novels, trending · 1,920 series from AniList".
# @max 60
read-view-all-trending-novels = Light novels, trending

# The line over Search's results opened by View all on a New row: the format and the filter. The count
# follows it: "Manga started in the last 12 months · 1,920 series from AniList".
# @max 60
read-view-all-new-manga = Manga started in the last 12 months

# The line over Search's results opened by View all on a New row: the format and the filter. The count
# follows it: "Webtoons started in the last 12 months · 1,920 series from AniList".
# @max 60
read-view-all-new-webtoons = Webtoons started in the last 12 months

# The line over Search's results opened by View all on a New row: the format and the filter. The count
# follows it: "Light novels started in the last 12 months · 1,920 series from AniList".
# @max 60
read-view-all-new-novels = Light novels started in the last 12 months

# The line over Search's results opened by View all on a Finished row: the format and the order. The count
# follows it: "Finished manga, most popular · 1,920 series from AniList".
# @max 60
read-view-all-finished-manga = Finished manga, most popular

# The line over Search's results opened by View all on a Finished row: the format and the order. The count
# follows it: "Finished webtoons, most popular · 1,920 series from AniList".
# @max 60
read-view-all-finished-webtoons = Finished webtoons, most popular

# The line over Search's results opened by View all on a Finished row: the format and the order. The count
# follows it: "Finished light novels, most popular · 1,920 series from AniList".
# @max 60
read-view-all-finished-novels = Finished light novels, most popular

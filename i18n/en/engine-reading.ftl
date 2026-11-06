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
# @arg page: int
# @max 32
reader-resumed = Resumed at page { $page }

# A notice in the corner after jumping away with the scrubber or the page grid. Tapping it goes
# back to the page.
# @arg page: int
# @max 24
reader-back-to-page = Back to page { $page }

# A notice when another device read further, so the reader moved on to where it stopped. It
# offers Go back. The device is its name, such as Living room tablet.
# @arg chapter: text
# @arg page: int
# @arg device: text
reader-moved = Moved to chapter { $chapter }, page { $page }, where { $device } stopped

# A notice the first time a series opens with its reading direction guessed. It offers Change.
# @max 32
reader-detected-right-to-left = Right to left, detected

# A notice the first time a series opens with its reading direction guessed. It offers Change.
# @max 32
reader-detected-left-to-right = Left to right, detected

# A notice the first time a series opens as one long strip, guessed from its pages. It offers
# Change.
# @max 32
reader-detected-strip = Long strip, detected

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
# @arg status: int
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
# @arg version: int
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
# @arg page: int
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

## The Linux reader: its toolbar, its end page and its settings popover. The wording is the Apple reader's.

# Tooltip of the reader toolbar's settings button.
# @only linux
# @max 24
linux-reader-settings-open = Reader settings

# Line shown while a chapter opens, after the first moment.
# @only linux
# @max 30
linux-reader-opening = Opening the chapter

# End page heading when the next chapter is not out yet.
# @only linux
# @max 40
linux-reader-end-not-out = The next chapter isn't out yet

# End page heading at the end of a finished series.
# @only linux
# @max 40
linux-reader-end-finished = End of the series

# End page heading when the next chapter is not downloaded and the device is offline.
# @only linux
# @max 48
linux-reader-end-offline = The next chapter isn't downloaded

# End page heading when no source has the next chapter.
# @only linux
# @max 48
linux-reader-end-no-source = No next chapter in your { -source-pl } yet

# Button on the end page that goes back to the series page.
# @only linux
# @max 28
linux-reader-back-to-series = Back to the series

# Heading of the error when a chapter would not open.
# @only linux
# @max 40
linux-reader-chapter-failed = Couldn't open the chapter

# Names of the reader settings in its popover.
# @only linux
# @max 20
linux-reader-reading-mode = Reading mode

# @only linux
# @max 20
linux-reader-fit = Fit

# @only linux
# @max 20
linux-reader-crop-borders = Crop borders

# @only linux
# @max 20
linux-reader-page-layout = Page layout

# @only linux
# @max 20
linux-reader-background = Background

# @only linux
# @max 20
linux-reader-look = Look

# @only linux
# @max 20
linux-reader-font = Font

# @only linux
# @max 20
linux-reader-text-size = Text size

# @only linux
# @max 20
linux-reader-margins = Margins

# @only linux
# @max 20
linux-reader-line-spacing = Line spacing

# @only linux
# @max 20
linux-reader-justify = Justify text

# @only linux
# @max 20
linux-reader-direction = Direction

# @only linux
# @max 20
linux-reader-spreads = Wide pages

# @only linux
# @max 20
linux-reader-strip-width = Strip width

# @only linux
# @max 22
linux-reader-strip-gap = Image gap

# The fit that fills the window's height.
# @only linux
# @max 16
linux-reader-fit-height = Height

# Reading mode names.
# @only linux
# @max 16
linux-reader-mode-paged = Paged

# @only linux
# @max 16
linux-reader-mode-scrolled = Scroll

# Title of the sheet that prepares chapters as files for an e-reader.
# @only linux
# @max 24
linux-send-title = Send to e-reader

# Button that starts preparing the files.
# @only linux
# @max 20
linux-send-prepare = Prepare files

# Line while the files are prepared.
# @only linux
# @arg done: int
# @arg total: int
# @max 40
linux-send-progress = Preparing { $done } of { $total }

# Button that copies the prepared files to a folder the person picks.
# @only linux
# @max 28
linux-send-save = Save to a folder

# Menu item of a series that opens that sheet.
# @only linux
# @max 24
linux-send-menu = Send to e-reader

# Heading when the web view that shows a novel cannot start.
# @only linux
# @max 40
linux-reader-novel-unavailable = Novels can't open here

# Line under it: what a novel needs on this computer.
# @only linux
# @max 80
linux-reader-novel-needs-webkit = Novels need WebKitGTK 6 with its sandbox working.

# The reader's scrubber as a screen reader says it: the page showing and how many there are.
# @only linux
# @max 40
# @arg page: plain
# @arg pages: plain
linux-reader-page-of = Page { $page } of { $pages }

# Tab of the reader's Chapters popover with the chapter's pages as small pictures; the other tab
# is the list of chapters.
# @only linux
# @max 14
linux-reader-pages = Pages

## The reader's end page, failed page, notices and settings headings. The wording is the Apple
## reader's, with "this computer" for the device.

# Line on the end page for a chapter that has not counted as read yet.
# @only linux
# @max 30
linux-reader-end-not-read = Not marked read yet

# The one amber button of the end page: opens the next chapter.
# @only linux
# @max 20
linux-reader-continue = { -continue-cap }

# End page text when the next chapter is not out yet.
# @only linux
# @max 110
linux-reader-end-not-out-body = { -app-name } checks for new chapters in the background and adds them to Updates.

# End page text when the reader has caught up with the series.
# @only linux
# @max 110
linux-reader-end-caught-up-body = New chapters show in Updates when your { -source-pl } have them.

# End page text at the end of a finished series.
# @only linux
# @max 80
linux-reader-end-finished-body = There are no more chapters.

# End page text when the next chapter is not downloaded and the computer is offline.
# @only linux
# @max 110
linux-reader-end-offline-body = You're offline. It opens here once you're connected.

# Hint under a page's error: turning the page goes on past it.
# @only linux
# @max 60
linux-reader-page-failed-hint = Turn the page to go on without it.

# Line while a chapter opens, once its number is known.
# @arg chapter: text
# @only linux
# @max 40
linux-reader-opening-number = Opening chapter { $chapter }

# Line while a chapter opens and a source is asked for the series' chapters.
# @only linux
# @max 40
linux-reader-finding-source = Finding a { -source }

# Button on the Resumed notice.
# @only linux
# @max 20
linux-reader-start-over = Start over

# Button on the Moved notice.
# @only linux
# @max 20
linux-reader-go-back = Go back

# Button on the mode detected notice: opens the reader settings.
# @only linux
# @max 20
linux-reader-change = Change

# Heading in the reader settings over what this series keeps for itself.
# @only linux
# @max 30
linux-reader-this-series = This series

# Heading in the reader settings over what applies to every manga.
# @only linux
# @max 40
linux-reader-all-manga = All manga

# Heading in the reader settings over what applies to every manga on this computer.
# @only linux
# @max 40
linux-reader-all-manga-here = All manga on this computer

# Heading in the reader settings over what applies to every webtoon.
# @only linux
# @max 40
linux-reader-all-strips = All webtoons

# Heading in the reader settings over what applies to every webtoon on this computer.
# @only linux
# @max 40
linux-reader-all-strips-here = All webtoons on this computer

# Heading in the reader settings over what applies to every light novel.
# @only linux
# @max 40
linux-reader-all-novels = All light novels

# The reading mode choice that follows the series' details.
# @only linux
# @max 16
linux-reader-mode-auto = Auto

# Under the reading mode when it was guessed.
# @only linux
# @max 50
linux-reader-mode-detected = Detected from the series details.

# Button under the reading mode when the series has a choice of its own: drops it.
# @only linux
# @max 20
linux-reader-use-default = Use default

# Row that opens the rest of the reader settings.
# @only linux
# @max 30
linux-reader-more-settings = More reader settings

# Line spacing choices.
# @only linux
# @max 12
linux-reader-line-tight = Tight

# @only linux
# @max 12
linux-reader-line-normal = Normal

# @only linux
# @max 12
linux-reader-line-loose = Loose

# The margins choice between Narrow and Wide.
# @only linux
# @max 12
linux-reader-margins-normal = Normal

# Accessible names of the two ends of the text size slider.
# @only linux
# @max 20
linux-reader-text-smaller = Smaller text

# @only linux
# @max 20
linux-reader-text-larger = Larger text

# Title of the small popover that jumps to a page, and its shortcut's name.
# @only linux
# @max 20
linux-reader-go-to-page = Go to page

# Placeholder of the Go to page field.
# @arg total: int
# @only linux
# @max 20
linux-reader-go-to-page-field = 1 to { $total }

# Heading of the reader's keys in the keyboard shortcuts window.
# @only linux
# @max 20
linux-shortcuts-reader = Reader

# Shortcut: turn the page.
# @only linux
# @max 40
linux-shortcuts-turn = Turn the page

# Shortcut: the first or last page.
# @only linux
# @max 40
linux-shortcuts-first-last = First or last page

# Shortcut: a novel's text larger, smaller or back to its size.
# @only linux
# @max 50
linux-shortcuts-text-size = Larger, smaller or default text

# Shortcut: back to the page before a jump, or out of the reader.
# @only linux
# @max 50
linux-shortcuts-reader-back = Back to the page before a jump

# Shortcut: close the reader.
# @only linux
# @max 40
linux-shortcuts-close-reader = Close the reader

# Two names joined, or a list's last name joined to the rest, in the end page's tracker line:
# "AniList and MangaUpdates".
# @arg first: text
# @arg last: text
# @only linux
# @max 60
linux-reader-names-and = { $first } and { $last }

# On the plate that holds a page's place while it loads. The page is its number, such as 23.
# @arg page: int
# @only linux
# @max 20
linux-reader-page-plate = Page { $page }

# Heading when a chapter could not open. The chapter is its number, such as 13.
# @arg chapter: text
# @only linux
# @max 50
linux-reader-open-failed = Couldn't open chapter { $chapter }

# Heading when a chapter with no number could not open.
# @only linux
# @max 50
linux-reader-open-failed-plain = Couldn't open this chapter

# Choice in Send to e-reader's Chapters row: the next unread chapters, as many as the number.
# @arg n: int
# @only linux
# @max 30
linux-send-next = { $n ->
    [one] The next unread chapter
   *[other] The next { $n } unread
}

# Choice in Send to e-reader's Chapters row: every chapter not read yet.
# @only linux
# @max 30
linux-send-all-unread = All unread

# Under Send to e-reader's Chapters row when every chapter is read.
# @only linux
# @max 40
linux-send-none-unread = Every chapter is read

# Toast after the prepared files were saved to a folder.
# @arg n: int
# @only linux
# @max 40
linux-send-saved = { $n ->
    [one] The file is saved
   *[other] { $n } files are saved
}

# Toast when the prepared files could not be written to the folder picked.
# @only linux
# @max 60
linux-send-save-failed = Couldn't save the files in that folder

# Tooltip and spoken name of the button on Updates that looks for new chapters now.
# @only linux
# @max 40
linux-updates-check = Check for new chapters

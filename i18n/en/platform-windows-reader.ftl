## Windows: the reader over the window, its group, its flyouts, the page between chapters and its states. The wording is the Apple reader's.

# Name and tooltip of the Aa button in the reader's group, which opens the reader's settings.
# @only windows
# @max 24
windows-reader-settings-open = Reader settings

# The opening screen's line, under the cover, after the first moment. The chapter label comes from the source.
# @only windows
# @arg chapter: text
# @max 48
windows-reader-opening = Opening { $chapter }

# The opening screen's line after a few seconds of waiting for a source to answer.
# @only windows
# @arg seconds: int
# @max 40
windows-reader-opening-slow = Finding a { -source } · { $seconds } s

# Heading of the error when a chapter would not open.
# @only windows
# @arg chapter: text
# @max 48
windows-reader-chapter-failed = Couldn't open { $chapter }

# Button of the error when a chapter would not open: back to the series page, where the source is chosen.
# @only windows
# @max 24
windows-reader-choose-source = Choose { -source }

# Hint under a page's error: the next page still opens.
# @only windows
# @max 60
windows-reader-page-failed-hint = Turn the page to go on without it.

# Top line of the page between chapters: what just ended. The chapter label comes from the source.
# @only windows
# @arg chapter: text
# @max 48
windows-reader-end-of-chapter = End of { $chapter }

# Line under it when the chapter has not counted as read yet.
# @only windows
# @max 40
windows-reader-end-not-read = Not marked read yet

# Line under the top line once the chapter counted as read and the trackers were updated.
# The trackers are names joined by the app: "AniList and Kitsu".
# @only windows
# @arg trackers: text
# @max 60
windows-reader-end-tracker = Read · { $trackers } updated

# Two tracker names joined, for the line above.
# @only windows
# @arg first: text
# @arg second: text
# @max 40
windows-reader-end-trackers-two = { $first } and { $second }

# Detail line under the next chapter's title on the page between chapters.
# @only windows
# @arg chapter: text
# @arg pages: int
# @max 48
windows-reader-end-next-detail = { $chapter } · { $pages } pages

# Line naming the source the next chapter was downloaded from.
# @only windows
# @arg source: text
# @max 40
windows-reader-end-from = From { $source }

# The one amber button of the page between chapters: opens the next chapter.
# @only windows
# @max 20
windows-reader-continue = { -continue-cap }

# Button on the page between chapters that closes the reader and goes back to the series page.
# @only windows
# @max 28
windows-reader-back-to-series = Back to the series

# End page heading when the next chapter is not out yet.
# @only windows
# @arg chapter: text
# @max 48
windows-reader-end-not-out = { $chapter } isn't out yet

# End page text when the next chapter is not out yet.
# @only windows
# @max 110
windows-reader-end-not-out-body = { -app-name } checks for new chapters in the background and adds them to Updates.

# End page text when the reader has caught up with the series.
# @only windows
# @max 110
windows-reader-end-caught-up-body = New chapters show in Updates when your { -source-pl } have them.

# End page heading at the end of a finished series.
# @only windows
# @max 40
windows-reader-end-finished = End of the series

# End page text at the end of a finished series.
# @only windows
# @max 60
windows-reader-end-finished-body = There are no more chapters.

# End page heading when no source has the next chapter.
# @only windows
# @arg chapter: text
# @max 60
windows-reader-end-no-source = No { $chapter } in your { -source-pl } yet

# End page heading when the next chapter is not downloaded and the PC is offline.
# @only windows
# @arg chapter: text
# @max 48
windows-reader-end-offline = { $chapter } isn't downloaded

# End page text when the next chapter is not downloaded and the PC is offline.
# @only windows
# @max 80
windows-reader-end-offline-body = You're offline. It opens here once you're connected.

# Page between chapters, under the tracker line: the book server the chapter came from has the progress.
# @only windows
# @arg server: text
# @max 60
windows-reader-server-saved = Saved to { $server }

# Same place, while the server has not answered yet: shown after 600 ms, then replaced by the result.
# @only windows
# @arg server: text
# @max 60
windows-reader-server-sending = Sending to { $server }…

# Same place, offline: the progress waits and is sent later.
# @only windows
# @arg server: text
# @max 80
windows-reader-server-offline = Will send to { $server } when you're back online

# Same place, the server did not answer: Kaeri tries again by itself.
# @only windows
# @arg server: text
# @max 80
windows-reader-server-retrying = { $server } didn't answer. { -app-name } will try again.

# Same place, the server refused: the progress waits for Try again. The reason is one of the four below.
# @only windows
# @arg server: text
# @arg reason: text
# @max 110
windows-reader-server-failed = Not saved to { $server }: { $reason }.

# A reason for the line above.
# @only windows
# @max 40
windows-reader-why-key = the server refused the key

# A reason for the line above.
# @only windows
# @max 48
windows-reader-why-password = the server refused the password

# A reason for the line above.
# @only windows
# @max 56
windows-reader-why-gone = this series isn't on the server any more

# A reason for the line above.
# @only windows
# @max 30
windows-reader-why-gave-up = no answer for a day

# The group's line under the series title. The chapter label comes from the source.
# @only windows
# @arg chapter: text
# @arg page: int
# @arg total: int
# @max 60
windows-reader-group-line = { $chapter } · page { $page } of { $total }

# The group's line when a spread shows two pages.
# @only windows
# @arg chapter: text
# @arg first: int
# @arg second: int
# @max 60
windows-reader-group-line-spread = { $chapter } · pages { $first } and { $second }

# The group's line for a strip of images.
# @only windows
# @arg chapter: text
# @arg image: int
# @arg total: int
# @max 60
windows-reader-group-line-strip = { $chapter } · image { $image } of { $total }

# The group's line for a novel in one long scroll.
# @only windows
# @arg chapter: text
# @arg percent: int
# @max 60
windows-reader-group-line-scroll = { $chapter } · { $percent }%

# What a screen reader says when the page turns.
# @only windows
# @arg page: int
# @arg total: int
# @max 40
windows-reader-page-label = Page { $page } of { $total }

# What a screen reader says when a spread turns.
# @only windows
# @arg first: int
# @arg second: int
# @arg total: int
# @max 48
windows-reader-pages-label = Pages { $first } and { $second } of { $total }

# Name of the scrubber, spoken.
# @only windows
# @max 16
windows-reader-scrubber = Page

# Button on the Resumed notice.
# @only windows
# @max 16
windows-reader-start-over = Start over

# Button on the Moved notice.
# @only windows
# @max 16
windows-reader-go-back = Go back

# Button on the reading mode notice, which opens the reader's settings.
# @only windows
# @max 16
windows-reader-change = Change

# Heading of the settings that belong to one series.
# @only windows
# @max 24
windows-reader-this-series = This series

# Heading of the manga settings that stay on this PC.
# @only windows
# @max 30
windows-reader-all-manga-pc = All manga on this PC

# Heading of the webtoon settings that stay on this PC.
# @only windows
# @max 30
windows-reader-all-strips-pc = All webtoons on this PC

# Heading of the novel settings.
# @only windows
# @max 30
windows-reader-all-novels = All light novels

# Row: how the series reads.
# @only windows
# @max 20
windows-reader-reading-mode = Reading mode

# The reading mode's value when it was guessed. The mode is "right to left", "left to right" or "strip".
# @only windows
# @arg mode: text
# @max 30
windows-reader-mode-auto = Auto · { $mode }

# Option of the reading mode picker that lets Kaeri guess again.
# @only windows
# @max 12
windows-reader-mode-auto-option = Auto

# A guessed reading mode, inside "Auto · right to left".
# @only windows
# @max 20
windows-reader-mode-rtl-lower = right to left

# A guessed reading mode, inside "Auto · left to right".
# @only windows
# @max 20
windows-reader-mode-ltr-lower = left to right

# A guessed reading mode, inside "Auto · strip".
# @only windows
# @max 12
windows-reader-mode-strip-lower = strip

# Under the reading mode for a guessed mode.
# @only windows
# @max 60
windows-reader-mode-detected-note = Detected from the series details.

# Under the reading mode for the series' own choice.
# @only windows
# @max 120
windows-reader-mode-choice-note = Your choice is saved for this series and follows it to your other { -device-pl }.

# Button that drops the series' own reading mode.
# @only windows
# @max 20
windows-reader-use-default = Use default

# A novel's reading mode: pages.
# @only windows
# @max 12
windows-reader-mode-paged = Paged

# A novel's reading mode: one long scroll.
# @only windows
# @max 12
windows-reader-mode-scrolled = Scroll

# Row of the manga settings: one page or two.
# @only windows
# @max 20
windows-reader-page-layout = Page layout

# Row of the manga settings: how a page is fitted.
# @only windows
# @max 12
windows-reader-fit = Fit

# A choice for how a page is fitted: to the window's height.
# @only windows
# @max 12
windows-reader-fit-height = Height

# Row of the manga settings.
# @only windows
# @max 24
windows-reader-crop-borders = Crop borders

# Row of the webtoon settings: the space between a strip's images.
# @only windows
# @max 28
windows-reader-gap = Space between images

# Row of the manga settings: what shows round the page.
# @only windows
# @max 16
windows-reader-background = Background

# Row that opens Settings on its Reading pane.
# @only windows
# @max 28
windows-reader-more-settings = More reader settings

# Row: the e-reader-like look.
# @only windows
# @max 20
windows-reader-paper-mode = Paper mode

# Under the Paper mode switch.
# @only windows
# @max 96
windows-reader-paper-mode-note = Paper colours, and pages turn at once with nothing in the reader moving.

# Rows of the novel settings.
# @only windows
# @max 16
windows-reader-font = Font

# @only windows
# @max 16
windows-reader-text-size = Text size

# @only windows
# @max 16
windows-reader-line-spacing = Line spacing

# A choice of line spacing.
# @only windows
# @max 12
windows-reader-line-tight = Tight

# A choice of line spacing, and of margins.
# @only windows
# @max 12
windows-reader-line-normal = Normal

# A choice of line spacing.
# @only windows
# @max 12
windows-reader-line-loose = Loose

# @only windows
# @max 16
windows-reader-margins = Margins

# @only windows
# @max 20
windows-reader-justify = Justify text

# Row of the novel settings: the page's colours.
# @only windows
# @max 12
windows-reader-look = Look

# The text size's value.
# @only windows
# @arg percent: int
# @max 8
windows-reader-percent = { $percent }%

# Button names of the text size row.
# @only windows
# @max 20
windows-reader-smaller = Smaller text

# @only windows
# @max 20
windows-reader-larger = Larger text

# Tabs of the Chapters flyout.
# @only windows
# @max 16
windows-reader-pages-tab = Pages

# @only windows
# @max 16
windows-reader-chapters-tab = Chapters

# A volume's heading in the chapter list.
# @only windows
# @arg volume: text
# @max 24
windows-reader-volume = Volume { $volume }

# A chapter's length in the chapter list.
# @only windows
# @arg pages: int
# @max 20
windows-reader-chapter-pages = { $pages } pages

# Name of a page in the page grid, spoken.
# @only windows
# @arg page: int
# @max 16
windows-reader-grid-page = Page { $page }

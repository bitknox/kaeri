## Apple only: the reader, its chrome, its sheets and its states.

# Button in the reader's control group that closes the reader.
reader-close = Close

# Button in the reader's control group that opens the chapters and the page grid.
reader-chapters = Chapters

# Button in the reader's control group that opens the reader's settings. Aa is the glyph.
reader-settings-open = Reader settings

# Spoken label of a page in the reader.
# @arg page: int
# @arg total: int
# @arg title: text
reader-page-label = Page { $page } of { $total }, { $title }

# The reader's scrubber, spoken.
reader-scrubber = Page

# The scrubber's value, spoken.
# @arg page: int
# @arg total: int
reader-scrubber-value = Page { $page } of { $total }

# Title of the row that picks the look: paper, light, sepia, dark or the system's.
reader-look = Look

# The settings button's label in the control group.
reader-text-label = Aa

# The control group's line under the title. The chapter label comes from the source.
# @arg chapter: text
# @arg page: int
# @arg total: int
reader-group-line = { $chapter } · page { $page } of { $total }

# The control group's line when a spread shows two pages.
# @arg chapter: text
# @arg first: int
# @arg second: int
reader-group-line-spread = { $chapter } · pages { $first } and { $second }

# The control group's line for a strip.
# @arg chapter: text
# @arg image: int
# @arg total: int
reader-group-line-strip = { $chapter } · image { $image } of { $total }

# The control group's line for a novel, paged.
# @arg chapter: text
# @arg page: int
# @arg total: int
reader-group-line-novel = { $chapter } · page { $page } of { $total }

# The control group's line for a novel in one long scroll.
# @arg chapter: text
# @arg percent: int
reader-group-line-scroll = { $chapter } · { $percent }%

# Line in the margin of a novel's page.
# @arg count: int
reader-pages-left = { $count ->
    [one] One page left in this chapter
   *[other] { $count } pages left in this chapter
}

# Link in the group that opens Go to page, and the field's label.
reader-go-to-page = Go to page

# Placeholder of the Go to page field.
# @arg total: int
reader-go-to-page-field = 1 to { $total }

# Label of the end page: what just ended.
# @arg chapter: text
reader-end-of-chapter = End of { $chapter }

# Line on the end page when the chapter counted as read.
reader-end-read = Read

# Line on the end page for a chapter that has not counted yet.
reader-end-not-read = Not marked read yet

# Small label above the next chapter's title on the end page.
reader-end-next = Next

# Detail line under the next chapter's title. The chapter label is the source's own words.
# @arg chapter: text
# @arg pages: int
reader-end-next-detail = { $chapter } · { $pages } pages

# The one amber button of the end page: opens the next chapter.
reader-continue = { -continue-cap }

# Button on the end page that goes back to the series page.
reader-back-to-series = Back to the series

# Hint under Continue on the end page.
reader-swipe-to-continue = Swipe to keep reading

# End page heading when the next chapter is not out yet.
# @arg chapter: text
reader-end-not-out = { $chapter } isn't out yet

# End page text when the next chapter is not out yet.
reader-end-not-out-body = { -app-name } checks for new chapters in the background and adds them to Updates.

# End page heading when the reader has caught up with the series.
reader-end-caught-up = You're caught up

# End page text when the reader has caught up with the series.
reader-end-caught-up-body = New chapters show in Updates when your { -source-pl } have them.

# End page heading at the end of a finished series.
reader-end-finished = End of the series

# End page text at the end of a finished series.
reader-end-finished-body = There are no more chapters.

# End page heading when no source has the next chapter.
# @arg chapter: text
reader-end-no-source = No { $chapter } in your { -source-pl } yet

# End page heading when the next chapter is not downloaded and the device is offline.
# @arg chapter: text
reader-end-offline = { $chapter } isn't downloaded

# End page text when the next chapter is not downloaded and the device is offline.
reader-end-offline-body = You're offline. It opens here once you're connected.

# Line that opens a chapter, over the cover.
# @arg chapter: text
reader-opening = Opening { $chapter }

# The opening line after a few seconds of waiting.
# @arg seconds: int
reader-opening-slow = Finding a { -source } · { $seconds } s

# Plate with the page's number while a page loads.
# @arg page: int
reader-page-loading = Loading page { $page }

# Heading of the error in a page's place.
# @arg page: int
reader-page-failed = Couldn't load page { $page }

# Button in the error in a page's place.
reader-try-again = Try again

# Hint under a page's error.
reader-page-failed-hint = Swipe to go on without it.

# Heading of the error when a chapter would not open.
# @arg chapter: text
reader-chapter-failed = Couldn't open { $chapter }

# Button of the error when a chapter would not open.
reader-choose-source = Choose { -source }

# Heading of the error when a chapter has no pages.
reader-nothing-to-read = Nothing to read here

# Notice over the page when the person's Focus turned Paper mode on.
reader-focus-paper = Paper mode is on while your Focus is.

# Button on the Resumed notice.
reader-start-over = Start over

# Button on the Moved notice.
reader-go-back = Go back

# Button on the mode detected notice.
reader-change = Change

# Heading of the settings that belong to one series.
reader-this-series = This series

# Heading of the manga settings.
reader-all-manga = All manga

# Heading of the manga settings on a device: it names the device kind.
reader-all-manga-here = All manga on this { -device }

# Heading of the novel settings.
reader-all-novels = All light novels

# Heading of the web comic settings.
reader-all-strips = All webtoons

# Row: how the series reads.
reader-reading-mode = Reading mode

# Reading mode value when it was guessed.
# @arg mode: text
reader-mode-auto = Auto · { $mode }

# Reading mode names.
reader-mode-rtl = Right to left
reader-mode-ltr = Left to right
reader-mode-strip = Strip
reader-mode-paged = Paged
reader-mode-scrolled = Scroll

# Under the reading mode picker for a guessed mode.
reader-mode-detected-note = Detected from the series details.

# Under the reading mode picker for a choice.
reader-mode-choice-note = Your choice is saved for this series and follows it to your other { -device-pl }.

# Option of the reading mode picker that lets Kaeri guess again.
reader-mode-auto-option = Auto

# Detail of the Auto option.
reader-mode-auto-detail = { -app-name } picks from the series details

# Detail of the Strip option.
reader-mode-strip-detail = For webtoons and long images

# Line under a series' own choice.
# @arg title: text
reader-mode-override-note = Set for { $title } on all your { -device-pl }.

# Button that drops the series' own choice.
reader-use-default = Use default

# Rows of the manga sheet.
reader-fit = Fit
reader-fit-screen = Screen
reader-fit-width = Width
reader-crop-borders = Crop borders
reader-background = Background
reader-background-theme = Theme
reader-background-black = Black
reader-background-white = White
reader-paper-mode = Paper mode
reader-more-settings = More reader settings
reader-search-settings = Search reader settings

# Rows of the novel sheet.
reader-font = Font
reader-font-serif = Serif
reader-font-sans = Sans
reader-line-spacing = Line spacing
reader-line-tight = Tight
reader-line-normal = Normal
reader-line-loose = Loose
reader-margins = Margins
reader-margins-narrow = Narrow
reader-margins-normal = Normal
reader-margins-wide = Wide
reader-pages = Pages
reader-text-size = Text size
reader-justify = Justify text

# The five looks.
reader-look-paper = Paper
reader-look-light = Light
reader-look-sepia = Sepia
reader-look-dark = Dark
reader-look-system = System

# Rows of More reader settings.
reader-page-layout = Page layout
reader-layout-auto = Auto
reader-layout-single = One
reader-layout-double = Two
reader-spreads = Wide pages
reader-spreads-split = Split
reader-spreads-rotate = Turn
reader-spreads-whole = Whole
reader-tap-zones = Tap zones
reader-tap-edges = Edges
reader-tap-sides = Halves
reader-tap-off = Off
reader-volume-keys = Volume buttons turn pages
reader-strip-width = Width
reader-greyscale = Greyscale pages
reader-quiet = Quiet the phone while reading
reader-foot = Show pages left
reader-folio = Show page numbers
reader-group-layout = Layout
reader-group-images = Images
reader-group-navigation = Navigation
reader-group-text = Text
reader-group-paper = Paper mode
reader-on = On
reader-off = Off

# Paper mode's page.
reader-paper-mode-note = Pages turn at once and nothing in the reader moves.
reader-paper-quiet-note = iPhone quiets notifications with a Focus. Set one to turn on when { -app-name } opens, and { -app-name } switches to Paper mode while it is on.
reader-paper-quiet-value = With a Focus

# Title of the Focus filter in the system's Focus settings.
reader-focus-filter-title = Reading

# Description of the Focus filter in the system's Focus settings.
reader-focus-filter-description = Turns on Paper mode in the reader while this Focus is on.

# Parameter of the Focus filter.
reader-focus-filter-paper = Paper mode

# Heading in the chapters sheet.
reader-pages-tab = Pages
reader-chapters-tab = Chapters

# A volume's heading in the chapter list.
# @arg volume: text
reader-volume = Volume { $volume }

# Chapters sheet: a chapter's state line.
# @arg page: int
# @arg total: int
reader-chapter-in-progress = Page { $page } of { $total }
reader-chapter-read = Read
reader-chapter-new = new

# Chapters sheet: a chapter's length.
# @arg pages: int
reader-chapter-pages = { $pages } pages

# Chapters sheet: the offline line.
reader-offline-line = Offline. Downloaded chapters still open.

# Chapters sheet: the list is on its way.
reader-chapters-loading = Loading chapters

# Mac menu: the Go menu and its items.
reader-menu-go = Go
reader-menu-next-page = Next Page
reader-menu-previous-page = Previous Page
reader-menu-next-chapter = Next Chapter
reader-menu-previous-chapter = Previous Chapter
reader-menu-go-to-page = Go to Page…
reader-menu-chapters = Chapters
reader-menu-settings = Reader Settings…
reader-menu-zoom-in = Zoom In
reader-menu-zoom-out = Zoom Out
reader-menu-zoom-fit = Fit to Window

# Mac menu item: back to where the person was before a jump.
# @arg page: int
reader-menu-back-to-page = Back to Page { $page }

# Mac menu item when there is no place to go back to.
reader-menu-back = Back to Previous Page

# Title of the Mac reader window.
reader-window = Reader

# Keyboard step row on the Mac.
reader-key-step = Keyboard step
reader-key-step-panel = Panel
reader-key-step-screen = Screen

# Gap between a strip's images.
reader-strip-gap = Gap between images
reader-gap-none = None
reader-gap-small = Small

# Accessibility value of the pointer hint on the first chapter's tap zones.
reader-zone-back = Back
reader-zone-forward = Forward
reader-zone-menu = Menu

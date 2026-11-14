# Reading in the terminal past Home and Search: a series' page with its chapters, and the reader
# it opens. The terminal draws them as text.

## Series page

# Under a series' title on the terminal's series page: the source its chapters are read from.
# @arg source: text
# @only tui
tui-read-series-from = Reading from { $source }

# After the source on the series page: the list the series is on and the trackers that keep that
# list, such as "Reading on AniList". The list is its name, such as "Reading" or "Planning"; the
# trackers are already joined.
# @arg list: text
# @arg trackers: text
# @only tui
tui-read-series-list-on = { $list } on { $trackers }

# After the source on the series page, when no tracker keeps the list: "On your Reading list".
# @arg list: text
# @only tui
tui-read-series-list = On your { $list } list

# Beside the progress meter on the series page: how many of the chapters listed are read.
# @arg read: int
# @arg total: int
# @only tui
tui-read-series-read = { $total ->
    [one] { $read } of 1 chapter read
   *[other] { $read } of { $total } chapters read
 }

# At the top of the series page while the computer is offline: the chapters listed are the ones
# kept from before, and only the downloaded ones open.
# @only tui
tui-read-series-offline = Offline. Downloaded chapters still open.

# Left of the status line on the series page while the computer is offline.
# @max 24
# @only tui
tui-read-status-offline = Reading · offline

# Header chip on the series page while the computer is offline, in place of how many sources are
# on. The mark is a cross. Lower case, as the header's other chips are.
# @arg mark: text
# @only tui
tui-read-header-offline = { -source-pl } { $mark }

## Status line and keys

# Hint in the status line on the series page: the key that marks the chapter under the cursor
# read, or unread when it is read. Shown in lower case. Also a row of the key reference.
# @max 20
# @only tui
tui-read-read-unread = Read or unread

# Hint in the status line on the series page: the key that picks the list the series is on.
# Shown in lower case.
# @max 12
# @only tui
tui-read-hint-status = Status

# Hint in the status line on reading's Home, on a series in Continue: the key that opens the
# chapter where reading stopped. Shown in lower case.
# @max 12
# @only tui
tui-read-hint-continue = { -continue-reading-cap }

# Heading of the series page's keys in the key reference.
# @only tui
tui-read-keys-series = Series

# A row of the key reference: open the chapter under the cursor on the series page.
# @only tui
tui-read-keys-read = Read the chapter

# A row of the key reference: download the chapter under the cursor on the series page.
# @only tui
tui-read-keys-download = Download this chapter

# A row of the key reference: pick the list the series is on, such as Reading or Planning.
# @only tui
tui-read-keys-status = Set the list status

# A row of the key reference on reading's Home: open a series' page with its chapters, from a
# series in Continue, where Enter opens the chapter itself.
# @only tui
tui-read-keys-chapters = Open the series page

## Reader

# Left of the status line while a chapter opens in the terminal's reader. The chapter is its
# number, such as 14.
# @arg chapter: text
# @max 30
# @only tui
tui-reader-opening = Opening chapter { $chapter }

# The reader's opening line while no source has been picked for the series yet.
# @only tui
tui-reader-finding = Finding a { -source }

# Left of the reader's status line: the page in view and how many the chapter has.
# @arg page: plain
# @arg pages: plain
# @max 30
# @only tui
tui-reader-page-of = Page { $page } of { $pages }

# Left of the reader's status line while two pages show side by side.
# @arg first: plain
# @arg second: plain
# @arg pages: plain
# @max 36
# @only tui
tui-reader-pages-of = Pages { $first } and { $second } of { $pages }

# After the page count in the reader's status line, when the terminal draws pages as coloured
# blocks because it has no image protocol.
# @max 12
# @only tui
tui-reader-blocks = Blocks

# Said once a run on the reader's status rule, the first time a chapter opens in a terminal with
# no image protocol: the pages show as coloured blocks.
# @only tui
tui-reader-blocks-notice = No image protocol here. Pages show as blocks.

# Hint after "Resumed at page 8" on the reader's status rule: the key that starts the chapter
# from its first page. Shown in lower case.
# @max 16
# @only tui
tui-reader-start-over = Start over

# Hint after the notice that the chapter moved to where another device read to: the key that
# goes back to the page this computer was on. Shown in lower case.
# @max 16
# @only tui
tui-reader-go-back = Go back

# Hint after the notice that the reader guessed how this series reads: the key that opens the
# reader settings to change it. Shown in lower case.
# @max 16
# @only tui
tui-reader-change = Change

# Middle of the reader when the terminal was started with images turned off, and a chapter of
# manga or webtoon pages opens. The option is typed as it is on the command line.
# @only tui
tui-reader-images-off = Pages are pictures, and { -app-name } started with --no-images.

# Under that line.
# @only tui
tui-reader-images-off-how = Start it without that option to read manga and webtoons here.

# Under that line.
# @only tui
tui-reader-images-off-novels = Light novels open as text either way.

# Heading in the reader when a chapter couldn't open. The chapter is its number, such as 16.
# @arg chapter: text
# @max 40
# @only tui
tui-reader-chapter-failed = Couldn't open chapter { $chapter }

# Top line of the page after the last chapter of a finished series.
# @max 40
# @only tui
tui-reader-end-series = End of the series

# Under that line.
# @only tui
tui-reader-end-no-more = There are no more chapters.

# Under "You're caught up" on the page after the last chapter there is.
# @only tui
tui-reader-end-caught-up-line = New chapters show on Home when your { -source-pl } have them.

# On the page after the last chapter there is, when the series goes on. The chapter is the next
# one's number, such as 18.
# @arg chapter: text
# @max 40
# @only tui
tui-reader-end-not-out = Chapter { $chapter } isn't out yet

# The same, when the next chapter's number isn't known.
# @max 40
# @only tui
tui-reader-end-not-out-next = The next chapter isn't out yet

# Under either of those lines.
# @only tui
tui-reader-end-not-out-line = { -app-name } checks for new chapters in the background. New ones show on Home.

# On the page between chapters when no source has the next chapter. The chapter is its number,
# such as 18.
# @arg chapter: text
# @only tui
tui-reader-end-no-source = No chapter { $chapter } in your { -source-pl } yet

# The same, when the next chapter's number isn't known.
# @only tui
tui-reader-end-no-source-next = No next chapter in your { -source-pl } yet

# On the page between chapters while offline, and as a toast when a chapter that isn't on this
# computer is opened offline. The chapter is its number, such as 16.
# @arg chapter: text
# @max 40
# @only tui
tui-reader-end-offline = Chapter { $chapter } isn't downloaded

# The same, when the chapter's number isn't known.
# @max 48
# @only tui
tui-reader-end-offline-next = The next chapter isn't downloaded

# Under that line on the page between chapters.
# @only tui
tui-reader-end-offline-line = You're offline. It opens here once you're connected.

# Under the next chapter on the page between chapters when it is an extra, such as 16.5 between
# 16 and 17: skipping to the next numbered chapter leaves the extra unread. The chapter is the
# extra's number.
# @arg chapter: text
# @only tui
tui-reader-end-skip-unread = Chapter { $chapter } stays unread if skipped.

# Under the next chapter's first page on the page between chapters. The chapter is its number.
# @arg chapter: text
# @max 40
# @only tui
tui-reader-end-first-page = First page of chapter { $chapter }

# The reader's heading when the terminal is too small to show a page.
# @max 30
# @only tui
tui-reader-too-small = Needs a larger terminal

# Under that heading: the smallest size the reader shows pages at, in columns and rows.
# @arg columns: plain
# @arg rows: plain
# @max 30
# @only tui
tui-reader-too-small-line = At least { $columns } by { $rows }

## Reader keys

# Hint in the reader's status line: the key that turns to the next page. Shown in lower case.
# @max 12
# @only tui
tui-reader-hint-next = Next

# Hint in the reader's status line: the key that turns back a page. Shown in lower case.
# @max 12
# @only tui
tui-reader-hint-back = Back

# Hint in the reader's status line while a chapter opens: the key that stops it and goes back.
# Shown in lower case.
# @max 12
# @only tui
tui-reader-hint-stop = Stop

# Hint in the reader's status line: the key that opens the reader settings. Shown in lower case.
# @max 12
# @only tui
tui-reader-hint-settings = Settings

# Hint in the reader's status line on a page that couldn't load: the turn key goes on to the
# next page without it. Shown in lower case.
# @max 24
# @only tui
tui-reader-hint-without = Go on without it

# Hint in the reader's status line on the page between chapters. Shown in lower case.
# @max 16
# @only tui
tui-reader-hint-next-chapter = Next chapter

# Hint in the reader's status line: the key that leaves the reader for the series page it was
# opened from. Shown in lower case.
# @max 24
# @only tui
tui-reader-hint-series = Back to the series

# Hint in the reader's status line after the last chapter of a finished series: the key that
# opens its first chapter. Shown in lower case.
# @max 36
# @only tui
tui-reader-hint-again = Read again from the start

# Heading of the reader's keys in the key reference.
# @only tui
tui-reader-keys-reader = Reader

# Heading of the keys that change how the reader shows pages, in the key reference.
# @only tui
tui-reader-keys-view = Reader view

# Beside the reader's keys in the key reference.
# @only tui
tui-reader-keys-rtl = In right-to-left manga, ← turns forward and → turns back, as the page moves. Space always turns forward.

# Beside the reader's keys in the key reference, under that.
# @only tui
tui-reader-keys-left-out = Keys that do nothing in the open chapter are left out: no zoom in a novel, no width in manga.

# Ends the toast that says a command does nothing on this screen, as "on the Downloads screen"
# does: the command works in the reader.
# @only tui
tui-reader-where = in the reader

# A row of the key reference and the command palette: turn to the next page.
# @only tui
tui-reader-action-next = Next page

# A row of the key reference and the command palette: turn back a page.
# @only tui
tui-reader-action-previous = Previous page

# A row of the key reference: turn to the page on the left, which is the next one in
# right-to-left manga.
# @only tui
tui-reader-action-left = Page to the left

# A row of the key reference: turn to the page on the right.
# @only tui
tui-reader-action-right = Page to the right

# A row of the key reference: go to the chapter's first page.
# @only tui
tui-reader-action-first = First page

# A row of the key reference: go to the chapter's last page.
# @only tui
tui-reader-action-last = Last page

# A row of the key reference: open the next chapter.
# @only tui
tui-reader-action-next-chapter = Next chapter

# A row of the key reference: open the chapter before this one.
# @only tui
tui-reader-action-previous-chapter = Previous chapter

# A row of the key reference: go back to the page read before a jump.
# @only tui
tui-reader-action-back-to-page = Back to page

# A row of the key reference: hide the header and status line for a bare page, or show them.
# @only tui
tui-reader-action-bare = Bare page

# A row of the key reference: show one page or two side by side.
# @only tui
tui-reader-action-pages = One or two pages

# A row of the key reference: open the reader settings.
# @only tui
tui-reader-action-settings = Reader settings

# A row of the key reference: leave the reader for the screen it was opened from.
# @only tui
tui-reader-action-leave = Leave the reader

# A row of the key reference: on the page between chapters, open the next numbered chapter past
# the extras.
# @only tui
tui-reader-action-skip = Skip the extras

## Fit width, zoom and strips

# A row of the key reference: scroll a page shown at fit width, a zoomed page or a strip down.
# @only tui
tui-reader-action-scroll-down = Scroll down

# A row of the key reference: scroll up.
# @only tui
tui-reader-action-scroll-up = Scroll up

# A row of the key reference: read the series as pages that turn, or as one long strip.
# @only tui
tui-reader-action-mode = Paged or strip

# A row of the key reference: fit the page to the window, or to its width and scroll it.
# @only tui
tui-reader-action-fit = Fit screen or width

# A row of the key reference: zoom in on the page, or leave the zoom.
# @only tui
tui-reader-action-zoom = Zoom

# A row of the key reference: zoom in a step.
# @only tui
tui-reader-action-zoom-in = Zoom in

# A row of the key reference: zoom out a step.
# @only tui
tui-reader-action-zoom-out = Zoom out

# Hint in the reader's status line: the keys that scroll the page or the strip. Shown in lower
# case.
# @max 12
# @only tui
tui-reader-hint-scroll = Scroll

# Hint in the reader's status line at fit width: the key that fits the page to the window
# again. Shown in lower case.
# @max 16
# @only tui
tui-reader-hint-fit-screen = Fit screen

# Hint in the reader's status line while zoomed: the keys that move the view about the page.
# Shown in lower case.
# @max 12
# @only tui
tui-reader-hint-move = Move

# Hint in the reader's status line while zoomed: the keys that zoom in and out. Shown in lower
# case.
# @max 12
# @only tui
tui-reader-hint-zoom = Zoom

# Hint in the reader's status line while zoomed: the key that leaves the zoom. Shown in lower
# case.
# @max 16
# @only tui
tui-reader-hint-leave-zoom = Leave zoom

# Hint in the reader's status line in a strip: the key that scrolls on by most of a screen.
# Shown in lower case.
# @max 16
# @only tui
tui-reader-hint-next-screen = Next screen

# Hint in the reader's status line in a wide window with two pages side by side: the key that
# shows one page at a time. Shown in lower case.
# @max 16
# @only tui
tui-reader-hint-one-page = One page

# Hint in the reader's status line in a wide window with one page: the key that shows two side
# by side. Shown in lower case.
# @max 16
# @only tui
tui-reader-hint-two-pages = Two pages

# After the page count in the reader's status line while the page fits the window's width and
# scrolls.
# @max 16
# @only tui
tui-reader-fit-width = Fit width

# Left of the reader's status line while zoomed in, before the page count: "Zoom 300%".
# @arg percent: plain
# @max 16
# @only tui
tui-reader-zoom = Zoom { $percent }%

# Under the small map of the page at the right while zoomed in: which page it is.
# @arg page: plain
# @max 12
# @only tui
tui-reader-zoom-page = Page { $page }

# Left of the reader's status line in a webtoon strip: the chapter, and the image at the top of
# the window out of the chapter's images. The chapter is its number, such as 15.
# @arg chapter: text
# @arg image: plain
# @arg images: plain
# @max 48
# @only tui
tui-reader-strip-at = Chapter { $chapter } · image { $image } of { $images }

# On the reader's status rule in a strip, once the chapter it ran on from counted as read. The
# chapter is its number, such as 14.
# @arg chapter: text
# @max 60
# @only tui
tui-reader-strip-read = Chapter { $chapter } counted as read

# On the reader's status rule in a strip, once the chapter counted as read and the trackers
# were updated. The trackers are already joined, such as "AniList".
# @arg chapter: text
# @arg trackers: text
# @max 80
# @only tui
tui-reader-strip-read-tracker = Chapter { $chapter } counted as read · { $trackers } updated

# On the reader's status rule in a strip, once the chapter counted as read while the computer
# is offline: the trackers are told once it is back online.
# @arg chapter: text
# @arg trackers: text
# @max 90
# @only tui
tui-reader-strip-read-offline = Chapter { $chapter } counted as read · { $trackers } updates when you're back online

## Light novels

# A row of the key reference: make a light novel's text column narrower.
# @only tui
tui-reader-action-narrower = Text narrower

# A row of the key reference: make a light novel's text column wider.
# @only tui
tui-reader-action-wider = Text wider

# A row of the key reference in a light novel: switch between turning screens and scrolling
# lines.
# @only tui
tui-reader-action-mode-novel = Paged or scrolled

# Hint in the reader's status line in a light novel: the keys that change the text's width.
# Shown in lower case.
# @max 12
# @only tui
tui-reader-hint-width = Width

# Hint in the settings over the reader: the keys that change the setting in the row. Shown in
# lower case.
# @max 12
# @only tui
tui-reader-hint-change = Change

# A light novel's picture, on a line of its own in the text: the picture's description.
# @arg alt: text
# @max 200
# @only tui
tui-reader-novel-picture = Picture: { $alt }

# A light novel's picture with no description, on a line of its own in the text.
# @max 20
# @only tui
tui-reader-novel-picture-plain = Picture

# In the reader's status line in a light novel, after the part's name: how far through the part
# the text in view reaches, such as "34%".
# @arg percent: plain
# @max 8
# @only tui
tui-reader-novel-through = { $percent }%

## Reader settings

# Heading in the reader settings over what this series keeps for itself.
# @max 40
# @only tui
tui-reader-settings-series = This series

# Heading in the reader settings over what applies to every light novel on this computer.
# @max 40
# @only tui
tui-reader-settings-novels-here = Light novels on this computer

# Heading in the reader settings over what applies to every light novel, on every device.
# @max 40
# @only tui
tui-reader-settings-novels-all = All light novels

# Heading in the reader settings over what applies to every manga and webtoon on this computer.
# @max 40
# @only tui
tui-reader-settings-manga-here = Manga on this computer

# Heading in the reader settings over what applies to every manga and webtoon, on every device.
# @max 40
# @only tui
tui-reader-settings-manga-all = All manga

# Heading in the reader settings over the settings the apps have and the terminal leaves to
# itself.
# @max 40
# @only tui
tui-reader-settings-not-here = Not in the terminal

# Under that heading in a light novel's reader settings.
# @only tui
tui-reader-settings-novel-note = Font, text size and look follow the terminal's own settings. Margins come from the text width.

# Under that heading in a manga's reader settings.
# @only tui
tui-reader-settings-manga-note = Look, colour and background follow the terminal's own settings. Tap zones are for touch screens.

# Heading over the sample of the text at the width being set, beside a light novel's reader
# settings.
# @max 20
# @only tui
tui-reader-settings-preview = Preview

# At the foot of a light novel's reader settings.
# @max 70
# @only tui
tui-reader-settings-novel-foot = Changes apply at once. The place in the text stays.

# At the foot of a manga's reader settings.
# @max 70
# @only tui
tui-reader-settings-manga-foot = Changes apply at once. The page in view stays.

# A row of the reader settings: paged or scrolled, or paged or a strip.
# @max 20
# @only tui
tui-reader-setting-mode = Mode

# A row of the reader settings: right to left or left to right.
# @max 20
# @only tui
tui-reader-setting-direction = Direction

# A row of the reader settings: a page fitted to the screen or to its width.
# @max 20
# @only tui
tui-reader-setting-fit = Fit

# A row of the reader settings: one page at a time, two, or as the window allows.
# @max 20
# @only tui
tui-reader-setting-page-layout = Page layout

# A row of the reader settings: what a page printed across two pages does on a one-page screen.
# @max 20
# @only tui
tui-reader-setting-spreads = Spreads

# A row of the reader settings: white borders cut from the pages.
# @max 20
# @only tui
tui-reader-setting-crop = Crop borders

# A row of the reader settings: how wide a webtoon strip runs.
# @max 20
# @only tui
tui-reader-setting-strip-width = Strip width

# A row of the reader settings: how many columns a light novel's lines run to.
# @max 20
# @only tui
tui-reader-setting-text-width = Text width

# A row of the reader settings: a blank line between paragraphs or none.
# @max 20
# @only tui
tui-reader-setting-line-spacing = Line spacing

# A row of the reader settings: lines spaced out to end together at the text width.
# @max 20
# @only tui
tui-reader-setting-justify = Justify

# A reading mode: a screen of text or pages at a time.
# @max 12
# @only tui
tui-reader-value-paged = Paged

# A reading mode for light novels: the text scrolls a line at a time.
# @max 12
# @only tui
tui-reader-value-scrolled = Scrolled

# A reading mode for manga: whole pages one under another.
# @max 12
# @only tui
tui-reader-value-vertical = Vertical

# A line spacing in the terminal: a blank line between paragraphs.
# @max 12
# @only tui
tui-reader-value-open = Open

# A line spacing in the terminal: no line between paragraphs.
# @max 12
# @only tui
tui-reader-value-tight = Tight

# A width in the reader settings, in the terminal's columns, such as "66 columns".
# @arg columns: int
# @max 20
# @only tui
tui-reader-value-columns = { $columns ->
    [one] 1 column
   *[other] { $columns } columns
}

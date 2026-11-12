# Under a page that is still loading after four seconds.
# @arg page: plain
# @max 40
reader-page-loading = Loading page { $page }

# On the plate of a page that is still loading.
# @arg page: plain
# @max 20
reader-page-plate = Page { $page }

# On the plate of a strip image that is still loading.
# @arg image: plain
# @max 20
reader-image-plate = Image { $image }

# Under a strip image's number after four seconds.
# @arg image: plain
# @max 30
reader-image-loading = Loading image { $image }

# Under a strip image's number (Image 4) when it couldn't be loaded, above Try again.
# @max 30
reader-image-failed = Couldn't load

# Heading where a page could not be loaded.
# @arg page: plain
# @max 40
reader-page-failed = Couldn't load page { $page }

# Button that loads a failed page again.
# @max 20
reader-try-again = Try again

# Closes the reader and goes back to the series.
# @max 16
reader-close = Close

# Under the top line of the page between chapters, once the chapter counts as read.
# @max 40
reader-end-read = Counted as read

# Under the end of chapter line, naming the trackers the read went to.
# @arg trackers: text
# @max 80
reader-end-tracker = Counted as read · { $trackers } updated

# Under the end of chapter line when the device is offline: the trackers hear of the read once
# it is back online.
# @arg tracker: text
# @max 90
reader-end-tracker-offline = Counted as read · { $tracker } updates when you're back online

# Small heading above the next chapter on the page between chapters.
# @max 16
reader-end-next = Next

# A reading mode, and a reading direction.
# @max 20
reader-mode-rtl = Right to left

# A reading mode, and a reading direction.
# @max 20
reader-mode-ltr = Left to right

# A reading mode: one long scroll of images.
# @max 12
reader-mode-strip = Strip

# A choice for how a page is fitted.
# @max 12
reader-fit-screen = Screen

# A choice for how a page is fitted.
# @max 12
reader-fit-width = Width

# A choice of typeface.
# @max 12
reader-font-serif = Serif

# A choice of typeface.
# @max 12
reader-font-sans = Sans

# A look for reading: warm paper.
# @max 12
reader-look-paper = Paper

# A look for reading: a light page.
# @max 12
reader-look-light = Light

# A look for reading: a brown page.
# @max 12
reader-look-sepia = Sepia

# A look for reading: a dark page.
# @max 12
reader-look-dark = Dark

# A choice of margins.
# @max 12
reader-margins-narrow = Narrow

# A choice of margins.
# @max 12
reader-margins-wide = Wide

# A choice for page layout.
# @max 12
reader-layout-auto = Auto

# A choice for page layout.
# @max 12
reader-layout-single = One

# A choice for page layout.
# @max 12
reader-layout-double = Two

# A choice for wide pages: cut in two.
# @max 12
reader-spreads-split = Split

# A choice for wide pages: turned to fill the screen.
# @max 12
reader-spreads-rotate = Turn

# A choice for wide pages: shown whole.
# @max 12
reader-spreads-whole = Whole

# A choice for the reader's tap zones: taps near the left and right edges turn the page.
# Replaced by Sides; kept until every app has the new tap zones page.
# @max 12
reader-tap-edges = Edges

# Tap zone layout: Sides when pages turn sideways, L shape when they move downwards. The name
# under its tile.
# @max 20
reader-tap-auto = Auto

# Tap zone layout: previous page on one side, next page on the other, controls in the middle.
# @max 20
reader-tap-sides = Sides

# Tap zone layout: previous page along the top and down one side, next page along the bottom and
# down the other side, in the shape of the letter L.
# @max 20
reader-tap-l-shape = L shape

# Tap zone layout: controls along the top, a narrow column for previous page, and the rest of the
# page for next page.
# @max 20
reader-tap-wide-next = Wide next

# Tap zone layout: previous page across the top, next page across the bottom.
# @max 20
reader-tap-top-bottom = Top and bottom

# Tap zone layout: next page down both the left and the right side, previous page in the bottom
# middle.
# @max 26
reader-tap-both-next = Next on both sides

# Tap zone layout: taps only show and hide the controls, and never turn the page.
# @max 20
reader-tap-off = Off

# Tap zones page: the choice for paged reading (pages that turn), one of two at the top.
# @max 16
reader-tap-paged = Paged

# Tap zones page: the choice for strips and scrolled novels, one of two at the top.
# @max 16
reader-tap-scrolling = Scrolling

# Tap zones page: row that sets how wide the zones at the edges are.
# @max 24
reader-tap-size = Zone size

# Choice of Zone size: the edge zones are a third of the screen.
# @max 12
reader-tap-standard = Standard

# Choice of Zone size: the edge zones are a quarter of the screen.
# @max 12
reader-tap-small = Small

# Tap zones page: switch that swaps the left and right zones, for the left hand.
# @max 28
reader-tap-flip-sides = Flip left and right

# Tap zones page: switch that swaps the top and bottom zones.
# @max 28
reader-tap-flip-ends = Flip top and bottom

# Tap zones page: button that shows the zones over the page.
# @max 20
reader-tap-show = Show zones

# Tap zones page, beside Show zones: the setting is not synced. The kind is the device's own word,
# as in sync-turn-off: “Mac”, “iPhone”, “phone”.
# @arg kind: text
# @max 32
reader-tap-here = Stays on this { $kind }

# The reader's settings on a Mac, Windows or Linux: the row that opens the tap zones page, which
# is about clicks there.
# @max 20
reader-click-zones = Click zones

# Name of the tap zone that turns to the next page, in the guide over the page and as a screen
# reader action.
# @max 24
reader-zone-next = Next page

# Name of the tap zone that turns to the previous page, in the guide and as a screen reader action.
# @max 24
reader-zone-previous = Previous page

# Name of the tap zone that shows and hides the reader's controls, in the guide.
# @max 24
reader-zone-controls = Controls

# Screen reader action on the page that shows the reader's controls.
# @max 24
reader-zone-show-controls = Show controls

# Name of the tap zone that scrolls a strip or a scrolled novel down, in the guide.
# @max 24
reader-zone-scroll-down = Scroll down

# Name of the tap zone that scrolls a strip or a scrolled novel up, in the guide.
# @max 24
reader-zone-scroll-up = Scroll up

# Under Controls in the tap zone guide: where the zones are changed.
# @max 40
reader-zone-change = Change in Reader settings

# Tap zone layout Off in words, the accessibility value of its tile.
reader-tap-off-about = Taps show and hide the controls

# Tap zone layout in words, the accessibility value of its tile: Sides, next page on the right.
reader-tap-sides-right-about = Next page on the right side, previous page on the left side, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: Sides, next page on the left.
reader-tap-sides-left-about = Next page on the left side, previous page on the right side, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: L shape, next page low and on the right.
reader-tap-l-shape-bottom-right-about = Next page along the bottom and on the right, previous page along the top and on the left, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: L shape, next page low and on the left.
reader-tap-l-shape-bottom-left-about = Next page along the bottom and on the left, previous page along the top and on the right, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: L shape flipped top and bottom, next page high and on the right.
reader-tap-l-shape-top-right-about = Next page along the top and on the right, previous page along the bottom and on the left, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: L shape flipped top and bottom, next page high and on the left.
reader-tap-l-shape-top-left-about = Next page along the top and on the left, previous page along the bottom and on the right, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: Wide next, controls on top, previous page on the left.
reader-tap-wide-next-top-left-about = Controls along the top, previous page in a narrow column on the left, next page everywhere else

# Tap zone layout in words, the accessibility value of its tile: Wide next, controls on top, previous page on the right.
reader-tap-wide-next-top-right-about = Controls along the top, previous page in a narrow column on the right, next page everywhere else

# Tap zone layout in words, the accessibility value of its tile: Wide next flipped top and bottom, previous page on the left.
reader-tap-wide-next-bottom-left-about = Controls along the bottom, previous page in a narrow column on the left, next page everywhere else

# Tap zone layout in words, the accessibility value of its tile: Wide next flipped top and bottom, previous page on the right.
reader-tap-wide-next-bottom-right-about = Controls along the bottom, previous page in a narrow column on the right, next page everywhere else

# Tap zone layout in words, the accessibility value of its tile: Top and bottom.
reader-tap-top-bottom-about = Next page across the bottom, previous page across the top, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: Top and bottom flipped.
reader-tap-top-bottom-flipped-about = Next page across the top, previous page across the bottom, controls in the middle

# Tap zone layout in words, the accessibility value of its tile: Next on both sides.
reader-tap-both-next-about = Next page down both sides, previous page at the bottom in the middle, controls above it

# Tap zone layout in words, the accessibility value of its tile: Next on both sides flipped top and bottom.
reader-tap-both-next-flipped-about = Next page down both sides, previous page at the top in the middle, controls below it

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Sides, next page on the right.
reader-tap-sides-right-scroll-about = Scroll down on the right side, scroll up on the left side, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Sides, next page on the left.
reader-tap-sides-left-scroll-about = Scroll down on the left side, scroll up on the right side, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# L shape, next page low and on the right.
reader-tap-l-shape-bottom-right-scroll-about = Scroll down along the bottom and on the right, scroll up along the top and on the left, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# L shape, next page low and on the left.
reader-tap-l-shape-bottom-left-scroll-about = Scroll down along the bottom and on the left, scroll up along the top and on the right, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# L shape flipped top and bottom, next page high and on the right.
reader-tap-l-shape-top-right-scroll-about = Scroll down along the top and on the right, scroll up along the bottom and on the left, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# L shape flipped top and bottom, next page high and on the left.
reader-tap-l-shape-top-left-scroll-about = Scroll down along the top and on the left, scroll up along the bottom and on the right, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Wide next, controls on top, previous page on the left.
reader-tap-wide-next-top-left-scroll-about = Controls along the top, scroll up in a narrow column on the left, scroll down everywhere else

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Wide next, controls on top, previous page on the right.
reader-tap-wide-next-top-right-scroll-about = Controls along the top, scroll up in a narrow column on the right, scroll down everywhere else

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Wide next flipped top and bottom, previous page on the left.
reader-tap-wide-next-bottom-left-scroll-about = Controls along the bottom, scroll up in a narrow column on the left, scroll down everywhere else

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Wide next flipped top and bottom, previous page on the right.
reader-tap-wide-next-bottom-right-scroll-about = Controls along the bottom, scroll up in a narrow column on the right, scroll down everywhere else

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Top and bottom.
reader-tap-top-bottom-scroll-about = Scroll down across the bottom, scroll up across the top, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Top and bottom flipped.
reader-tap-top-bottom-flipped-scroll-about = Scroll down across the top, scroll up across the bottom, controls in the middle

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Next on both sides.
reader-tap-both-next-scroll-about = Scroll down on both sides, scroll up at the bottom in the middle, controls above it

# Tap zone layout in words for a strip or a scrolled novel, the accessibility value of its tile:
# Next on both sides flipped top and bottom.
reader-tap-both-next-flipped-scroll-about = Scroll down on both sides, scroll up at the top in the middle, controls below it

# Button in the reader's controls that opens the chapters and the page grid.
# @max 16
reader-chapters = Chapters

# Line in the margin of a novel's page.
# @arg count: int
# @max 44
reader-pages-left = { $count ->
    [one] One page left in this chapter
   *[other] { $count } pages left in this chapter
}

# Chapters sheet and the chapter list: the list is on its way.
# @max 30
reader-chapters-loading = Loading chapters

# Background choice.
# @max 16
# @max 24
reader-background-theme = Theme

# Background choice.
# @max 16
# @max 24
reader-background-black = Black

# Background choice.
# @max 16
# @max 24
reader-background-white = White

# Title of the end page when the series has no newer chapter.
# @max 40
# @max 24
reader-end-caught-up = You're caught up

# A choice for the gap between a strip's images: none at all.
# @max 12
# @max 24
reader-gap-none = None

# A choice for the gap between a strip's images: a small one.
# @max 12
# @max 24
reader-gap-small = Small

# Look choice that follows the system's light or dark.
# @max 16
# @max 24
reader-look-system = System

# Top line of the page between chapters. The chapter is its number, such as 12.
# @arg chapter: text
# @max 40
reader-end-of-chapter-number = End of chapter { $chapter }

# The next chapter's facts on the page between chapters: its length and how it reads. The chapter is its number, such as 13; the facts are already written.
# @arg chapter: text
# @arg facts: text
# @max 60
reader-end-facts = Chapter { $chapter } · { $facts }

# The next chapter's length on the page between chapters.
# @arg count: int
# @max 24
reader-end-pages = { $count ->
    [one] One page
   *[other] { $count } pages
 }

# Under the next chapter's title on the page between chapters, when nothing more is known of it than the source it came from.
# @arg source: text
# @max 40
reader-end-from = From { $source }

# Under the series title in the reader's controls, and the title on a strip's marker into the next chapter. The chapter is its number, such as 12.
# @arg chapter: text
# @max 30
reader-chapter-line = Chapter { $chapter }

# The next chapter's facts and where it came from: on the divider inside a strip, and under the next chapter's title on the page between chapters.
# @arg facts: text
# @arg source: text
# @max 60
reader-divider-facts-source = { $facts } · from { $source }

# Line on the page between chapters, near its buttons: one more swipe opens the next chapter.
# @max 30
reader-end-swipe = Swipe to keep reading

# Button beside Continue on the page between chapters, shown only when the next chapter is an extra (such as 16.5 between 16 and 17). It opens the chapter named, passing over the extras before it. The chapter is its number, such as 17.
# @arg chapter: text
# @max 32
reader-end-skip-to = Skip to chapter { $chapter }

# What a screen reader adds after "Skip to chapter 17": the extra it passes over stays unread in the chapter list. The chapter is the extra's number, such as 16.5.
# @arg chapter: text
# @max 60
reader-end-skip-hint = Chapter { $chapter } stays unread.

# What a screen reader adds after "Skip to chapter 17" when it passes over more than one extra, or an extra with no number (such as an omake): those chapters stay unread in the chapter list.
# @max 60
reader-end-skip-hint-extras = The extras before it stay unread.

# The next episode's length on the divider inside a strip.
# @arg count: int
# @max 24
reader-divider-images = { $count ->
    [one] One image
   *[other] { $count } images
 }

# Under the series title in the reader's controls, and the title on a strip's marker into the next chapter. The chapter is its number, such as 12.
# @arg chapter: text
# @arg title: text
# @max 60
reader-chapter-line-titled = Chapter { $chapter } · { $title }

# The reader's opening line while the series' source sends its chapter list.
# @arg source: text
reader-wait-chapter-list = Getting the chapter list from { $source }

# The reader's opening line while the source sends the chapter's pages or text.
# @arg chapter: text
# @arg source: text
reader-wait-chapter-pages = Getting chapter { $chapter } from { $source }

# The opening line after a few seconds, with how long it has waited so far.
# @arg line: text
# @arg seconds: int
reader-wait-seconds = { $line } · { $seconds } s

# Under the opening line once the wait has gone on for a long time, above Read from another source and Go back.
reader-wait-long = This is taking a while.

## Credit pages and chapter parts

# Beside the line that says how many credit pages were skipped, on the end page and in a long
# strip. Tapping it shows them.
reader-credit-show = Show

# Spoken hint of the line that says how many credit pages were skipped.
reader-credit-show-hint = Shows the page

# Notice over a credit page shown again, and the caption above it in a long strip.
reader-credit-shown = Skipped as a credit page

# The notice's action on a shown credit page: show this page from now on.
reader-credit-not = Not a credit page

# Notice after Not a credit page, with Undo.
reader-credit-kept = This page will show from now on

# In the reader's line in place of a page number, while a credit page shows: "Chapter 87 · credit page".
reader-credit-page = credit page

# The same, when more than one credit page is shown: "credit page 1 of 2".
# @arg n: plain
# @arg total: plain
reader-credit-page-of = credit page { $n } of { $total }

# The reader's line under the title while a credit page shows: the chapter, then "credit page"
# or "credit page 1 of 2".
# @arg chapter: text
# @arg place: text
reader-group-line-place = { $chapter } · { $place }

# Spoken name of a credit page shown again.
reader-credit-page-label = Credit page

# Notice when a skipped credit page's picture can't be read, so it can't be shown.
reader-credit-failed = Couldn't load this credit page

# The scrubber's preview on the first page of a chapter's later part.
# @arg page: plain
# @arg part: plain
reader-part-preview = Page { $page } · part { $part }

# The same, when the part before it is missing from every group: "Page 14 · part 4 · part 3
# missing".
# @arg page: plain
# @arg part: plain
# @arg missing: plain
reader-part-preview-missing = Page { $page } · part { $part } · part { $missing } missing

# Spoken value of the scrubber on the first page of a chapter's later part.
# @arg part: plain
# @arg page: plain
reader-part-starts = Part { $part } starts at page { $page }

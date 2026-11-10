# Under a page that is still loading after four seconds.
# @arg page: plain
# @max 40
reader-page-loading = Loading page { $page }

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
# @max 12
reader-tap-edges = Edges

# A choice for the reader's tap zones: the left and right halves turn the page.
# @max 12
reader-tap-sides = Halves

# A choice for the reader's tap zones: taps do not turn the page.
# @max 12
reader-tap-off = Off

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

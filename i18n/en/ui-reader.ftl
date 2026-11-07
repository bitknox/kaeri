# Under a page that is still loading after four seconds.
# @arg page: int
# @max 40
reader-page-loading = Loading page { $page }

# Heading where a page could not be loaded.
# @arg page: int
# @max 40
reader-page-failed = Couldn't load page { $page }

# Button that loads a failed page again.
# @max 20
reader-try-again = Try again

# Closes the reader and goes back to the series.
# @max 16
reader-close = Close

# Under the top line of the page between chapters, once the chapter counts as read.
# @max 16
reader-end-read = Read

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

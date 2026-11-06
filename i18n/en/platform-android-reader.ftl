## The Android reader and its e-ink layout: the words the phone app and the e-reader build share.

# Under a page that is still loading after four seconds.
# @only android
# @arg page: int
# @max 40
reader-page-loading = Loading page { $page }

# Heading where a page could not be loaded.
# @only android
# @arg page: int
# @max 40
reader-page-failed = Couldn't load page { $page }

# Under the failed page's reason on a phone: turning on skips the missing page.
# @only android
# @max 60
reader-page-failed-swipe = Swipe to go on without it.

# Button that loads a failed page again.
# @only android
# @max 20
reader-try-again = Try again

# Button that goes on past a page that could not be loaded.
# @only android
# @max 20
reader-skip-page = Skip page

# What a screen reader says for the page on screen.
# @only android
# @arg page: int
# @arg pages: int
# @max 40
reader-page-of = Page { $page } of { $pages }

# What a screen reader says for two pages side by side.
# @only android
# @arg first: int
# @arg second: int
# @arg pages: int
# @max 40
reader-pages-of = Pages { $first } and { $second } of { $pages }

# A screen reader action, and the label of the tap zone that turns forward.
# @only android
# @max 24
reader-action-next = Next page

# A screen reader action, and the label of the tap zone that turns back.
# @only android
# @max 24
reader-action-previous = Previous page

# A screen reader action, and the label of the tap zone in the middle that shows the reader's controls.
# @only android
# @max 24
reader-action-controls = Controls

# Under Next page on the e-ink tap zone guide, for a book read right to left.
# @only android
# @max 30
eink-zone-next-rtl = Tap, or swipe right

# Under Next page on the e-ink tap zone guide, for a book read left to right.
# @only android
# @max 30
eink-zone-next-ltr = Tap, or swipe left

# Under Controls on the e-ink tap zone guide.
# @only android
# @max 20
eink-zone-tap = Tap

# The page number in the page's margin on an e-ink screen.
# @only android
# @arg page: int
# @arg pages: int
# @max 20
eink-folio-one = { $page } of { $pages }

# The page numbers in the margin when two pages show side by side.
# @only android
# @arg first: int
# @arg second: int
# @arg pages: int
# @max 30
eink-folio-two = { $first } and { $second } of { $pages }

# Under the series title in the e-ink reader's top bar. The chapter is its number, such as 12.
# @only android
# @arg chapter: text
# @arg title: text
# @max 60
eink-chapter-titled = Chapter { $chapter } · { $title }

# Under the series title when the chapter has no title of its own.
# @only android
# @arg chapter: text
# @max 30
eink-chapter = Chapter { $chapter }

# Button in the e-ink reader's top bar that lists the chapters.
# @only android
# @max 16
eink-contents = Contents

# Button that goes back out of the reader.
# @only android
# @max 16
eink-back = Back

# Under the page number in the e-ink reader's bottom bar.
# @only android
# @arg count: int
# @max 40
eink-pages-left = { $count ->
    [one] One page left in chapter
   *[other] { $count } pages left in chapter
 }

# Button in the e-ink reader's bottom bar.
# @only android
# @max 20
eink-next-chapter = Next chapter

# Button in the e-ink reader's bottom bar.
# @only android
# @max 24
eink-previous-chapter = Previous chapter

# Button in the e-ink reader's bottom bar that opens the reader's settings.
# @only android
# @max 16
eink-settings = Settings

# Button that asks the e-ink screen for a full refresh, to clear what earlier pages left behind.
# @only android
# @max 20
eink-clean-screen = Clean screen

# Top line of the page between chapters. The chapter is its number, such as 12.
# @only android
# @arg chapter: text
# @max 40
eink-end-of-chapter = End of chapter { $chapter }

# On the page between chapters, once the chapter counts as read.
# @only android
# @max 40
eink-counted-read = Counted as read

# Small heading above the next chapter on the page between chapters.
# @only android
# @max 16
eink-end-next = Next

# The next chapter's length on the page between chapters.
# @only android
# @arg count: int
# @max 20
eink-end-pages = { $count ->
    [one] One page
   *[other] { $count } pages
 }

# How the next chapter reads, on the page between chapters.
# @only android
# @max 20
eink-reads-rtl = right to left

# How the next chapter reads, on the page between chapters.
# @only android
# @max 20
eink-reads-ltr = left to right

# How the next chapter reads, on the page between chapters.
# @only android
# @max 20
eink-reads-strip = long strip

# The next chapter is downloaded, on the page between chapters.
# @only android
# @max 20
eink-on-device = on this { -device }

# The main button on the page between chapters.
# @only android
# @arg chapter: text
# @max 32
eink-continue-to = { -continue-cap } to chapter { $chapter }

# Button on the page between chapters that leaves the reader for the series page.
# @only android
# @max 24
eink-back-to-series = Back to series

# Line at the foot of the page between chapters.
# @only android
# @max 40
eink-turn-to-continue = Turn the page to { -continue }

# Heading on the page between chapters when no source has the next chapter yet.
# @only android
# @arg chapter: text
# @max 60
eink-next-missing = Chapter { $chapter } isn't in your { -source-pl } yet

# Heading on the page between chapters when the next chapter is not out.
# @only android
# @max 60
eink-next-not-out = The next chapter isn't out yet

# Heading on the page between chapters when every chapter out so far is read.
# @only android
# @max 60
eink-caught-up = You're caught up

# Line under those headings.
# @only android
# @max 140
eink-checks-again = { -app-name } checks your { -source-pl } again when you open Updates or this series. It will show under New chapters.

# Heading on the page between chapters when the series has no more chapters.
# @only android
# @max 60
eink-finished = That was the last chapter

# Line under it.
# @only android
# @max 60
eink-finished-line = The series is finished.

# Heading on the page between chapters when the next chapter needs a connection.
# @only android
# @max 60
eink-next-offline = Not on this { -device }, and you're offline

# Line under it.
# @only android
# @max 100
eink-next-offline-line = Connect to Wi-Fi to load it, or open something already here.

# Button on the page between chapters that opens the downloads.
# @only android
# @max 20
eink-open-downloads = Open downloads

# The one line an e-ink screen shows while a chapter loads.
# @only android
# @arg chapter: text
# @max 30
eink-loading-chapter = Loading chapter { $chapter }

# Under a failed page on an e-ink screen.
# @only android
# @max 60
eink-page-failed-details = The details are in More, under Messages.

# Heading of the question asked when the device may have an e-ink screen.
# @only android
# @max 40
eink-ask-title = Is this an e-ink screen?

# Line under it.
# @only android
# @max 240
eink-ask-body = { -app-name } found signs of an e-ink screen but couldn't confirm it. The e-ink layout pages instead of scrolling, never animates and asks the screen to clear itself when it needs to.

# The answer that turns the e-ink layout on.
# @only android
# @max 28
eink-ask-yes = Use the e-ink layout

# The answer that keeps the phone layout.
# @only android
# @max 34
eink-ask-no = Keep the standard layout

# Foot of the e-ink question.
# @only android
# @max 80
eink-ask-later = You can change this later in Settings, under Screen.

# Heading of the question asked when an e-ink screen may or may not show colour.
# @only android
# @max 40
eink-colour-title = Does this screen show colour?

# Line under it, above four coloured squares.
# @only android
# @max 90
eink-colour-body = { -app-name } couldn't tell from this { -device }. Look at the four squares.

# What a screen reader says for the four squares.
# @only android
# @max 60
eink-colour-squares = Four squares: red, green, blue and amber

# Line under the squares.
# @only android
# @max 120
eink-colour-hint = If they look like four greys, choose Only greys. You can change this in Settings › Screen.

# The answer for a colour screen.
# @only android
# @max 20
eink-colour-yes = I see colour

# The answer for a grey screen.
# @only android
# @max 20
eink-colour-no = Only greys

# Leaves the colour question unanswered.
# @only android
# @max 12
eink-colour-skip = Skip

# Shown after the colour question was skipped.
# @only android
# @max 90
eink-colour-skipped = Skipped, pictures stay in colour, which a grey screen shows as greys.

# A one-line hint on e-readers whose page buttons may be set to something other than Volume.
# @only android
# @max 100
eink-page-keys-hint = Set the page buttons to Volume in the { -device }'s settings to turn pages.

# Beside a dot on a chapter or series that is new since it was last opened.
# @only android
# @max 12
eink-new = New

# The action on the notice that says a chapter opened where it was left: back to its first page.
# @only android
# @max 16
reader-notice-start-over = Start over

# The action on the notice that says another device read further: back to where this one was.
# @only android
# @max 16
reader-notice-go-back = Go back

# The action on the notice that says how a series was detected to read: change it.
# @only android
# @max 16
reader-notice-change = Change

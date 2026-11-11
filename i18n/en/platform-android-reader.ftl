## The Android reader and its e-ink layout: the words the phone app and the e-reader build share.

# Under the failed page's reason on a phone: turning on skips the missing page.
# @only android
# @max 60
reader-page-failed-swipe = Swipe to go on without it.

# Button that goes on past a page that could not be loaded.
# @only android
# @max 20
reader-skip-page = Skip page

# What a screen reader says for the page on screen.
# @only android
# @arg page: plain
# @arg pages: plain
# @max 40
reader-page-of = Page { $page } of { $pages }

# What a screen reader says for two pages side by side.
# @only android
# @arg first: plain
# @arg second: plain
# @arg pages: plain
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
# @arg page: plain
# @arg pages: plain
# @max 20
eink-folio-one = { $page } of { $pages }

# The page numbers in the margin when two pages show side by side.
# @only android
# @arg first: plain
# @arg second: plain
# @arg pages: plain
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
eink-continue-to = { -continue-reading-cap } to chapter { $chapter }


# Line at the foot of the page between chapters.
# @only android
# @max 40
eink-turn-to-continue = Turn the page to { -continue-reading }

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
eink-ask-later = You can change this later in Settings, under Reading.

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
eink-colour-hint = If they look like four greys, choose Only greys. You can change this in Settings › Reading.

# The answer for a colour screen.
# @only android
# @max 20
eink-colour-yes = Yes, colour

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
eink-colour-skipped = Skipped. Pictures stay in colour. A grey screen shows them as greys.

# A one-line hint on e-readers whose page buttons may be set to something other than Volume.
# @only android
# @max 100
eink-page-keys-hint = To turn pages with the buttons, set them to Volume in the { -device }'s settings.

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

# Button in the reader's controls that opens its settings, drawn as Aa.
# @only android
# @max 24
reader-settings = Reader settings

# Under the novel's title in the reader's controls, while it scrolls. The chapter is its number, such as 4.
# @only android
# @arg chapter: text
# @max 50
reader-novel-line-scroll = Chapter { $chapter } · scroll

# At the end of a scrolling novel's scrubber: the time left in the chapter at the person's pace.
# @only android
# @arg minutes: int
# @max 12
reader-minutes-left = { $minutes } min

# Name of the tap zone on the reading side, shown once on the first chapter.
# @only android
# @max 24
reader-zone-next = Next page

# Heading of the page grid.
# @only android
# @max 20
reader-grid-title = Pages

# Main button on the page between chapters.
# @only android
# @max 16
reader-end-continue = { -continue-reading-cap }

# Quiet button on the page between chapters that leaves the reader for the series page.
# @only android
# @max 24
reader-end-series = Back to the series

# Button on the last page of a finished series that starts it from the first chapter.
# @only android
# @max 34
reader-end-again = Read again from the start

# The one line while a chapter opens. The chapter is its number, such as 13.
# @only android
# @arg chapter: text
# @max 40
reader-opening-chapter = Opening chapter { $chapter }

# Heading when a chapter could not open. The chapter is its number, such as 13.
# @only android
# @arg chapter: text
# @max 50
reader-failed-chapter = Couldn't open chapter { $chapter }

# Heading in the reader's settings above the choices that belong to this series only.
# @only android
# @max 20
reader-sheet-series = This series

# Heading in the reader's settings above the choices every manga shares.
# @only android
# @max 20
reader-sheet-all-manga = All manga

# Heading in the reader's settings above the choices every webtoon shares.
# @only android
# @max 20
reader-sheet-all-webtoons = All webtoons

# Heading in the reader's settings above the choices every light novel shares.
# @only android
# @max 24
reader-sheet-all-novels = All light novels

# Row in the reader's settings that opens the rest of the settings.
# @only android
# @max 30
reader-sheet-more = More reader settings

# Row in the reader's settings: how the pages are read.
# @only android
# @max 20
reader-row-reading-mode = Reading mode

# Value of the reading mode when the reader worked it out. The way is how it reads, such as right to left.
# @only android
# @arg way: text
# @max 30
reader-mode-auto-way = Auto · { $way }

# The first choice in the reading mode picker: let the reader work it out.
# @only android
# @max 12
reader-mode-auto-name = Auto

# Line under Auto in the reading mode picker. The way is how it reads, such as right to left.
# @only android
# @arg way: text
# @max 50
reader-mode-auto-line = { $way }, from the series details

# Line under Strip in the reading mode picker.
# @only android
# @max 40
reader-mode-strip-line = For webtoons and long images

# Note under the reading mode when the reader guessed it.
# @only android
# @max 44
reader-mode-detected = Detected from the series details.

# Note under the reading mode after a choice for this series. The title is the series'.
# @only android
# @arg title: text
# @max 60
reader-mode-set = Set for { $title } on all your { -device-pl }.

# Link in that note that drops the series' own choice.
# @only android
# @max 16
reader-mode-use-default = Use default

# Foot of the reading mode picker.
# @only android
# @max 110
reader-mode-saved = Your choice is saved for this series and follows it to your other { -device-pl }.

# Row in the reader's settings: how a page is fitted to the screen.
# @only android
# @max 12
reader-row-fit = Fit

# Row in the reader's settings: cut the blank borders off pages.
# @only android
# @max 20
reader-row-crop = Crop borders

# Row in the reader's settings: the switch for Paper mode.
# @only android
# @max 20
reader-row-paper = Paper mode

# Row in the reader's settings: how big the text is.
# @only android
# @max 16
reader-row-text-size = Text size

# Row in the reader's settings: the typeface of a novel.
# @only android
# @max 12
reader-row-font = Font

# Row in the reader's settings: the colours the page is read in.
# @only android
# @max 12
reader-row-look = Look

# Row in the reader's settings: the space between lines.
# @only android
# @max 20
reader-row-line-spacing = Line spacing

# A choice of line spacing or margins.
# @only android
# @max 12
reader-spacing-tight = Tight

# A choice of line spacing or margins.
# @only android
# @max 12
reader-spacing-normal = Normal

# A choice of line spacing.
# @only android
# @max 12
reader-spacing-loose = Loose

# Row in the reader's settings: the space around the text.
# @only android
# @max 12
reader-row-margins = Margins

# Row in the reader's settings: whether a novel turns pages or scrolls.
# @only android
# @max 12
reader-row-pages = Pages

# A choice for how a novel moves.
# @only android
# @max 12
reader-pages-paged = Paged

# A choice for how a novel moves.
# @only android
# @max 12
reader-pages-scroll = Scroll

# Row in the reader's settings: even text edges.
# @only android
# @max 20
reader-row-justify = Justify text

# Row in the reader's settings: one page or two side by side.
# @only android
# @max 20
reader-row-page-layout = Page layout

# Row in the reader's settings: what happens to a page that is two pages wide.
# @only android
# @max 20
reader-row-spreads = Wide pages

# Row in the reader's settings: whether pages are shown in colour.
# @only android
# @max 20
reader-row-colour = Colour pages

# A choice for colour.
# @only android
# @max 16
reader-colour-auto = As drawn

# A choice for colour.
# @only android
# @max 12
reader-colour-colour = Colour

# A choice for colour: only greys.
# @only android
# @max 12
reader-colour-greys = Greys

# Row in the reader's settings: where taps turn the page.
# @only android
# @max 20
reader-row-tap = Tap zones

# Row in the reader's settings: the volume keys turn pages.
# @only android
# @max 30
reader-row-volume = Volume keys turn pages

# Row in the reader's settings: how wide a strip is on a big screen.
# @only android
# @max 20
reader-row-strip-width = Strip width

# Row in the reader's settings, inside Paper mode: show pages in greys.
# @only android
# @max 20
reader-row-greyscale = Greyscale pages

# Row in the reader's settings, inside Paper mode: hold notifications back while reading.
# @only android
# @max 40
reader-row-quiet = Quiet the phone while reading

# Line under that row.
# @only android
# @max 130
reader-quiet-line = Notifications wait until you close the reader. Android asks for access to Do Not Disturb first.

# Notice the first time the phone is quiet while reading.
# @only android
# @max 44
reader-quiet-notice = Notifications wait while you read

# Row in the reader's settings: the line at the foot of a novel's page.
# @only android
# @max 30
reader-row-foot = Pages left in chapter

# Row in the reader's settings: the page number in the margin.
# @only android
# @max 24
reader-row-folio = Show page number

# Row in the reader's settings: the reading direction.
# @only android
# @max 16
reader-row-direction = Direction

# Heading in the reader's settings page.
# @only android
# @max 16
reader-more-heading-layout = Layout

# Heading in the reader's settings page.
# @only android
# @max 16
reader-more-heading-navigation = Navigation

# Heading in the reader's settings page.
# @only android
# @max 16
reader-more-heading-text = Text

# Heading in the reader's settings page.
# @only android
# @max 16
reader-more-heading-images = Images

# Heading in the reader's settings page.
# @only android
# @max 16
reader-more-heading-paper = Paper mode

# Button that goes back from the reader's settings page to the sheet.
# @only android
# @max 12
reader-settings-back = Back

# Line under the Paper mode switch.
# @only android
# @max 100
reader-paper-line = Pages turn at once and nothing in the reader moves. Change any part below.

# What a screen reader says for a page in the page grid.
# @only android
# @arg page: plain
# @max 30
reader-grid-page = Go to page { $page }

# What a screen reader says for the slider that moves through the chapter.
# @only android
# @max 24
reader-scrubber-in-chapter = Page in chapter

# Heading when a novel could not be shown.
# @only android
# @max 40
reader-failed-novel = Couldn't show this chapter

# Title of the end page when no source has the next chapter.
# @only android
# @arg chapter: text
# @max 60
reader-end-none = No chapter { $chapter } in your { -source-pl } yet

# Line under the no next chapter title.
# @only android
# @max 120
reader-end-none-line = { -app-name } checks for new chapters in the background and adds them to Updates.

# Title of the end page after the last chapter of a finished series.
# @only android
# @max 30
reader-end-finished-title = The end

# Line under the end title of a finished series.
# @only android
# @max 60
reader-end-finished-line = This series has no more chapters.

# Title of the end page when the next chapter needs a connection.
# @only android
# @arg chapter: text
# @max 60
reader-end-offline-title = Chapter { $chapter } isn't downloaded

# Line under the offline end page title.
# @only android
# @max 140
reader-end-offline-line = You're offline. It opens here once you're connected, or download it on the series page for next time.

# The name of the Do Not Disturb mode in the system's list.
# @only android
# @max 30
reader-quiet-rule = Reading in { -app-name }

# What the system says starts the reading mode.
# @only android
# @max 40
reader-quiet-trigger = While the reader is open

# Shown when access to Do Not Disturb was refused.
# @only android
# @max 100
reader-quiet-denied = Do Not Disturb access wasn't allowed, so notifications keep coming.

# Setting row for the ground behind a page.
# @only android
# @max 24
reader-row-background = Background

# Fit choice.
# @only android
# @max 16
reader-fit-height = Height

# Reading mode.
# @only android
# @max 24
reader-mode-vertical = Vertical pages

# Line under the vertical pages mode.
# @only android
# @max 40
reader-mode-vertical-line = Pages one under another

# Heading of a volume in the chapter list.
# @only android
# @arg volume: text
# @max 24
reader-volume-line = Volume { $volume }

# A folded volume whose chapters are all read.
# @only android
# @arg volume: text
# @max 30
reader-volume-read = Volume { $volume } · read

# Heading for chapters outside any volume.
# @only android
# @max 24
reader-volume-none = Chapters

# On the plate of a page that is still loading.
# @only android
# @arg page: plain
# @max 20
reader-page-plate = Page { $page }

# On the plate of a strip image that is still loading.
# @only android
# @arg image: int
# @max 20
reader-image-plate = Image { $image }

# On a strip image's plate in place of its number after four seconds.
# @only android
# @arg image: int
# @max 30
reader-image-loading = Loading image { $image }

# Under Opening chapter when it takes more than three seconds.
# @only android
# @max 30
reader-opening-finding = Finding a { -source }

# Button on a chapter that could not open.
# @only android
# @max 24
reader-failed-choose = Choose { -source }

# Hint in the settings search field.
# @only android
# @max 30
reader-settings-search = Search settings

# Paper mode part.
# @only android
# @max 30
reader-paper-turn = Turn pages at once

# Label on the previous page tap zone.
# @only android
# @max 24
reader-zone-previous = Previous page

# Label on the middle tap zone.
# @only android
# @max 24
reader-zone-controls = Controls

# Description of the small A at the start of the text size slider.
# @only android
# @max 24
reader-size-small = Smaller text

# Description of the large A at the end of the text size slider.
# @only android
# @max 24
reader-size-large = Larger text

# The value of Paper mode in the settings list when it is on.
# @only android
# @max 12
reader-value-on = On

# The value of Paper mode in the settings list when it is off.
# @only android
# @max 12
reader-value-off = Off

# Row in the reader's settings for strips: the space between a strip's images.
# @only android
# @max 30
reader-row-strip-gap = Gap between images

# Paper mode part: the Paper look while it is day, the Dark look at night.
# @only android
# @max 30
reader-paper-look = Paper look by day

# Paper mode part: a slider for how far the page's white is dimmed.
# @only android
# @max 30
reader-paper-dim = Dim the page

# The opening screen's line after 3 s: what it waits on, then how long it has waited, such as "Finding a source · 4 s".
# @only android
# @arg line: text
# @arg seconds: int
# @max 60
reader-opening-waited = { $line } · { $seconds } s

# Heading of the list of other sources a failed chapter can be read from.
# @only android
# @max 40
reader-sources-title = Read this chapter from

# Heading on the Paper mode page above the parts it changes.
# @only android
# @max 30
reader-paper-parts = What it changes

# The e-reader's last tab: sources, trackers, sync, settings and About, each a row.
# @only android
# @max 12
eink-tab-more = More

# The link at the end of a heading on e-ink Home that opens the whole list, such as all of Continue.
# @only android
# @max 12
eink-home-all = All

# Under a small cover in Continue on a 6 inch e-reader's Home: the chapter and page the person
# stopped on, shortened to fit one line under a narrow cover. "Ch" is short for chapter and "p"
# for page; use your language's usual short forms.
# @only android
# @arg chapter: text
# @arg page: plain
# @max 24
eink-place-chapter-page = Ch { $chapter } · p { $page }

# Under a small cover in Continue on a 6 inch e-reader's Home: how far into a novel's chapter the
# person is, shortened to fit one line under a narrow cover. "Ch" is short for chapter.
# @only android
# @arg chapter: text
# @arg percent: int
# @max 24
eink-place-chapter-percent = Ch { $chapter } · { $percent }%

# Heading on the e-reader's Home over the newest chapters that came out; All opens Updates.
# @only android
# @max 30
eink-home-new-chapters = New chapters

# A row on the e-reader's More page that opens the tracker accounts.
# @only android
# @max 30
eink-more-trackers = { -tracker-cap-pl }

# A row on the e-reader's More page that opens syncing with other devices.
# @only android
# @max 30
eink-more-sync = Sync

# The line under Settings on the e-reader's More page: what the settings cover.
# @only android
# @max 60
eink-more-settings-line = Storage, notifications and language

# Under the title of the e-reader's Downloads: what downloads take and what is free, such as "1.3 GB on this device · 21 GB free".
# @only android
# @arg used: text
# @arg free: text
# @max 60
eink-downloads-space = { $used } on this { -device } · { $free } free

# Under the title of the e-reader's Downloads when the free space isn't known.
# @only android
# @arg used: text
# @max 60
eink-downloads-space-used = { $used } on this { -device }

# Heading on the e-reader's Downloads over the chapters downloading or waiting.
# @only android
# @max 30
eink-downloads-downloading = Downloading

# Heading on the e-reader's Downloads over the series with chapters kept here.
# @only android
# @max 30
eink-downloads-here = On this { -device }

# The line under Keep next chapters ready on the e-reader's Downloads while it is on.
# @only android
# @max 80
eink-keep-ready-on = On · the next chapters of series you're reading

# The line under Keep next chapters ready on the e-reader's Downloads while it is off.
# @only android
# @max 60
eink-keep-ready-off = Off

# Switch under Volume keys turn pages in the phone reader's settings: volume up turns forward
# instead of volume down.
# @only android
# @max 40
reader-row-volume-swap = Swap the volume keys

# Heading of the e-ink reader's settings sheet over the rows for the e-ink screen itself.
# @only android
# @max 30
eink-sheet-screen = This screen

# Row in the e-ink reader's settings: a full refresh on every page with a picture. Off by default,
# so the device's own settings decide how the screen clears.
# @only android
# @max 40
eink-row-refresh-image = Full refresh on picture pages

# Row in the e-ink reader's settings: how often text pages get a full refresh, or Off, the default.
# @only android
# @max 36
eink-row-refresh-text = Full refresh on text pages

# Row in the e-ink reader's settings: how often lists get a full refresh as they page, or Off, the
# default.
# @only android
# @max 36
eink-row-refresh-list = Full refresh on lists

# Value of a full refresh row in the e-ink reader's settings: once every so many pages.
# @only android
# @arg count: int
# @max 24
eink-refresh-every = { $count ->
    [one] Every page
   *[other] Every { $count } pages
 }

# Row in the e-ink reader's settings: which of the screen's own update modes pages are drawn in,
# shown only where the app can set them. Its choices are the device's setting and the modes below,
# or Standard, Fast and Best quality on a colour screen with speed modes.
# @only android
# @max 30
eink-row-screen-mode = Screen mode

# Choice of Screen mode, and its default: the app leaves the update mode to the device's own
# settings for this app.
# @only android
# @max 28
eink-mode-device = The { -device }'s setting

# Choice of Screen mode: the screen's sharpest grey mode. HD is the name e-readers give this mode.
# @only android
# @max 16
eink-mode-hd = HD

# Choice of Screen mode: the screen's mode with the least ghosting on text pages. Regal is the name
# e-readers give this mode; keep it as the device shows it.
# @only android
# @max 16
eink-mode-regal = Regal

# Choice of Screen mode: quicker updates in fewer greys, with a little ghosting.
# @only android
# @max 16
eink-mode-balanced = Balanced

# Choice of Screen mode on a colour screen with speed modes: the screen's usual mode.
# @only android
# @max 16
eink-speed-standard = Standard

# Choice of Screen mode: quicker updates with more ghosting.
# @only android
# @max 16
eink-speed-fast = Fast

# Choice of Screen mode on a colour screen with speed modes: the slowest and cleanest updates.
# @only android
# @max 16
eink-speed-best = Best quality

# Row in the e-ink reader's settings on a colour screen: softens the rainbow pattern in colour pictures.
# @only android
# @max 36
eink-row-rainbow = Soften colour patterns

# Row in the e-ink reader's settings on a colour screen: how strong colours are drawn.
# @only android
# @max 30
eink-row-saturation = Colour strength

# Row in the e-ink reader's settings on a colour screen: the pictures' contrast.
# @only android
# @max 30
eink-row-contrast = Contrast

# Value of an amount row in the e-ink reader's settings, such as Text size or Contrast.
# @only android
# @arg percent: int
# @max 8
eink-percent = { $percent }%

# Button that makes a setting in the e-ink reader's settings smaller.
# @only android
# @max 30
eink-step-down = Less

# Button that makes a setting in the e-ink reader's settings bigger.
# @only android
# @max 30
eink-step-up = More

# Button before a list setting's value in the e-ink reader's settings, such as Screen mode: picks
# the choice before. Read out by screen readers.
# @only android
# @max 30
eink-step-previous = Previous choice

# Button after a list setting's value in the e-ink reader's settings, such as Screen mode: picks
# the choice after. Read out by screen readers.
# @only android
# @max 30
eink-step-next = Next choice

# Marks the chapter open in the reader on the e-ink contents page.
# @only android
# @max 20
eink-contents-reading = Reading

# What TalkBack says the line of skipped credit pages does, after "Double-tap to": an
# imperative, as Android's click labels are.
# @only android
# @max 24
reader-credit-show-action = Show the page

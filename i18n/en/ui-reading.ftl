# Reading's places in the apps: Read, Updates, the series page, the reading library, sources for
# books and what Kaeri is for. Words for chapters, volumes and pages. Series and chapter titles,
# source names and tracker names arrive as arguments.

## Sections

# Sidebar and tab label: the shows' Home when both media are on.
nav-watch = Watch

# Sidebar and tab label: reading's Home.
nav-read = Read

# Sidebar and tab label: new chapters from followed series.
nav-updates = Updates

## Read, reading's Home

# Shelf heading on Read: the series being read, each at its place.
home-read-continue = { -continue-reading-cap }

# Shelf heading on Read: series releasing now.
home-read-releasing = Releasing now

# Line under the spotlight of a popular series, when the engine gives no line of its own.
home-read-checked-on-open = Checked when you open { -app-name }

# Line at the end of Read, under the last shelf.
home-read-end = That's everything for now. Updates and Search have the full lists.

# Line at the end of Read with no book sources yet. A button to add one follows it.
home-read-no-sources = Add a folder, a server or a { -plugin-store } to read from Home.

# In place of Read's discovery rows on a first run with no connection: nothing was kept yet.
read-home-offline-rows = Trending and new series show once you're online.

# E-ink: the one row on Read's Home that opens the list of discovery rows, and that page's title.
# @max 24
read-home-browse = Browse

# E-ink: the line under Browse on Read's Home.
read-home-browse-line = Trending, new and finished, by format

# E-ink: a discovery row in the Browse list, naming its first two series and how many follow.
# @arg first: text
# @arg second: text
# @arg more: int
read-home-browse-row = { $first }, { $second } and { $more } more

# E-ink: the note on the Browse page. The rows are AniList's catalogue; sources are asked only
# when a series is opened.
read-home-browse-note = From AniList. Your { -source-pl } are asked when you open a series.

# E-ink: on the Browse page when AniList didn't answer. The time is when the lists were kept.
# @arg time: text
read-home-browse-kept = Lists kept from { $time } still open.

# Under a day in Read's week: what usually comes out that day, when the time isn't known.
week-usually = Usually

# Under a day in Read's week with nothing due.
week-nothing-usually = Nothing usually

## Updates

# Line under Updates' title on iPhone and iPad.
# @arg ago: text
updates-checked = Checked { $ago }

# Line above Updates on the Mac: when it checked, and which series it checks.
# @arg ago: text
chapter-updates-checked-caught-up = Checked { $ago } · series you are caught up on

# Line while Updates asks every followed series now.
chapter-updates-checking = Checking for new chapters…

# Toolbar button: queue every unread chapter in Updates.
updates-download-new = Download new

# Updates with nothing in it.
updates-empty-title = No new chapters

# Line under Updates' empty title.
chapter-updates-empty-line = { -app-name } checks series on your Reading list that you are caught up on, a few times a day.

# Day heading in Updates.
chapter-updates-today = Today

# Day heading in Updates.
chapter-updates-yesterday = Yesterday

# After a chapter on an Updates row, when several groups put it out.
# @arg n: int
chapter-updates-versions = { $n ->
    [one] 1 version
   *[other] { $n } versions
 }

# Accessibility label on an Updates row's cover, which opens the series.
# @arg title: text
chapter-updates-open-series = Open { $title }

# Accessibility hint on an Updates row.
chapter-updates-open-chapter-hint = Opens the chapter

# Context menu item on an Updates row.
chapter-updates-open-series-menu = Open series

## Main actions

# The series page's main action, and the spotlight's.
# @arg n: text
action-read-chapter = Read chapter { $n }

# The main action when a chapter is part read.
# @arg n: text
# @arg page: plain
action-continue-chapter = { -continue-reading-cap } chapter { $n } · page { $page }

# The main action for a light novel part, part read.
# @arg n: text
# @arg percent: int
action-continue-part = { -continue-reading-cap } part { $n } · { $percent }%

# The main action for a light novel part, part read, when how far is not known.
# @arg n: text
action-continue-part-plain = { -continue-reading-cap } part { $n }

# The main action for a light novel part not opened yet.
# @arg n: text
action-read-part = Read part { $n }

# The main action for a light novel part, part read, named in the source's own words ("Part 3",
# "Chapter 3") as its row is.
# @arg label: text
# @arg percent: int
action-continue-named = { -continue-reading-cap } { $label } · { $percent }%

# The main action for a light novel part, part read, named in the source's own words, when how
# far is not known.
# @arg label: text
action-continue-named-plain = { -continue-reading-cap } { $label }

# The main action for a light novel part not opened yet, named in the source's own words.
# @arg label: text
action-read-named = Read { $label }

# The main action when every chapter is read.
action-read-again = Read again

## The series page

# Label of the source row on the series page.
series-reading-from = Reading from

# Button beside a row on the series page that opens its sheet.
series-change = Change

# After the source's name: how many other sources have the series.
# @arg n: int
series-sources-more = { $n ->
    [one] 1 more
   *[other] { $n } more
 }

# Value of the trackers row while a tracker asks which series this is.
series-tracker-ask = Which series is this? Choose

# Value of the group row: the chosen group is used for this series.
series-preferred-group = Your preferred group here

# A tracker's progress in chapters, with the total.
# @arg n: int
# @arg total: int
series-tracker-chapters-of = { $n } of { $total ->
    [one] { $total } chapter
   *[other] { $total } chapters
 }

# A tracker's progress in chapters, total unknown.
# @arg n: int
series-tracker-chapters = { $n ->
    [one] 1 chapter
   *[other] { $n } chapters
 }

# A tracker's progress in chapters, then the volume.
# @arg chapters: text
# @arg volume: plain
series-tracker-with-volume = { $chapters } · volume { $volume }

# A light novel tracker's progress in volumes.
# @arg volume: plain
# @arg total: plain
series-tracker-volumes = Volume { $volume } of { $total }

# Main place when the source can't be read from.
series-read-from-another = Read from another { -source }

# Status menu item: a score submenu.
series-score = Score…

# Status menu's label before the series is on a list.
series-add-to-list = Add to list

# Last item of a series' list menu: take the series off every tracker's list. Asks first.
series-remove-from-lists = Remove from lists…

# Title of the question before a series is taken off the lists.
# @arg title: text
series-remove-from-lists-title = Remove { $title } from your lists?

# Line in that question.
series-remove-from-lists-line = It comes off your list on each { -tracker } you have connected. Your place and read chapters stay in { -app-name }.

# Heading over what the removal came to, one line per tracker.
series-removed-title = Your lists

# Title while choosing chapters on a phone: how many are chosen.
# @arg n: int
series-chosen = { $n ->
    [one] 1 chapter
   *[other] { $n } chapters
}

# Button while choosing chapters on a phone: choose every unread chapter.
series-choose-all-unread = All unread

# Heading in Read from over the sources searched without finding the series.
read-from-not-found = Not found in

# Button beside such a source: look for the series there by other words.
read-from-search-there = Search there

# Placeholder of the field that looks for the series in one source.
# @arg source: text
read-from-search-placeholder = Search { $source }

# Button that runs that search.
read-from-search = Search

# Under the field when the source found nothing by those words.
read-from-search-none = Nothing found by these words

# The More menu: the Tracking sheet.
series-tracking = Tracking

# The More menu: the Read from sheet.
series-read-from-menu = Change { -source }…

# The More menu: start again from chapter 1.
series-read-again-menu = Read again from the start…

# Heading of the chapter list when the series has volumes.
series-volumes = Volumes

# Heading of the chapter list.
series-chapters = Chapters

# Accessibility label of the chapter order menu.
series-order = Order

# Chapter order.
series-newest-first = Newest first

# Chapter order.
series-oldest-first = Oldest first

# A volume's heading in the chapter list.
# @arg n: text
series-volume = Volume { $n }

# The heading for chapters that are in no volume yet.
series-no-volume = Not in a volume yet

# After a volume's heading: which chapters it holds.
# @arg from: text
# @arg to: text
series-chapter-range = chapters { $from } to { $to }

# After a volume's heading when every chapter in it is read.
series-volume-read = read

# Accessibility value of a volume heading.
series-expanded = Expanded

# Accessibility value of a volume heading.
series-collapsed = Collapsed

# The chapter list when the source has none.
series-no-chapters-title = No chapters in this { -source } yet

# Line under it.
series-no-chapters-line = Another { -source } may have them.

# The chapter list when the source failed. Downloaded chapters are listed under it.
# @arg source: text
series-chapters-failed = Couldn't load chapters from { $source }.

# A chapter row's title.
# @arg n: text
series-chapter-number = Chapter { $n }

# Accessibility label of the bookmark mark on a chapter row.
series-bookmarked = Bookmarked

# Chapter menu and swipe action.
series-mark-read = Mark read

# Chapter menu and swipe action.
series-mark-unread = Mark unread

# Chapter menu item: every chapter before this one is read too.
series-mark-read-up-to = Mark read up to here

# Chapter menu and swipe action.
series-download = Download

# Chapter menu item: open the chapter in the reader.
series-read-chapter-menu = Read

# Chapter menu item.
series-bookmark = Bookmark

# Chapter menu item.
series-remove-bookmark = Remove bookmark

# Menu of a chapter that several groups put out.
# @arg n: int
series-versions = { $n ->
    [one] 1 version
   *[other] { $n } versions
 }

# In that menu while the versions load.
series-versions-loading = Looking for versions…

# The version in use, in that menu.
# @arg name: text
series-version-in-use = { $name } in use

# Make another group the one used for this series.
# @arg name: text
series-prefer-version = Prefer { $name } for this series

## Chapter states

# A downloaded chapter, after its state.
chapter-state-downloaded = Downloaded

# A chapter waiting to download.
chapter-state-queued = Queued

# A chapter waiting for Wi-Fi to download.
chapter-state-wifi = Queued · waits for Wi-Fi

# A chapter downloading.
# @arg done: int
# @arg total: int
chapter-state-downloading = Downloading · { $done } of { $total } pages

# A chapter that failed to download, with the reason.
# @arg reason: text
chapter-state-failed = Couldn't download. { $reason }

# A part-read chapter.
# @arg page: plain
# @arg of: plain
chapter-state-page-of = Page { $page } of { $of }

# A part-read chapter whose length isn't known.
# @arg page: plain
chapter-state-page = Page { $page }

# A read chapter.
chapter-state-read = Read

# A chapter that came out since you last looked.
chapter-state-new = New

# Button: stop a chapter's download.
chapter-download-stop = Stop download

# Button: try a failed chapter's download again.
chapter-retry = Retry

# Chapter menu item.
chapter-download-delete = Delete download

## Downloads on the series page

# Download menu item.
# @arg n: int
download-next = Next { $n } unread

# Download menu item.
download-unread = Every unread chapter

# Download menu item: a volume's chapters not yet downloaded.
# @arg volume: text
# @arg n: int
download-volume-count = { $volume } · { $n ->
    [one] 1 chapter
   *[other] { $n } chapters
 }

# Download menu item: turns on choosing chapters in the list.
download-choose = Choose chapters…

# Download menu item.
download-delete-series = Delete downloads of this series

# The amber button in the Choose chapters bar.
# @arg n: int
download-chapters = Download { $n ->
    [one] 1 chapter
   *[other] { $n } chapters
 }

# Line above that button.
download-over-wifi = Over Wi-Fi

## Read again

# Title of the question before reading a series again.
# @arg title: text
read-again-title = Read { $title } again from the start?

# What happens on each side.
read-again-line = Your place goes back to chapter 1 on all your { -device-pl }. Chapters stay marked read on your { -tracker-pl }, and AniList counts a reread.

# The confirming button.
read-again-confirm = Read again

## Read from

# Title of the sheet that changes where a series' chapters come from.
series-read-from-title = Read from

# While the sheet asks every source.
series-read-from-asking = Asking your { -source-pl }…

# The sheet with no source that has the series.
series-read-from-none-title = No { -source } has this series

# Line under it.
series-read-from-none-line = Add a folder, a server or a { -plugin-store } in Your { -source-pl }.

# A source's chapters, first on its line.
# @arg latest: text
series-read-from-chapters = Chapters 1 to { $latest }

# A source's chapters, after its kind on the line.
# @arg latest: text
series-read-from-chapters-later = chapters 1 to { $latest }

# After a folder's chapters.
# @arg device: text
series-read-from-on-device = on this { $device }

# Footnote of the Read from sheet.
series-read-from-note = Read marks belong to the series, so they carry over when you change. Downloads from the old { -source } stay until you delete them.

## Tracking

# Heading for trackers that don't track this series.
series-not-tracked = Not tracked here

# A tracker that isn't connected.
series-tracker-connect = Connect in Settings to track this series

# A tracker that has no such series.
series-tracker-not-there = No match on this { -tracker }

# A tracker looking for the series.
series-tracker-looking = Looking for this series…

# A tracker that is behind, and catches up on the next sync.
# @arg has: plain
# @arg gets: plain
series-tracker-behind = Has chapter { $has }. Gets { $gets } on the next sync.

# A tracker ahead of Kaeri.
# @arg has: plain
series-tracker-ahead = Has chapter { $has }, further than { -app-name }.

# A tracker's score out of ten.
# @arg score: text
series-tracker-score = { $score } / 10

# Menu item on a tracker that differs.
series-tracker-keep-kaeri = Keep { -app-name }'s place

# Menu item on a tracker that differs.
# @arg tracker: text
series-tracker-take-theirs = Use { $tracker }'s place

# Under a tracker's guesses for the series.
series-tracker-none-of-these = None of these

## Reading lists

# AniList's list.
read-list-reading = Reading

# AniList's list.
read-list-planning = Planning

# AniList's list.
read-list-completed = Completed

# AniList's list.
read-list-paused = Paused

# AniList's list.
read-list-dropped = Dropped

# AniList's list.
read-list-rereading = Rereading

# Series in folders and servers that are on no list.
read-list-your-files = Your files

## What Kaeri is for

# A choice.
medium-watch = Watch

# A choice.
medium-read = Read

# A choice.
medium-both = Both

# Line under the Watch choice.
medium-watch-line = Shows and films from your { -source-pl }

# Line under the Read choice.
medium-read-line = Manga, webtoons and light novels from your { -source-pl }

# Line under the Both choice.
medium-both-line = Watching and reading, each in its own place

# The answer, as the Settings row says it.
medium-answer-watch = Watching

# The answer, as the Settings row says it.
medium-answer-read = Reading

# The answer, as the Settings row says it.
medium-answer-both = Watching and reading

## Sources for books

# Title of the sheet and the toolbar button.
sources-add-title = Add a { -source }

# Line in the Add a source sheet.
book-add-line = Books you already have, or { -plugin-pl } from a { -store } whose owner invited you.

# A kind in the Add a source sheet.
book-add-folder = A folder

# Line under it.
# @arg device: text
book-add-folder-line = CBZ, EPUB, PDF and folders of images on this { $device } or a drive

# A kind in the Add a source sheet.
book-add-server = A server

# Line under it.
book-add-server-line = Komga, Kavita or an OPDS catalogue you can sign in to

# A kind in the Add a source sheet.
book-add-store = A { -plugin-store }

# Line under it.
book-add-store-line = Paste an invite from the { -store }'s owner

# Title of the Add a server sheet.
book-add-server-title = Add a server

# The sheet's amber button.
book-add-server-button = Add server

# What the address answered, before the sign-in.
# @arg kind: text
sources-found-server = Found a { $kind } server

# What the server held once signed in, when it said how many series it lists.
# @arg kind: text
# @arg n: int
sources-found-server-series = { $n ->
    [one] Found a { $kind } server with 1 series
   *[other] Found a { $kind } server with { $n } series
 }

# While the address is tried.
book-server-looking = Looking at this address…

# Line under the password.
book-server-note = { -app-name } sends your reading progress back to this server. The password stays in the Keychain.

# Heading in Your sources.
book-sources-folders = Folders

# Heading in Your sources.
book-sources-servers = Servers

# Heading in Your sources.
book-sources-plugins = { -plugin-cap-pl }

# Under the plugins, before a link to the plugin stores.
book-sources-plugins-footer = { -plugin-cap-pl } come from { -plugin-store-pl } you join.

# How many series a folder or server holds.
# @arg n: int
book-sources-series = { $n ->
    [one] 1 series
   *[other] { $n } series
 }

# A folder being looked through.
# @arg n: int
book-sources-scanning = Looking through · { $n } found

# A folder that can't be found, such as a drive that isn't plugged in.
book-sources-missing = Not found. Is the drive connected?

# A source that is turned off.
book-sources-off = Off

# On a server's line.
book-sources-progress-back = progress goes back to the server

# A server's kind.
book-sources-opds = OPDS catalogue

# A plugin's store.
# @arg store: text
book-sources-from-store = From { $store }

# A plugin's version.
# @arg version: text
book-sources-version = version { $version }

# Button on a folder: series whose match needs a look.
# @arg n: int
book-sources-check-matches = Check matches · { $n }

# Title of the Check matches sheet.
book-sources-check-matches-title = Check matches

# A folder menu item: look through the folders again.
book-sources-scan = Look again

# A server menu switch.
book-sources-report-progress = Send progress to the server

# A server menu switch.
book-sources-use = Use this server

# An address that answered but serves no books.
book-sources-not-a-book-server = This address answers, but not as a book server.

# A folder name with no match.
book-sources-no-match = No match

# Button: keep the series as Your files, unmatched.
book-sources-keep = Keep unmatched

# Button: search for the match by hand.
book-sources-find = Find

# Footnote of the Check matches sheet.
book-sources-unmatched-note = You can still read unmatched series from the folder. They get no { -tracker } progress until they're matched.

## The reading library

# Accessibility label of the control that sets Shows or Reading in the Library and Search.
library-scope-label = Shows or Reading

# A side of that control.
library-scope-shows = Shows

# A side of that control.
library-scope-reading = Reading

# The reading library's order: as the engine sends it, most recently read first.
read-sort-last-read = Last read

# Accessibility value of the list menu: the list and how many series are on it.
# @arg list: text
# @arg n: int
read-library-list-value = { $list }, { $n ->
    [one] 1 series
   *[other] { $n } series
 }

# A reading list with nothing on it. A Search button follows.
# @arg list: text
read-library-empty = Nothing on { $list } yet

# Line under it.
read-library-empty-line = Series you add from Search or a series page show here.

# Your files with nothing in it.
read-library-files-empty-title = Nothing in your files yet

# Line under it.
read-library-files-empty-line = Series in your folders and servers that are on no list show here.

# The button under it.
read-library-files-add = Add a folder or server

## Reading's Search

# The search field's prompt while Search looks for reading.
read-search-placeholder = Search reading

# The last row of reading's results with both media on: matches in the other scope.
# @arg n: int
# @arg scope: text
search-other-scope = { $n ->
    [one] 1 match
   *[other] { $n } matches
 } in { $scope }

# The button on that row, which switches the scope.
search-show-them = Show them

# The line over browsing: what is shown, and where the list comes from.
# @arg what: text
read-search-browse-line = { $what } · from AniList

# Over reading's results once every result is listed: how many series AniList found.
# @arg n: int
read-search-total = { $n ->
    [one] 1 series from AniList
   *[other] { $n } series from AniList
 }

# Search couldn't ask AniList.
read-search-failed = Couldn't search

# No results for what was typed.
# @arg query: text
read-search-nothing = Nothing found for { $query }

# No results while browsing with filters.
read-search-nothing-browse = Nothing matches these filters

# Line under no results.
read-search-nothing-line = Try other words, or a title in another language.

# Line under no results with filters set. A Clear filters button follows.
read-search-nothing-filters-line = Fewer filters may find more.

# The button under it.
read-search-clear-filters = Clear filters

# A sort of reading's Search, AniList's.
read-sort-popular = Popular

# A sort of reading's Search, AniList's.
read-sort-trending = Trending

# A sort of reading's Search, AniList's.
read-sort-score = Score

# A sort of reading's Search, AniList's.
read-sort-newest = Newest

# A filter.
read-filter-format = Format

# A filter.
read-filter-origin = Origin

# A filter.
read-filter-status = Status

# A filter.
read-filter-genres = Genres

# Title of the iPhone's filters sheet.
read-filter-title = Filters

# Button in that sheet that clears every filter.
read-filter-reset = Reset

# Accessibility value of a filter with nothing chosen.
read-filter-any = Any

# A filter's name before its value, once set: "Origin: Korea".
# @arg name: text
read-filter-set = { $name }:

# A filter with several values: the first, then how many more.
# @arg first: text
# @arg more: int
read-filter-many = { $first } and { $more } more

# A genre left out.
# @arg genre: text
read-filter-not-genre = Not { $genre }

# How many genres are set.
# @arg n: int
read-filter-genres-count = { $n ->
    [one] 1 genre
   *[other] { $n } genres
 }

# A genre's state: only series with it.
read-filter-included = Included

# A genre's state: no series with it.
read-filter-left-out = Left out

# A filter of reading's Search: when a series started.
read-search-started = Started

# The filter bar's chip that searches only the person's own sources, leaving AniList out.
# @max 24
read-search-sources-only = Your { -source-pl } only

# Search's section heading under AniList's results: one group per source of the person's.
read-search-from-sources = From your { -source-pl }

# E-ink and the terminal: a source group still waiting for its answer. Also its screen reader
# label. The source is its name.
# @arg source: text
read-search-asking = Asking { $source }…

# A source group whose source failed to answer. The reason and Try again follow.
# @arg source: text
read-search-source-failed = { $source } didn't answer.

# The reason under it when the source gave no answer in time.
# @arg seconds: int
read-search-source-timeout = No answer in { $seconds } s.

# One line at the end of the sources' groups for every source that found nothing. The sources
# are their names, joined as a list.
# @arg sources: text
read-search-source-nothing = Nothing found in { $sources }.

# On an AniList result that one of the person's sources has. The source is its name.
# @arg source: text
read-search-in-source = In { $source }

# On an AniList result that more than one of the person's sources has. Always 2 or more.
# @arg n: int
read-search-in-sources = { $n ->
    [one] In { $n } of your { -source-pl }
   *[other] In { $n } of your { -source-pl }
 }

# Search, when AniList found nothing but the person's sources did.
read-search-anilist-nothing = AniList found nothing for these words.

# Search, when AniList could not be reached.
read-search-anilist-failed = AniList didn't answer.

# The line under it.
read-search-anilist-failed-line = Your { -source-pl } were still asked.

# Search, when nothing was found anywhere: under the line saying AniList found nothing.
read-search-sources-nothing = Nothing in your { -source-pl } either.

# Over reading's results while more pages are to come, so AniList's count is not known yet.
read-search-total-unknown = Series from AniList

# The link under From your sources that shows the rest of AniList's results.
read-search-more-anilist = More from AniList

# Beside More from AniList: how many of AniList's results show.
# @arg shown: int
# @arg total: int
read-search-shown = { $shown } of { $total } shown

# Beside More from AniList while AniList's count is not known yet: how many results show.
# @arg shown: int
read-search-shown-some = { $shown } shown

# Beside a source's name in Search: the kind of source it is.
read-search-source-kind-server = Server
read-search-source-kind-folder = Folder
read-search-source-kind-plugin = { -plugin-cap }

# The button of a source group with more than five hits.
# @arg n: int
read-search-source-more = Show { $n } more

## First run with reading on

# Over a first-run page's title: which page of how many.
# @arg step: int
# @arg total: int
onboarding-step = { $step } of { $total }

# Line under "Welcome to Kaeri" on the first page, naming both media.
onboarding-welcome-line = Keep track of what you watch and read, from your own { -source-pl }.

# Note under the welcome line.
onboarding-welcome-note = Your lists and progress stay on your { -device-pl }. { -tracker-cap-pl } and { -source-pl } are yours to add.

# Title of the page that asks what Kaeri is for, and of the sheet after the update.
onboarding-medium-title = Watch, read, or both?

# Line under the title of the medium page.
onboarding-medium-line = { -app-name } shows only what you pick. You can change it in Settings.

# Accessibility hint on Continue while nothing is chosen on the medium page.
onboarding-medium-hint = Choose one of the three first

# Title of the first-run page that offers AniList.
onboarding-lists-title = Bring in your lists

# Line on the AniList page after Watch.
# @arg tracker: text
onboarding-lists-line-watch = { -app-name } keeps your watching in step with { $tracker }.

# Line on the AniList page after Read.
# @arg tracker: text
onboarding-lists-line-read = { -app-name } keeps your reading in step with { $tracker }.

# Line on the AniList page after Both.
# @arg tracker: text
onboarding-lists-line-both = { -app-name } keeps your reading and watching in step with { $tracker }.

# Line under AniList's name on the first-run page.
onboarding-lists-anilist-line = Lists, progress and suggestions

# Under AniList on the first-run page after Watch: the other trackers, for later.
onboarding-lists-later-watch = You can add MyAnimeList, Kitsu, Simkl and Trakt later in Settings, under Accounts.

# Under AniList on the first-run page after Read: the other trackers, for later.
onboarding-lists-later-read = You can add MyAnimeList, Kitsu and MangaUpdates later in Settings, under Accounts.

# Under AniList on the first-run page after Both: the other trackers, for later.
onboarding-lists-later-both = You can add MyAnimeList, Kitsu, MangaUpdates, Simkl and Trakt later in Settings, under Accounts.

# The first-run page's main button: sign in to the tracker.
# @arg tracker: text
onboarding-connect = Connect { $tracker }

# Title of the last first-run page after Read or Both.
onboarding-books-title = Where your books come from

# Line under the title of the books page.
onboarding-books-line = { -app-name } comes with no books. Add what you already have, or join a { -plugin-store } with an invite from its owner.

# Row on the books page: add a folder of books.
onboarding-books-folder = Add a folder

# Line under Add a folder on the books page.
onboarding-books-folder-line = CBZ, EPUB, PDF and folders of images

# Row on the books page: add a book server.
onboarding-books-server = Add a server

# Line under Add a server on the books page.
onboarding-books-server-line = Komga, Kavita or an OPDS catalogue

# Heading over the invite field on the books page.
onboarding-books-invite-heading = { -plugin-store-cap } invite

# Placeholder of the invite field on the books page.
onboarding-books-invite-placeholder = Paste an invite

# Line under the invite field: whose code a plugin is.
onboarding-books-plugin-line = { -plugin-cap-pl } run code from people you trust. Only join { -plugin-store-pl } whose owner you know.

# Title of the last first-run page after Both: where shows and books come from.
onboarding-both-title = Where your shows and books come from

# Line under the title of that page.
onboarding-both-line = { -app-name } comes with no shows or books. Add what you already have, or join a { -plugin-store } with an invite from its owner.

# Heading over the shows' rows on that page.
onboarding-both-shows = Shows

# Heading over the books' rows on that page.
onboarding-both-books = Books

# Line under Add a folder in the shows' part, on a computer.
# @arg device: text
onboarding-both-shows-folder-line = Video files on this { $device }

# Line under Add a server in the shows' part.
onboarding-both-shows-server-line = Your Jellyfin server

## Asking what Kaeri is for after the update and on joining

# Accessibility value of a choice row: its place among the three.
# @arg n: int
# @arg total: int
medium-choice-position = { $n } of { $total }

# Under the choices in the sheet.
medium-change-later = You can change it in Settings.

# Line of the sheet over Home after the update that brought reading.
# @arg device: text
medium-update-line = { -app-name } can now read manga, webtoons and light novels from your { -source-pl }. Pick what { -app-name } is for on this { $device }.

# Title of the sheet when the other devices answer Watch. The devices are their names, joined;
# n is how many there are.
# @arg devices: text
# @arg n: int
medium-join-title-watch = { $n ->
    [one] { $devices } is set to watching
   *[other] { $devices } are set to watching
}

# Title of the sheet when the other devices answer Read.
# @arg devices: text
# @arg n: int
medium-join-title-read = { $n ->
    [one] { $devices } is set to reading
   *[other] { $devices } are set to reading
}

# Title of the sheet when the other devices answer Both.
# @arg devices: text
# @arg n: int
medium-join-title-both = { $n ->
    [one] { $devices } is set to watching and reading
   *[other] { $devices } are set to watching and reading
}

# Line of the sheet on joining.
# @arg device: text
medium-join-line = Pick what { -app-name } is for on this { $device }. Each { -device } keeps its own answer.

# Title of the confirmation before watching is turned off.
medium-off-title-watch = Turn off watching?

# Title of the confirmation before reading is turned off.
medium-off-title-read = Turn off reading?

# What turning watching off does.
# @arg device: text
medium-off-watch-body = Calendar, Seasonal, { -watch-together-cap } and your shows leave { -app-name } on this { $device }. Your list, progress and downloads stay, and come back if you turn watching on again.

# What turning reading off does.
# @arg device: text
medium-off-read-body = Updates and your reading leave { -app-name } on this { $device }. Your list, progress and downloads stay, and come back if you turn reading on again.

# Switch in the confirmation, off by default.
medium-off-delete-watch = Delete downloaded episodes

# Switch in the confirmation, off by default.
medium-off-delete-read = Delete downloaded chapters

# Under the switch: how much the downloads take.
# @arg size: text
# @arg device: text
medium-off-delete-size = { $size } on this { $device }

# The confirmation's main button.
medium-off-confirm-watch = Turn off watching

# The confirmation's main button.
medium-off-confirm-read = Turn off reading

## Settings for reading

# Settings row and page: what Kaeri is for on this device.
settings-kaeri-is-for = { -app-name } is for

# Line under Kaeri is for on the Mac and iPad.
settings-medium-note = Each { -device } keeps its own answer

# Under the choices on the iPhone's page.
settings-medium-footer = Each { -device } keeps its own answer. Turning one off keeps its list, progress and downloads.

# Line under Open at.
settings-open-at-note = Which Home { -app-name } starts on

# Settings pane and group: reading.
settings-group-reading = Reading

# Downloads group with both media on: shows.
settings-group-shows = Shows

# The Reading pane's column beside it.
settings-aside-reading = Reader defaults are where every series starts. A series you change in the reader keeps its own choices.

# Group: how the reader opens each kind of book.
settings-reader-defaults = Reader defaults

# Under the reader defaults.
settings-reader-defaults-footer = A series you change keeps its own mode, direction and look.

# Row: manga's reader default.
settings-reader-manga = Manga

# Row: webtoons' reader default.
settings-reader-webtoons = Webtoons

# Line under Webtoons.
settings-reader-webtoons-note = Webtoons always open as a long strip

# Row: light novels' reader default.
settings-reader-novels = Light novels

# A reader default: pages turned right to left.
settings-reader-rtl = Pages, right to left

# A reader default: pages turned left to right.
settings-reader-ltr = Pages, left to right

# A reader default: one long strip.
settings-reader-strip = Long strip

# A reader default for novels: pages.
settings-reader-pages = Pages

# A reader default for novels: one scrolling column.
settings-reader-scrolled = Scrolling

# Row: when a chapter counts as read.
settings-counts-read = Counts as read

# Line under Counts as read.
settings-counts-read-note = A short stay on the last page, or moving on to the next chapter

# Value of Counts as read: a short stay on the last page, or moving on past it.
settings-counts-read-value = After the last page

# Value of Counts as read: only moving on past the last page, to the end or the next chapter.
settings-counts-read-moving-on = Moving on from it

# Line under Counts as read when it is Moving on from it.
settings-counts-read-moving-on-note = Turning past the last page, or opening the next chapter

# Value of Counts as read: only the person's own marks; the reader marks nothing.
settings-counts-read-marked = When you mark it

# Line under Counts as read when it is When you mark it.
settings-counts-read-marked-note = { -continue-reading-cap } and your { -tracker-pl } follow your marks

# Group: series settings that follow you to your other devices.
settings-follow-group = Series settings that follow you

# Row in that group.
settings-follow-mode = Mode, direction and colour

# Line under that row.
settings-follow-mode-note = A series you change reads the same on your other { -device-pl }

# Under the group: what stays on this device.
# @arg device: text
settings-follow-footer = Text size, margins, tap zones and refresh stay on this { $device }.

# Line under Look in Series settings that follow you.
settings-look-follows-note = Colour or greys

# Value of New chapters in Downloads: the series on the Reading list.
settings-new-chapters-reading = Series on Reading

# Line under New chapters in Downloads.
settings-new-chapters-note = A series being read also keeps its next chapters ready

# Group in Accounts: how reading progress goes to the trackers.
settings-progress-group = Progress

# Value of Send progress after: as each chapter counts as read.
settings-progress-chapter = A chapter is read

# Value of Send progress after: once, when the reader closes.
settings-progress-close = Closing the reader

# Value of When trackers disagree: the higher chapter wins.
settings-disagree-furthest = Keep the furthest

# Value of When trackers disagree: the newest change wins, even a lower chapter.
settings-disagree-latest = The latest change

# Line under When trackers disagree.
settings-disagree-note = Status and score follow the latest change

# Line under Keep next chapters ready.
settings-keep-chapters-ready-note = The next chapters of series on your Reading list

# Line under Delete chapters after reading.
settings-delete-after-reading-note = Bookmarked chapters are kept

# Line under New chapters.
settings-chapter-notifications-note = Series on your Reading list

# Line under Downloads finished.
settings-downloads-finished-note = When the chapters downloading are all done

# Under New chapters.
settings-chapter-notifications-footer = Chapters have their own system channel, so you can silence them and keep the rest.

# Credit under the trackers with reading on.
accounts-mangaupdates-credit = { -app-name } talks to MangaUpdates through its public API.

# Placeholder of the user name field when signing in to MangaUpdates.
accounts-username = User name

## Downloads with reading on

# Beside the Reading heading in Downloads: how many chapters are kept and their size.
# @arg n: int
# @arg size: text
read-downloads-total = { $n ->
    [one] { $n } chapter · { $size }
   *[other] { $n } chapters · { $size }
}

# A series' line in Downloads while a chapter downloads.
# @arg chapter: text
# @arg done: int
# @arg total: int
read-downloads-active = { $chapter } · { $done } of { $total } pages

# After the chapter downloading: how many more wait.
# @arg n: int
read-downloads-more-queued = { $n } more queued

# A series' line in Downloads when its chapters wait to start.
# @arg n: int
read-downloads-queued = { $n ->
    [one] { $n } chapter queued
   *[other] { $n } chapters queued
}

# A series' line in Downloads when a chapter failed, with the reason.
# @arg chapter: text
# @arg reason: text
read-downloads-failed = { $chapter } couldn't download. { $reason }

# A series' line in Downloads: how many chapters are kept.
# @arg n: int
read-downloads-kept = { $n ->
    [one] { $n } chapter
   *[other] { $n } chapters
}

# After the kept count: how many of them are read.
# @arg n: int
read-downloads-read = { $n } read

# Button on a series in Downloads: delete its read chapters, keeping bookmarked ones.
read-downloads-delete-read = Delete read

## Send to e-reader

# Title of the Send to e-reader sheet, the chapter menu's item and the Settings row.
send-title = Send to e-reader

# Heading over the e-readers to choose from in the Send to e-reader sheet.
send-for = For

# Heading over what is being sent in the Send to e-reader sheet.
send-what = What

# Row in the Send to e-reader sheet: the chapters being sent.
send-chapters = Chapters

# The chapters being sent when there is one: its number or label.
# @arg chapter: text
send-chapters-one = { $chapter }

# The chapters being sent when there are several, from the first to the last.
# @arg first: text
# @arg last: text
send-chapters-range = { $first } to { $last }

# Row in the Send to e-reader sheet: how big the files will be.
send-size = Size

# The size of what is being sent, already sized (such as "64 MB"). It is a guess until the files are made.
# @arg size: text
send-size-about = About { $size }

# Under the e-readers in the Send to e-reader sheet: what Kaeri cannot know after the files are sent.
send-no-place = { -app-name } can't see your place on this e-reader. When you come back, the series page asks where you stopped.

# The one main button of the Send to e-reader sheet. It makes the files and then opens the share sheet.
send-prepare = Prepare and share

# While the files are made: which chapter of how many. The ring beside it shows how far along.
# @arg done: int
# @arg total: int
send-preparing = { $total ->
    [one] Preparing the chapter
   *[other] Preparing { $done } of { $total } chapters
}

# Under the progress of a send to a device: what the pages are made for.
# @arg device: text
send-sized-for = Pages are sized for { $device }

# Under the size in the Send sheet when the files are over what the e-reader takes in one file.
# @arg size: text
send-in-parts = Over { $size }, so the files go in parts.

# A send that failed on one chapter. The cause is the sentence that says why.
# @arg chapter: text
# @arg cause: text
send-failed-chapter = Couldn't prepare chapter { $chapter }. { $cause }

# A send that failed before or after its chapters. The cause is the sentence that says why.
# @arg cause: text
send-failed = Couldn't prepare the files. { $cause }

# Text of the Send sheet's Size row while the size is still being worked out.
send-size-working = Working it out

# Spoken value of the Chapters row in the Send sheet while its list is open.
send-chapters-open = Open

# Spoken value of the Chapters row in the Send sheet while its list is closed.
send-chapters-closed = Closed

# Spoken value of the ring in a send under way. Already a number from 0 to 100.
# @arg percent: int
send-progress-spoken = { $percent } percent

# Spoken action on a chapter row of a series: sends that chapter to an e-reader.
send-chapter-action = Send to e-reader

# Under an e-reader in the sheet when it takes files in parts: the size where a file is split.
# @arg size: text
send-parts-over = in parts over { $size }

# While a send waits for the network and the person can stop it. The engine's own line comes first.
send-waiting-cancel = Cancel send

# When the files are ready, over the button that opens the share sheet. The count is how many files.
# @arg n: int
send-ready = { $n ->
    [one] One file is ready.
   *[other] { $n } files are ready.
}

# Button in the Send to e-reader sheet that opens the share sheet again, after it was closed without sharing.
send-share-again = Share again

# A send in a list or banner that is under way, with a ring beside it. The device is the e-reader.
# @arg device: text
send-banner = Sending to { $device }

# The format of the files an e-reader is sent: a manga as a fixed-layout EPUB.
send-format-fixed-epub = Fixed-layout EPUB

# The format of the files an e-reader is sent: the Kobo form of EPUB.
send-format-kepub = KEPUB

# The format of the files an e-reader is sent: a plain EPUB.
send-format-epub = EPUB

# The format of the files an e-reader is sent: a comic book archive.
send-format-cbz = CBZ

# Line under an e-reader in the picker: the format the files will have, and the screen they are made for in pixels.
# @arg format: text
# @arg width: text
# @arg height: text
send-target-line = { $format } · { $width } × { $height }

## Where did you stop

# Title of the sheet that asks where you stopped reading on the e-reader.
stop-title = Where did you stop?

# Note on a series page after chapters were sent: opens the question.
# @arg device: text
stop-note = You sent chapters to { $device }. Where did you stop?

# Button in that note.
stop-note-action = Set your place

# First line of the sheet when one chapter was sent. The date is already written out (such as "2 November").
# @arg chapter: text
# @arg device: text
# @arg date: text
stop-sent-one = Chapter { $chapter } went to { $device } on { $date }.

# First line of the sheet when several chapters were sent, from the first to the last.
# @arg first: text
# @arg last: text
# @arg device: text
# @arg date: text
stop-sent-range = Chapters { $first } to { $last } went to { $device } on { $date }.

# Switch in the sheet: the last chapter sent was read to its end.
# @arg chapter: text
stop-finished = Finished chapter { $chapter }

# The main button of the sheet: keeps the chapter chosen as the place.
stop-set = Set your place

# The other button of the sheet: leaves the place as it is, with the place named.
# @arg chapter: text
# @arg page: plain
stop-keep = Keep chapter { $chapter } · page { $page }

# The other button of the sheet when the place is not a page.
stop-keep-plain = Keep current place

## The e-readers settings and the feed

# Heading of the E-readers group in the Reading settings.
ereaders-group = E-readers

# Row in the E-readers group: the e-reader Send to e-reader offers first. The value is the e-reader's name.
ereaders-send-row = Send to e-reader

# Row in the E-readers group that opens the feed settings. The value is On or Off.
ereaders-feed-row = OPDS feed

# Line under the Share on this network switch.
opds-share-note = An OPDS catalogue e-readers at home can browse

# Footnote under the feed settings.
# @arg device: text
opds-footer = The feed runs only while { -app-name } is open on this { $device }. Progress on those e-readers stays there.

# Row in the feed settings: the web address e-readers are given, with a button that copies it.
opds-address = Address

# Shown in place of the address while the feed is off or has no password.
opds-address-off = Turn the feed on to get its address.

# Row in the feed settings: the password e-readers ask for.
opds-password = Password

# Line under the password row.
opds-password-note = E-readers ask for it once

# Button in the feed settings that makes a new password. The e-readers ask for the new one.
opds-new-password = New password

# Row in the feed settings: the port the feed listens on.
opds-port = Port

# Line under the port row when it is empty.
opds-port-note = Leave it empty for any free port, or pick one from 1024 to 65535

# Line under the port row when the port asked for was taken and another is used.
# @arg wanted: plain
# @arg port: plain
opds-port-moved = Port { $wanted } was busy, so the feed uses { $port }.

# Heading of the group that picks what the feed shares.
opds-shares = What it shares

# Row in What it shares: the chapters downloaded on this device.
opds-share-downloads = Downloads

# Line under that row.
# @arg n: int
opds-share-downloads-count = { $n ->
    [one] One chapter
   *[other] { $n } chapters
}

# Row in What it shares: the series in the manga and comic folders.
opds-share-manga = Manga folder

# Row in What it shares: the series in the novel folders.
opds-share-novels = Novel folder

# Line under a folder row.
# @arg n: int
opds-share-series-count = { $n ->
    [one] One series
   *[other] { $n } series
}

# Heading of the list of e-readers that fetched files from the feed.
opds-fetched-by = Fetched by

# Line when no e-reader has fetched anything since the feed started.
opds-fetched-none = No e-reader has fetched anything yet.

# Shown in place of the address while the feed is switched on and starting to listen.
opds-starting = Starting…

# Shown in place of the address while the feed is on but the Mac is on no local network.
opds-paused-none = Paused. This Mac isn't on a local network.

# Shown in place of the address while the feed is on but the only connection goes through a VPN.
opds-paused-vpn = Paused. This Mac is only connected through a VPN.

# Title of the question before a new feed password is made.
opds-new-password-title = Make a new password?

# Text of that question.
opds-new-password-warning = E-readers that already use the feed stop working until you type the new password into each one.

# Spoken value of the Fetched by list's rows when an e-reader's last request is given. The time is already written out.
# @arg time: text
opds-fetched-time = Last asked { $time }

# Line under an e-reader in that list: how many files it took.
# @arg n: int
opds-fetched-files = { $n ->
    [one] One file
   *[other] { $n } files
}

# Name for an e-reader that did not give one.
# @arg address: text
opds-reader-unnamed = E-reader at { $address }

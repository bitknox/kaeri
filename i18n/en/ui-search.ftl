## Seasonal

# Button that shows the season before.
seasonal-previous = Previous season

# Button that shows the season after.
# @allow up-next
seasonal-next = Next season

# Short line while a season loads.
seasonal-loading = Loading this season

# Title of the state when a season could not load.
seasonal-load-failed = Couldn't load this season

# Title of the state when a season lists no titles. The season is already worded.
# @arg season: text
seasonal-empty-title = Nothing listed for { $season } yet

# Line under the empty season title.
seasonal-empty-detail = AniList adds a season's titles as they're announced.

# Title of the state when the chosen format has no titles this season.
seasonal-empty-format = No titles in this format

# Line under the empty format title.
seasonal-empty-format-detail = Try all formats.

# Button that opens the season in Search, where filters narrow it.
seasonal-filter-in-search = Filter in Search

# Tooltip of the Filter in Search button.
seasonal-filter-help = This season in Search, where every filter narrows it

# Sort option on Seasonal and Search: most popular first.
sort-popularity = Popularity

# Sort option on Seasonal and Search: highest score first.
sort-score = Score

# Sort option on Seasonal: by the day of the week the show airs.
sort-air-day = Air day

# Sort option on Search: best match for the words typed.
sort-relevance = Relevance

# Button showing the current sort order.
# @arg sort: text
sort-button = Sort: { $sort }

# Accessibility label of the sort button.
# @arg sort: text
sort-label = Sort by { $sort }

## Search

# Placeholder of the search field.
search-placeholder = Search titles

# Accessibility label of the button that empties the search field.
search-clear = Clear search

# Row under the field while a search runs. The query is what the person typed.
# @arg query: text
search-searching = Searching for “{ $query }”

# Heading over the results of a search.
# @arg query: text
search-results-for = Results for “{ $query }”

# Heading over the titles shown when nothing was typed.
search-browse = Browse

# Title of the state when a search finds nothing.
# @arg query: text
search-nothing-found = Nothing found for “{ $query }”

# Line under the title when a search finds nothing.
search-nothing-found-detail = Check the spelling, or try the English or Japanese title.

# Title of the state when filters leave nothing.
search-nothing-with-filters = Nothing found with these filters

# Title of the state when a search and its filters leave nothing.
# @arg query: text
search-nothing-found-with-filters = Nothing found for “{ $query }” with these filters

# Line under a state that filters emptied.
search-try-fewer-filters = Try fewer filters.

# Button that removes every filter.
search-clear-filters = Clear filters

# Title of the state when a search failed.
search-failed = Couldn't search

# Title of the state when search needs a connection and there is none.
search-needs-connection = Search needs a connection

# Line when the search service does not answer.
search-unreachable = Search isn't reachable right now. Try again in a minute.

# Heading over the searches a person made lately.
search-recent = Recent searches

# Accessibility label of the button that forgets one recent search.
# @arg query: text
search-recent-remove = Remove { $query }

# Button that shows every title in a group.
search-see-all = See all

# Heading over the genre shortcuts.
search-genres = Genres

# Line while more results load at the end of a list.
search-loading-more = Loading more titles

## Filters

# Title of the filters sheet and panel.
filters-title = Filters

# Accessible label and tooltip of the button that closes the filters panel.
filters-close = Close filters

# Button that opens the filters, with how many are on.
# @arg count: int
filters-button-count = All filters · { $count }

# Button that opens the filters when none is on.
filters-button = All filters

# Filter name.
filter-genre = Genre

# Filter name, and the heading over the genre choices.
filter-genres = Genres

# Filter name.
filter-year = Year

# Filter name.
filter-season = Season

# Filter name.
filter-format = Format

# Filter name.
filter-status = Status

# Filter name: how long a show is, in episodes.
filter-length = Length

# Filter name: the number of episodes.
filter-episodes = Episodes

# Filter name: the length of one episode.
filter-episode-length = Episode length

# Filter group: how well a show was received.
filter-reception = Reception

# Filter name: a show's minimum score.
filter-score-at-least = Score at least

# Filter name: hides titles few people rated.
filter-hide-obscure = Hide obscure titles

# Filter group: where a show comes from.
filter-origin = Origin

# Filter name: the country a show was made in.
filter-country = Country

# Filter name: when a show came out.
filter-released = Released

# Filter group: where a show can be watched.
filter-where-to-watch = Where to watch

# Filter name: only shows with a legal licence.
filter-licensed-only = Licensed only

# Filter name: how titles already on the person's list are treated.
filter-my-list = Titles on my list

# Value of a three-way filter: include the titles.
filter-show = Show

# Value of a three-way filter: leave the titles out.
filter-hide = Hide

# Value of a three-way filter: only the titles.
filter-only = Only

# Note under the filters that tells results change as the person picks.
filter-live-note = Results update as you change these.

# Filter group: tags.
filter-tags = Tags

# Placeholder of the field that finds a tag.
filter-tag-find = Find a tag

# Line when no tag matches what was typed.
filter-tag-none = No tag has that in its name

# Filter name: how closely a tag must fit a show.
# @arg rank: text
filter-tag-rank = Tag rank at least { $rank }

# Switch that shows tags that give away the story.
filter-spoiler-tags = Show spoiler tags

# Choice meaning no genre filter.
filter-any-genre = Any genre

# Choice meaning no year filter.
filter-any-year = Any year

# Choice meaning no season filter.
filter-any-season = Any season

# Choice meaning no format filter.
filter-any-format = Any format

# Choice meaning no status filter.
filter-any-status = Any status

# Choice meaning every format on Seasonal.
filter-all-formats = All formats

# Note on the season filter while no year is chosen.
filter-season-pick-year = Choose a year first

# Choice in the year range: up to a year.
# @arg year: text
filter-years-up-to = Up to { $year }

# State of a genre or tag in a filter: it must be there.
filter-included = Included

# State of a genre or tag in a filter: it must not be there.
filter-left-out = Left out

# Hint on a genre or tag on a touch screen.
filter-hint-touch = Tap once to include, twice to leave out.

# Hint on a genre or tag on the TV, where a remote presses. Only the Android TV app shows it.
filter-hint-remote = Press once to include, twice to leave out.

# Accessibility label of the button that removes one filter.
# @arg filter: text
filter-remove = Remove { $filter }

# Search: sort by how many people marked the title a favourite.
# @max 20
search-sort-favourites = Favourites

# Search: sort by release date, newest first.
# @max 20
search-sort-newest = Newest

# Search: sort by release date, oldest first.
# @max 20
search-sort-oldest = Oldest

# Search: sort by title, A to Z.
# @max 20
search-sort-title = Title

# Search: sort by what is popular right now.
# @max 20
search-sort-trending = Trending

# A search result's line when the viewer's own sources have the title.
search-line-plays = Plays here

# A search result's line when the title is in the viewer's library of files.
search-line-library = In your { -library }

# A search result's line when one of the viewer's media servers has the title.
# @arg server: text
# @max 32
search-line-server = On { $server }

# Label of Search's filter for where a title can be watched.
search-available = Available

# Choice of that filter: no limit.
search-available-all = All

# Choice of that filter: titles that play from this device's own sources.
search-available-here = Plays here

# Choice of that filter: titles on the licensed services the person chose.
search-available-services = On your services

# End of a search result's line: one of the person's licensed services has it.
# @arg service: text
search-line-service = On { $service }

## Counts, the dub, trending periods and picks

# The count line under a title in a list or on a card: how many episodes a finished show has.
# @arg count: int
# @max 24
count-episodes = { $count ->
    [one] One episode
   *[other] { $count } episodes
 }

# The same, short, for a cell too narrow for the long form.
# @arg count: int
# @max 12
count-episodes-short = { $count } ep

# The count line of a show that is airing and has a known total: episodes out so far, then the total.
# @arg out: int
# @arg total: int
# @max 24
count-out-of = { $out } of { $total } out

# The same, short.
# @arg out: int
# @arg total: int
# @max 16
count-out-of-short = { $out } of { $total }

# The count line of a show that is airing and has no known total: episodes out so far.
# @arg count: int
# @max 24
count-out = { $count ->
    [one] One episode out
   *[other] { $count } episodes out
 }

# The same, short.
# @arg count: int
# @max 12
count-out-short = { $count } out

# The count line of a show that has not started and has a date. The date is the day and short month, already formatted from the months below.
# @arg date: text
# @max 24
count-starts = Starts { $date }

# The count line of a show that has not started and has no date.
# @max 24
count-not-out = Not out yet

# The count line of a show the viewer is part way through, in the library: the next episode and the total.
# @arg episode: int
# @arg total: int
# @max 24
count-next-of = Ep { $episode } of { $total }

# The dub part of a count line when a source has a dub and does not say how far it goes.
# @max 12
count-dub = Dub

# The dub part of a count line: the last episode the dub has.
# @arg through: int
# @max 12
count-dub-through = Dub { $through }

# What a screen reader says for a count line: the long form of a finished show.
# @arg count: int
count-spoken-episodes = { $count ->
    [one] One episode
   *[other] { $count } episodes
 }

# What a screen reader says for a show that is airing, with a known total.
# @arg out: int
# @arg total: int
count-spoken-out-of = { $out } of { $total } episodes out

# What a screen reader says for a show that is airing with no known total.
# @arg count: int
count-spoken-out = { $count ->
    [one] One episode out
   *[other] { $count } episodes out
 }

# What a screen reader says for a show not out yet, with a date. The date is the day and the full month name.
# @arg date: text
count-spoken-starts = Starts { $date }

# What a screen reader says for a show the viewer is part way through.
# @arg episode: int
# @arg total: int
count-spoken-next-of = Episode { $episode } of { $total }


# What a screen reader says after the count when the dub goes to an episode.
# @arg through: int
count-spoken-dub-through = { -dubbed } to episode { $through }

# What a screen reader says after the count when a source has a dub and does not say how far it goes.
count-spoken-dub = has a dub

# A month's short name, for "Starts 9 Jan".
date-month-jan-short = Jan

# A month's full name, for a screen reader's "Starts 9 January".
date-month-jan-long = January

# A month's short name, for "Starts 9 Jan".
date-month-feb-short = Feb

# A month's full name, for a screen reader's "Starts 9 January".
date-month-feb-long = February

# A month's short name, for "Starts 9 Jan".
date-month-mar-short = Mar

# A month's full name, for a screen reader's "Starts 9 January".
date-month-mar-long = March

# A month's short name, for "Starts 9 Jan".
date-month-apr-short = Apr

# A month's full name, for a screen reader's "Starts 9 January".
date-month-apr-long = April

# A month's short name, for "Starts 9 Jan".
date-month-may-short = May

# A month's full name, for a screen reader's "Starts 9 January".
date-month-may-long = May

# A month's short name, for "Starts 9 Jan".
date-month-jun-short = Jun

# A month's full name, for a screen reader's "Starts 9 January".
date-month-jun-long = June

# A month's short name, for "Starts 9 Jan".
date-month-jul-short = Jul

# A month's full name, for a screen reader's "Starts 9 January".
date-month-jul-long = July

# A month's short name, for "Starts 9 Jan".
date-month-aug-short = Aug

# A month's full name, for a screen reader's "Starts 9 January".
date-month-aug-long = August

# A month's short name, for "Starts 9 Jan".
date-month-sep-short = Sep

# A month's full name, for a screen reader's "Starts 9 January".
date-month-sep-long = September

# A month's short name, for "Starts 9 Jan".
date-month-oct-short = Oct

# A month's full name, for a screen reader's "Starts 9 January".
date-month-oct-long = October

# A month's short name, for "Starts 9 Jan".
date-month-nov-short = Nov

# A month's full name, for a screen reader's "Starts 9 January".
date-month-nov-long = November

# A month's short name, for "Starts 9 Jan".
date-month-dec-short = Dec

# A month's full name, for a screen reader's "Starts 9 January".
date-month-dec-long = December

# Label of Search's filter that keeps to the titles one of the viewer's sources lists in a dub.
# @max 16
search-dubbed = { -dubbed-cap }

# Help text of that filter, for a screen reader.
search-dubbed-hint = Shows results your { -source-pl } list in a dub.

# Under Search's filters when the dub filter covered only part of the results. The count is how many results the { -source-pl } were asked about.
# @arg count: int
dub-coverage-first = From the first { $count } results.

# Under Search's filters when some { -source-pl } did not answer about dubs.
# @arg answered: int
# @arg asked: int
dub-coverage-sources = { $answered } of { $asked } { -source-pl } answered.

# Under Search's filters when the dub filter used only what this device already knows: offline, or a browse with no words typed.
dub-coverage-stored = { -dubbed-cap } uses what this { -device } already knows.

# Title of the state when the dub filter is on and none of the results has a dub.
dub-none-title = No { -dubbed } results

# Line under that title. The count is how many results the { -source-pl } were asked about.
# @arg count: int
dub-none-detail = Your { -source-pl } didn't list a dub for the first { $count } results.

# Button of that state: turns the dub filter off.
dub-none-show-all = Show all results

# Title of the state when no source answered about dubs in time.
dub-failed-title = Couldn't ask about dubs

# Line under that title.
dub-failed-detail = None of your { -source-pl } answered in time.

# Search's Trending heading and sort, for AniList's live trending.
# @max 24
trending-today = Trending today

# Search's Trending heading and sort, for the last seven days.
# @max 24
trending-week = Trending this week

# Search's Trending heading and sort, for the last thirty days.
# @max 26
trending-month = Trending this month

# One choice of the Trending period menu, where the heading already says Trending.
# @max 16
trending-choice-today = Today

# One choice of the Trending period menu: the last seven days.
# @max 16
trending-choice-week = This week

# One choice of the Trending period menu: the last thirty days.
# @max 16
trending-choice-month = This month

# Shown in the Trending rail when a period could not load. The period is already worded, such as "this week".
# @arg period: text
trending-failed = Couldn't load { $period }'s trending.

# Line under a period that was kept from earlier, with how long ago. The age is already worded, such as "yesterday".
# @arg age: text
trending-updated = Updated { $age }

# Button that picks a random title from the screen it is on: the filters in Search, the Planning list in the library.
# @max 20
surprise-me = Surprise me

# The same as a menu bar item, in title case.
# @max 20
surprise-me-menu = Surprise Me

# Button on a picked title's page that swaps it for another pick. It takes the place of Surprise me for that visit.
# @max 16
surprise-another = Another

# Hint of the Surprise me button on the library's Planning list.
surprise-hint-planning = Opens a random title from Planning

# Hint of the Surprise me button in Search.
surprise-hint-filters = Opens a random title from these filters

# Hint of the Another button.
surprise-another-hint = Opens another random title in place of this one

# Notice when Search's filters match no title to pick from.
surprise-empty-filters = No titles match these filters.

# Notice when the Planning list is empty.
surprise-empty-planning = Planning is empty.

# Notice when a pick from Search needs AniList and the device is offline.
surprise-offline = Offline. Search picks need AniList.

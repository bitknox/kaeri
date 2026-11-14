# Messages for Home: the line over the spotlight's title, the section headings and the line under
# each card. The engine picks which one applies; each app draws it.

# Line over the spotlight's title when it is a show the person is part way through.
# @max 32
home-spotlight-continuing = { -continue-cap } watching

# Line over the spotlight's title when it is a new season of a show the person finished. The title is the season they finished.
# @arg title: text
# @max 60
home-spotlight-new-season = New season · You finished { $title }

# Line over the spotlight's title when it is on the person's Planning list.
# @max 32
home-spotlight-planning = On your Planning list

# Line over the spotlight's title when people who liked a show the person finished rated it highly. The title is that show; only it truncates.
# @arg title: text
# @max 60
home-spotlight-because = Because you finished { $title }

# Line over the spotlight's title when it is recommended from what the person watched, and the show it is like is not known.
# @max 32
home-spotlight-recommended = Recommended for you

# Line over the spotlight's title when nothing of the person's own is waiting, so it is this season's most popular show they have not started.
# @max 32
home-spotlight-popular = Popular this season

# Line under the spotlight's title, and under a { -continue-cap } card, for an episode part way through: the episode and the minutes left in it.
# @arg episode: plain
# @arg minutes: int
# @max 40
home-time-left = Episode { $episode } · { $minutes } min left

# Line under the spotlight's title, and under a { -continue-cap } card, for a show still airing whose next episode is its newest one out.
# @arg episode: plain
# @max 32
home-episode-out = Episode { $episode } is out

# Line under the spotlight's title, and under a { -continue-cap } card, for the next episode of a show part way through when it is not the newest one out, such as any episode of a show that finished airing.
# @arg episode: plain
# @max 32
home-episode-next = Episode { $episode } next

# Line under a { -continue-cap } card whose next episode came out today.
# @arg episode: plain
# @max 32
home-episode-out-today = Episode { $episode } · out today

# Line under a poster on This season: the newest episode out.
# @arg episode: plain
# @max 28
home-episode-out-short = Episode { $episode } out

# Line under a This week or Airing today card whose episode is already out today. The time is worded as a span, such as 2h 5m.
# @arg ago: seconds
# @max 24
home-out-ago = Out { $ago } ago

# Short reason under a poster: a new season of a show the person finished.
# @max 28
home-reason-new-season = New season

# Short reason under a poster: the title is on the person's Planning list.
# @max 28
home-reason-planning = On your Planning list

# Short reason under a poster: recommended from what the person watched. The hover card gives the full reason.
# @max 28
home-reason-recommended = Recommended for you

# Heading over the shows the person is part way through.
# @max 24
home-continue = { -continue-cap }

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-winter = Starting in Winter

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-spring = Starting in Spring

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-summer = Starting in Summer

# Heading over next season's titles in the two weeks before it starts.
# @max 32
home-starting-autumn = Starting in Autumn

# Heading over this week's episodes of the shows the person follows. With no list and nothing watched, it heads the week's most popular broadcasts and premieres instead.
# @max 24
home-this-week = This week

# Heading over what airs today, for someone with nothing watched and no { -tracker } signed in.
# @max 24
home-airing-today = Airing today

# Heading over the suggestions made from what the person watched and their lists.
# @max 32
home-up-next = { -up-next-cap } for you

# Heading over this season's most popular titles.
# @max 24
home-this-season = This season

# Heading over the titles most watched and talked about on AniList right now, from any season, for someone with no list and nothing watched.
# @max 24
home-trending = Trending now

# Heading over recommendations from one show the person finished and rated well. The title is that show.
# @arg title: text
# @max 60
home-because = Because you finished { $title }

# Heading over popular titles of the genre the person finished most of, for a genre that has no
# heading of its own below. The genre is AniList's name as it writes it.
# @arg genre: text
# @max 40
home-more-genre = More { $genre }

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-action = More action

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-adventure = More adventure

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-comedy = More comedy

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-drama = More drama

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-ecchi = More ecchi

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-fantasy = More fantasy

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-horror = More horror

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-mahou-shoujo = More mahou shoujo

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-mecha = More mecha

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-music = More music

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-mystery = More mystery

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-psychological = More psychological

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-romance = More romance

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-sci-fi = More sci-fi

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-slice-of-life = More slice of life

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-sports = More sports

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-supernatural = More supernatural

# Heading over popular titles of a genre the person finished most of. Write the genre as it reads
# inside a sentence in this language.
# @max 40
home-more-genre-thriller = More thriller

# Line over the spotlight's title when it is the built-in demo.
# @max 32
home-spotlight-demo = Demo, free to share

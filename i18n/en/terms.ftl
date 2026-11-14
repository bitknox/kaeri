## The project's glossary as Fluent terms. A message never types one of
## these words: it says { -source } or { -source-cap-pl }, and the catalogue lint fails when it
## does. A translation of a term fixes the word everywhere. A form a sentence needs is a term of
## its own: -x-cap (starts a sentence), -x-pl (plural), -x-cap-pl. (Fluent allows a term's
## attributes only as selectors, so they cannot carry the forms.)

# The product name. It is never translated.
-app-name = Kaeri

# A place Kaeri plays episodes from: a folder, a media server, a catalogue or a plugin. Never "provider".
-source = source
-source-cap = Source
-source-pl = sources
-source-cap-pl = Sources

# A WebAssembly add-on that supplies a source.
-plugin = plugin
-plugin-cap = Plugin
-plugin-pl = plugins
-plugin-cap-pl = Plugins

# The invite-based list plugins are installed from.
-plugin-store = plugin store
-plugin-store-cap = Plugin store
-plugin-store-pl = plugin stores
-plugin-store-cap-pl = Plugin stores

# A plugin store on second mention, once the sentence has already said "plugin store". Short
# forms for tight places such as the terminal. Never the first mention.
-store = store
-store-cap = Store

# Shows you have saved, with status.
-library = library
-library-cap = Library

# AniList, Kitsu and similar accounts Kaeri updates.
-tracker = tracker
-tracker-cap = Tracker
-tracker-pl = trackers
-tracker-cap-pl = Trackers

# Watching in sync with other people. "Watch together" is only the name of the place and the
# action that start and join one; the thing itself is a watch party.
-watch-party = watch party
-watch-party-cap = Watch party
-watch-party-pl = watch parties
-watch-party-cap-pl = Watch parties

# The name of the place that starts and joins watch parties.
-watch-together-cap = Watch together

# One downloadable file in a listing. Not a place episodes come from.
-release = release
-release-cap = Release
-release-pl = releases
-release-cap-pl = Releases

# One release that holds many episodes.
-batch = batch
-batch-cap = Batch

# A phone, tablet, TV or computer running Kaeri.
-device = device
-device-cap = Device
-device-pl = devices
-device-cap-pl = Devices

# The button that resumes the show.
-continue = continue
-continue-cap = Continue

# The button that resumes a book, manga or webtoon: reading's own form of { -continue }. English says
# continue for both; a language whose { -continue } means "continue watching" says "keep reading" here.
-continue-reading = continue
-continue-reading-cap = Continue

# The button that goes on to the next step of a flow, such as setup. Not the button that resumes a
# show; that is { -continue }.
-proceed = continue
-proceed-cap = Continue

# The episode that plays after this one.
-up-next = up next
-up-next-cap = Up next

# The audio setting's two values.
-subbed = subbed
-subbed-cap = Subbed
-dubbed = dubbed
-dubbed-cap = Dubbed

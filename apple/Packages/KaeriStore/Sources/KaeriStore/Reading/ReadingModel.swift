// Reading's places as the engine sends them: Home, Updates, the lists, Search, a series and its
// chapters, the downloads and the book sources. The screens draw what is here and ask through
// these models; nothing is decided on this side.

import KaeriKit
import Foundation
import Observation

/// What Kaeri is for on this device, and the question still to ask about it.
@Observable @MainActor
public final class MediumModel {
    public private(set) var view: MediumView?
    /// The question the sheet answers, held from the moment it shows until it is answered, so a
    /// view that arrives meanwhile does not take the sheet away.
    public private(set) var asking: MediumAsk?
    @ObservationIgnored var send: ((Command) -> Void)?

    public init() {}

    /// The answer in force: Watch until the engine says otherwise.
    public var answer: KaeriIsFor { view?.answer ?? .watch }
    public var reads: Bool { answer == .read || answer == .both }
    public var watches: Bool { answer == .watch || answer == .both }
    public var both: Bool { answer == .both }
    /// Whether Kaeri is for shows at all in this build: reading available and not fixed.
    public var choosable: Bool { (view?.readingAvailable ?? false) && !(view?.fixed ?? true) }

    func apply(_ view: MediumView) {
        self.view = view
        if asking == nil, let ask = view.ask { asking = ask }
        if view.ask == nil { asking = nil }
        #if FIXTURES
        // Shots of one medium alone: -KaeriMedium read or watch, chosen as Settings would.
        if !launchChosen, let medium = Self.launchMedium(ProcessInfo.processInfo.arguments) {
            launchChosen = true
            if choosable, view.answer != medium { choose(medium) }
        }
        #endif
    }

    #if FIXTURES
    @ObservationIgnored private var launchChosen = false

    static func launchMedium(_ arguments: [String]) -> KaeriIsFor? {
        guard let index = arguments.firstIndex(of: "-KaeriMedium"), arguments.indices.contains(index + 1) else { return nil }
        switch arguments[index + 1] {
        case "read": return .read
        case "watch": return .watch
        case "both": return .both
        default: return nil
        }
    }
    #endif

    public func load() { send?(.medium(.load)) }

    /// Settings' choice. Turning a medium off may delete its downloads.
    public func choose(_ answer: KaeriIsFor, deleteDownloads: Bool = false) {
        send?(.medium(.choose(answer: answer, deleteDownloads: deleteDownloads)))
    }

    /// The first run's page, the sheet after the update, or the join sheet, answered.
    public func answer(_ ask: MediumAsk, with answer: KaeriIsFor) {
        asking = nil
        send?(.medium(.answer(ask: ask, answer: answer)))
    }

    #if FIXTURES
    /// For shots: the view as the engine would send it.
    public func showcase(_ view: MediumView) { apply(view) }
    #endif
}

/// A series' chapters as the engine last sent them.
public struct ReadChapters: Equatable, Sendable {
    public var edition: BookEditionRow?
    public var volumes: [VolumeGroup]
    public var failure: ReadingFailure?
}

/// Reading's search: the query on screen, its pages so far, and whether more can come.
public struct ReadSearchState: Equatable, Sendable {
    public var query: ReadSearchQuery
    public var tiles: [WorkTile] = []
    public var page: UInt32 = 0
    public var hasMore = false
    public var otherScope: UInt32?
    /// How many series AniList found in all, once the engine knows it for sure.
    public var total: UInt32?
    public var loading = false
    public var failure: ReadingFailure?
    /// How many results the first page brought: with words, those show above the sources' part
    /// and the rest under More from AniList.
    public var firstPage = 0
    /// The sources' part of a search with words: AniList's state beside it, each source's group
    /// in its own slot, and the AniList results a source hit joined. Nil while browsing.
    public var sources: ReadSourceSearch?

    /// Whether the search has words, and so asks the person's sources beside AniList.
    public var hasWords: Bool { !query.text.trimmingCharacters(in: .whitespaces).isEmpty }

    /// The sources that have an AniList result, by its work: "In Home library".
    public func joined(_ work: String) -> ReadSourceJoin? {
        sources?.joins.first { $0.work == work }
    }
}

@Observable @MainActor
public final class ReadingModel {
    public private(set) var home: ReadingHomeView?
    public private(set) var homeLoading = false
    public private(set) var updates: UpdatesView?
    public private(set) var lists: [LibrarySegmentRead: [WorkTile]] = [:]
    public private(set) var listsLoaded: Set<LibrarySegmentRead> = []
    public var search = ReadSearchState(query: ReadingModel.emptyQuery)
    public private(set) var series: [String: SeriesView] = [:]
    public private(set) var chapters: [String: ReadChapters] = [:]
    public private(set) var versions: [String: [ChapterVersion]] = [:]
    public private(set) var tracking: [String: [TrackerReadRow]] = [:]
    public private(set) var downloads: BookDownloadsView?
    /// What the chapters being chosen will take to download, per series.
    public private(set) var estimates: [String: DownloadEstimate] = [:]
    /// What Remove from lists came to, per series, until the next removal.
    public private(set) var removals: [String: [ListRemoval]] = [:]
    public private(set) var failures: [ReadingAbout: ReadingFailure] = [:]
    /// The reader's defaults per medium, for Settings' Reading pane.
    public private(set) var readerDefaults: [Medium: ReaderSettingsView] = [:]
    /// What reading's search filters choose from, fixed in the engine.
    public let filters: ReadFilters = readFilters()
    /// Send to e-reader and the feed.
    public let sending = SendModel()
    @ObservationIgnored var send: ((Command) -> Void)? {
        didSet { sending.command = send }
    }

    public init() {}

    public static let emptyQuery = ReadSearchQuery(
        text: "", medium: nil, formats: [], origins: [], status: nil, genres: [], excludeGenres: [], sort: nil)

    /// How many chapters wait unread in Updates: the sidebar's count.
    public var unread: Int { Int(updates?.unread ?? 0) }

    // MARK: Asking

    public func loadHome(refresh: Bool = false) {
        homeLoading = true
        failures[.home] = nil
        send?(.reading(.loadHome(refresh: refresh)))
    }

    public func loadUpdates() { send?(.reading(.loadUpdates)) }
    public func checkUpdates() { send?(.reading(.checkUpdates)) }
    public func downloadNew() { send?(.reading(.downloadNew)) }

    public func loadList(_ segment: LibrarySegmentRead) {
        send?(.reading(.loadLibrary(segment: segment)))
    }

    public func runSearch(_ query: ReadSearchQuery) {
        search.query = query
        search.total = nil
        search.loading = true
        search.failure = nil
        // A browse asks AniList alone; a search with words gets the sources' part at once.
        if !search.hasWords { search.sources = nil }
        send?(.reading(.search(query: query)))
    }

    /// Try again on one source's group: only that group asks again.
    public func retrySourceSearch(_ sourceId: String) {
        guard let sources = search.sources else { return }
        send?(.reading(.retrySourceSearch(query: sources.query, sourceId: sourceId)))
    }

    public func searchMore() {
        guard search.hasMore, !search.loading else { return }
        search.loading = true
        send?(.reading(.searchMore(query: search.query, page: search.page + 1)))
    }

    public func loadSeries(_ work: String) {
        failures[.series] = nil
        send?(.reading(.loadSeries(work: work)))
        send?(.reading(.loadChapters(work: work, edition: nil)))
    }

    /// A series' details without its chapters, for a cover or title shown outside its page.
    public func loadSeriesInfo(_ work: String) {
        guard series[work] == nil else { return }
        send?(.reading(.loadSeries(work: work)))
    }

    public func loadChapters(_ work: String, edition: String? = nil) {
        send?(.reading(.loadChapters(work: work, edition: edition)))
    }

    public func loadVersions(_ work: String, chapter: String) {
        send?(.reading(.loadVersions(work: work, chapter: chapter)))
    }

    public func chooseEdition(_ work: String, _ edition: BookEditionRow) {
        send?(.reading(.chooseEdition(
            work: work, sourceId: edition.sourceId, versionLabel: edition.versionLabel, language: edition.language)))
    }

    /// Marks, then the series and its chapters again, as the engine sends nothing for a mark.
    public func markRead(_ work: String, chapters ids: [String], read: Bool) {
        send?(.reading(.markRead(work: work, chapters: ids, read: read)))
        refreshSeries(work)
    }

    public func markReadUpTo(_ work: String, chapter: String) {
        send?(.reading(.markReadUpTo(work: work, chapter: chapter)))
        refreshSeries(work)
    }

    public func readAgain(_ work: String) {
        send?(.reading(.readAgain(work: work)))
        refreshSeries(work)
    }

    public func undoReadAgain(_ work: String) {
        send?(.reading(.undoReadAgain(work: work)))
        refreshSeries(work)
    }

    public func bookmark(_ work: String, chapter: String, on: Bool) {
        send?(.reading(.bookmark(work: work, chapter: chapter, on: on)))
        send?(.reading(.loadChapters(work: work, edition: chapters[work]?.edition?.sourceId)))
    }

    public func hideFromContinue(_ work: String) {
        send?(.reading(.hideFromContinue(work: work)))
    }

    public func setStatus(_ work: String, _ status: LibrarySegmentRead) {
        send?(.reading(.setStatus(work: work, status: status)))
    }

    /// Takes the series off every signed-in tracker's reading list; Kaeri keeps the place.
    public func removeFromLists(_ work: String) {
        removals[work] = nil
        send?(.reading(.removeFromLists(work: work)))
    }

    public func setScore(_ work: String, _ score: Float) {
        send?(.reading(.setScore(work: work, score: score)))
    }

    public func loadTracking(_ work: String) { send?(.reading(.loadTracking(work: work))) }

    public func chooseMapping(_ work: String, tracker: String, id: String?) {
        send?(.reading(.chooseMapping(work: work, tracker: tracker, trackerId: id)))
    }

    public func resolveConflict(_ work: String, tracker: String, keepLocal: Bool) {
        send?(.reading(.resolveConflict(work: work, tracker: tracker, keepLocal: keepLocal)))
    }

    public func connectMangaUpdates(username: String, password: String) {
        send?(.reading(.connectMangaUpdates(username: username, password: password)))
    }

    public func download(_ work: String, _ pick: DownloadPick) {
        send?(.reading(.download(work: work, pick: pick)))
        send?(.reading(.loadChapters(work: work, edition: chapters[work]?.edition?.sourceId)))
    }

    public func cancelDownloads(_ work: String, chapters ids: [String]) {
        send?(.reading(.downloadCancel(work: work, chapters: ids)))
    }

    public func deleteDownloads(_ work: String, chapters ids: [String]) {
        send?(.reading(.downloadDelete(work: work, chapters: ids)))
        send?(.reading(.loadChapters(work: work, edition: chapters[work]?.edition?.sourceId)))
    }

    /// Delete read: the downloads of the series' read chapters, bookmarked ones kept.
    public func deleteRead(_ work: String) {
        send?(.reading(.downloadDeleteRead(work: work)))
        send?(.reading(.loadDownloads))
    }

    /// Asks what downloading `pick` will take; an empty pick clears the line.
    public func estimateDownload(_ work: String, _ pick: DownloadPick) {
        if case .chapters(let ids) = pick, ids.isEmpty {
            estimates[work] = nil
            return
        }
        send?(.reading(.estimateDownload(work: work, pick: pick)))
    }

    public func loadDownloads() { send?(.reading(.loadDownloads)) }

    private func refreshSeries(_ work: String) {
        if series[work] != nil {
            send?(.reading(.loadSeries(work: work)))
            send?(.reading(.loadChapters(work: work, edition: chapters[work]?.edition?.sourceId)))
        }
        // Updates' rows turn haze once read.
        if updates != nil { send?(.reading(.loadUpdates)) }
    }

    // MARK: Hearing

    func observe(_ event: Event) {
        if case .reader(.settings(let view)) = event, view.work == nil {
            readerDefaults[view.medium] = view
        }
        guard case .reading(let event) = event else { return }
        sending.apply(event)
        apply(event)
    }

    /// Ask for the manga and novel defaults, for Settings.
    public func loadReaderDefaults() {
        send?(.reader(.loadSettings(medium: .manga, work: nil)))
        send?(.reader(.loadSettings(medium: .novel, work: nil)))
    }

    /// Change a medium's default, as Settings does: kept for the medium and synced.
    public func setReaderDefault(_ medium: Medium, _ key: ReaderSettingKey, _ value: ReaderValue) {
        send?(.reader(.setSetting(session: nil, medium: medium, work: nil, key: key, scope: .medium, value: value)))
    }

    func apply(_ event: ReadingEvent) {
        switch event {
        case .home(let view):
            home = view
            homeLoading = false
            #if FIXTURES
            if stagedSpotlight { home = Self.newChapterSpotlight(view) }
            #endif
        case .updates(let view):
            updates = view
        case .library(let segment, let tiles):
            lists[segment] = tiles
            listsLoaded.insert(segment)
        case .search(let query, let page, let tiles, let hasMore, let otherScope, let total):
            guard query == search.query else { return }
            if page == 0 { search.total = nil }
            if let total { search.total = total }
            search.tiles = page == 0 ? tiles : search.tiles + tiles
            if page == 0 { search.firstPage = tiles.count }
            search.page = page
            search.hasMore = hasMore
            search.otherScope = otherScope
            search.loading = false
            #if FIXTURES
            restageSources()
            #endif
        case .sourceSearch(let part):
            #if FIXTURES
            // A shot's staged sources stand; the testing engine's own answer would replace them.
            guard stagedSources == nil else { return }
            #endif
            guard part.query == search.query else { return }
            search.sources = part
            switch part.anilist {
            case .skipped:
                // Your sources only: AniList is not asked, so no results of its come.
                search.tiles = []
                search.hasMore = false
                search.total = nil
                search.loading = false
            case .failed:
                search.loading = false
            case .asking, .answered:
                break
            }
        case .series(let view):
            series[view.work] = view
        case .chapters(let work, let edition, let volumes, let failure):
            chapters[work] = ReadChapters(edition: edition, volumes: volumes, failure: failure)
        case .versions(_, let chapter, let list):
            versions[chapter] = list
        case .tracking(let work, let rows):
            tracking[work] = rows
        case .downloads(let view):
            downloads = view
        case .downloadEstimate(let estimate):
            estimates[estimate.work] = estimate
        case .removedFromLists(let work, let results):
            removals[work] = results
            refreshSeries(work)
        case .failed(let about, let failure):
            failures[about] = failure
            if about == .home { homeLoading = false }
            if about == .search {
                search.loading = false
                search.failure = failure
            }
        default:
            break
        }
    }

    #if FIXTURES
    /// For shots: an event as the engine would send it.
    public func showcase(_ event: ReadingEvent) { apply(event) }
    #endif

    /// For shots: the spotlight's New chapter case, on the series it shows, with its newest
    /// chapter out today. The fixtures' reader is behind in every series, so the engine never
    /// picks this case on its own there. Does nothing outside the fixtures.
    public func stageSpotlightShot() {
        #if FIXTURES
        stagedSpotlight = true
        if let home { self.home = Self.newChapterSpotlight(home) }
        #endif
    }

    /// For shots: Search's sources' part in one of the board's states (`SourceSearchShots`), on
    /// the words searched, held against the testing engine's own answer. Does nothing outside the
    /// fixtures.
    public func stageSourceSearchShot(_ name: String) {
        #if FIXTURES
        guard SourceSearchShots.names.contains(name) else { return }
        stagedSources = name
        restageSources()
        #endif
    }

    #if FIXTURES
    @ObservationIgnored private var stagedSources: String?

    private func restageSources() {
        guard let name = stagedSources else { return }
        if name == SourceSearchShots.sourcesOnly { search.query.sourcesOnly = true }
        // The fixture covers come from wherever the fake serves art; any cover on screen says where.
        let covers = search.tiles + (home?.continueReading ?? []) + (home?.rows.flatMap(\.tiles) ?? [])
        let art = covers.lazy.compactMap(\.cover).compactMap { url in
            url.range(of: "/cover/").map { String(url[..<$0.upperBound]) } // i18n-skip
        }.first
        let staged = SourceSearchShots.make(name, query: search.query, results: search.tiles, art: art)
        search.sources = staged
        if staged.anilist != .answered || name == SourceSearchShots.nothing || name == SourceSearchShots.sourceHits {
            search.tiles = []
            search.hasMore = false
            search.total = staged.anilist == .answered ? 0 : nil
        }
        search.loading = false
    }

    @ObservationIgnored private var stagedSpotlight = false

    private static func newChapterSpotlight(_ view: ReadingHomeView) -> ReadingHomeView {
        guard let tile = view.spotlight, var spot = view.spot else { return view }
        var view = view
        let novel = tile.medium == .novel
        spot.case = .newChapter
        spot.label = .reading(.readingSpotlightNewChapter)
        if novel {
            spot.line = .reading(.readingPartOutToday(part: "4"))
            spot.action = .readPart(chapter: "4", label: "4", named: nil)
        } else {
            spot.line = .reading(.readingOutTodayTitled(chapter: "15", title: "The Far Shore")) // i18n-skip
            spot.action = .read(chapter: "15", label: "15")
        }
        view.spot = spot
        return view
    }
    #endif
}

/// The sources that have a work, for Read from.
public struct WorkEditions: Equatable, Sendable {
    public var editions: [BookEditionRow]
    public var asking: Bool
    /// The sources searched without finding it, with why: "Not found in".
    public var missing: [EditionMiss] = []
}

/// What "Search there" found in one source for a work.
public struct SourceSeriesSearch: Equatable, Sendable {
    public var sourceId: String
    public var query: String
    public var choices: [SourceSeriesChoice] = []
    public var failure: ReadingFailure?
    public var searching = true
}

/// Folders, servers and plugins that books come from.
@Observable @MainActor
public final class BookSourcesModel {
    public private(set) var view: BookSourcesView?
    /// The server Add a server found at the address typed, before signing in.
    public private(set) var checked: (address: String, kind: BookServerKind, name: String, version: String)?
    public private(set) var added: (id: String, kind: BookServerKind, series: UInt32?)?
    public private(set) var failed: (step: ServerStep, failure: BookServerFailure)?
    public private(set) var checking = false
    public private(set) var matches: [String: [MatchChoice]] = [:]
    /// Which of your sources have a work, as each answers: the Read from sheet.
    public private(set) var forWork: [String: WorkEditions] = [:]
    /// "Search there" for a work, one source at a time.
    public private(set) var sourceSeries: [String: SourceSeriesSearch] = [:]
    @ObservationIgnored var send: ((Command) -> Void)?

    public init() {}

    public var isEmpty: Bool {
        guard let view else { return true }
        return view.folders.isEmpty && view.servers.isEmpty && view.plugins.isEmpty
    }

    public func load() { send?(.bookSources(.load)) }
    public func addFolder(_ folder: FolderRef) { send?(.bookSources(.addFolder(folder: folder))) }
    public func removeFolder(_ id: String) { send?(.bookSources(.removeFolder(id: id))) }
    public func scan() { send?(.bookSources(.scan)) }
    /// The books page shows with the demo on: fetch its first pages ahead.
    public func prefetchDemo() { send?(.bookSources(.prefetchDemo)) }
    public func removeServer(_ id: String) { send?(.bookSources(.removeServer(id: id))) }
    public func setOff(_ id: String, off: Bool) { send?(.bookSources(.setOff(id: id, off: off))) }
    public func setReportProgress(_ id: String, on: Bool) { send?(.bookSources(.setReportProgress(id: id, on: on))) }
    public func findMatch(group: String, query: String) { send?(.bookSources(.findMatch(group: group, query: query))) }
    public func chooseMatch(group: String, anilist: AnilistId) { send?(.bookSources(.chooseMatch(group: group, anilist: anilist))) }
    public func keep(group: String) { send?(.bookSources(.keep(group: group))) }
    public func restoreFolder(_ id: String) { send?(.bookSources(.restoreFolder(id: id))) }
    public func setOrder(_ ids: [String]) { send?(.bookSources(.setOrder(ids: ids))) }

    public func loadForWork(_ work: String) {
        forWork[work] = WorkEditions(
            editions: forWork[work]?.editions ?? [], asking: true, missing: forWork[work]?.missing ?? [])
        send?(.bookSources(.loadForWork(work: work)))
    }

    /// "Search there": look for the work in one source by the words typed.
    public func searchSource(_ work: String, sourceId: String, query: String) {
        sourceSeries[work] = SourceSeriesSearch(sourceId: sourceId, query: query)
        send?(.bookSources(.searchSourceForWork(work: work, sourceId: sourceId, query: query)))
    }

    /// The series picked in that source is the work; the editions come again.
    public func chooseSourceSeries(_ work: String, sourceId: String, seriesRef: String) {
        sourceSeries[work] = nil
        forWork[work]?.asking = true
        send?(.bookSources(.chooseSourceSeries(work: work, sourceId: sourceId, seriesRef: seriesRef)))
    }

    public func clearSourceSearch(_ work: String) { sourceSeries[work] = nil }

    public func checkServer(_ address: String) {
        checked = nil
        failed = nil
        added = nil
        checking = true
        send?(.bookSources(.checkServer(address: address)))
    }

    public func signIn(address: String, user: String, password: String) {
        failed = nil
        checking = true
        send?(.bookSources(.signIn(address: address, user: user, password: password)))
    }

    public func resetServerSheet() {
        checked = nil
        added = nil
        failed = nil
        checking = false
    }

    func observe(_ event: Event) {
        guard case .bookSources(let event) = event else { return }
        switch event {
        case .view(let view):
            self.view = view
        case .matches(let group, _, let choices, _):
            matches[group] = choices
        case .serverChecked(let address, let kind, let name, let version):
            checked = (address, kind, name, version)
            checking = false
        case .serverAdded(let id, let kind, let series):
            added = (id, kind, series)
            checking = false
        case .serverFailed(let step, let failure):
            failed = (step, failure)
            checking = false
        case .forWork(let work, let editions, let asking, var missing):
            #if FIXTURES
            if missing.isEmpty, let staged = stagedMisses[work] { missing = staged }
            #endif
            forWork[work] = WorkEditions(editions: editions, asking: asking, missing: missing)
        case .sourceSeries(let work, let sourceId, let query, let choices, let failure):
            guard let current = sourceSeries[work], current.sourceId == sourceId, current.query == query else { return }
            sourceSeries[work] = SourceSeriesSearch(
                sourceId: sourceId, query: query, choices: choices, failure: failure, searching: false)
        }
    }

    #if FIXTURES
    public func showcase(_ view: BookSourcesView) { self.view = view }
    /// For shots: an event as the engine would send it.
    public func showcase(_ event: BookSourceEvent) { observe(.bookSources(event)) }
    #endif

    /// For shots: the add-server sheet with a made-up server found at `address`, and with
    /// `added` signed in with its series counted. Does nothing outside the fixtures.
    public func stageServerShot(address: String, added: Bool) {
        #if FIXTURES
        showcase(.serverChecked(address: address, kind: .kavita, name: "Books", version: "0.8")) // i18n-skip
        if added { showcase(.serverAdded(id: "fixture-server", kind: .kavita, series: 212)) } // i18n-skip
        #endif
    }

    /// For shots: Read from with a made-up source that had no series of the work's name and
    /// one with another edition of it, drawn with `cover`, once the work's editions are in. Does
    /// nothing outside the fixtures.
    public func stageMissShot(work: String, cover: String? = nil) async {
        #if FIXTURES
        let miss = EditionMiss(sourceId: "fixture-comics", sourceName: "Comics server", // i18n-skip: made up
                               reason: .reading(.readingMissNoSeries))
        // Another edition of it, offered as a possible match. i18n-skip: made up
        let colour = SourceSeriesChoice(seriesRef: "ferry-colour", title: "The Lantern Ferry - Full Color", // i18n-skip
                                        cover: cover, year: 2019, language: "en", confidence: 70) // i18n-skip
        let possible = EditionMiss(sourceId: "fixture-northwind", sourceName: "Northwind", // i18n-skip: made up
                                   reason: .reading(.readingMissPossible), possible: colour)
        let misses = [miss, possible]
        // Kept, so the engine's later answers for the work carry it too.
        stagedMisses[work] = misses
        for _ in 0..<30 where forWork[work] == nil {
            try? await Task.sleep(for: .milliseconds(100))
        }
        guard let found = forWork[work] else { return }
        showcase(.forWork(work: work, editions: found.editions, asking: found.asking, missing: misses))
        #endif
    }

    #if FIXTURES
    @ObservationIgnored private var stagedMisses: [String: [EditionMiss]] = [:]
    #endif
}

// How the Fixtures app and the dev harness start the testing engine.

#if FIXTURES
import KaeriKit
import Foundation

extension EngineLaunch {
    /// The launch argument that names the scene: `-KaeriScene watching`, `fresh`,
    /// `sourceless` (Watching with no source set up), `unsure` (Watching with a source ahead that
    /// cannot choose 900001's match), `linkopened` (a magnet link opened, on the confirm step),
    /// `playing`, `licensed` (Sourceless with a licensed service chosen), `servers` (a Jellyfin
    /// server saved), `library` (a folder of episodes added), `storeowner` (a plugin store of
    /// yours, with invites in use, used up and cancelled), `storeinvited` (a link for five people
    /// to join), or one of the demo and crash scenes: `demoonly`, `demoplayed`, `demoandstore`,
    /// `demoremoved`, `subtitlestyled`, `crashoff`, `crashsaved`, `crashheld`.
    public static let sceneArgument = "-KaeriScene"

    /// The launch argument that turns on the Home world, `-KaeriHomeWorld 1`: about ten
    /// invented titles this season with drawn art, which Home's shelves need to look like the
    /// board. The engine reads it as `KAERI_FIXTURES_HOME_WORLD=1`, set here before it starts.
    public static let homeWorldArgument = "-KaeriHomeWorld"

    /// Set the Home world in the environment when the arguments ask for it.
    public static func applyHomeWorld(arguments: [String] = ProcessInfo.processInfo.arguments) {
        guard let index = arguments.firstIndex(of: homeWorldArgument), arguments.indices.contains(index + 1),
              arguments[index + 1] == "1" else { return }
        setenv("KAERI_FIXTURES_HOME_WORLD", "1", 1)
    }

    /// The launch argument that names what the Playing scene streams: `-KaeriMedia <path>`.
    /// With the Watching scene, every episode streams it, so a resume plays it at once.
    public static let mediaArgument = "-KaeriMedia"

    /// What the Playing scene streams when the arguments name nothing: the episode
    /// `apple/scripts/player-media.sh` makes in this checkout. The path stays in the checkout:
    /// `standardizedFileURL` would follow `apple/.mpv` when it links to a shared libmpv folder
    /// and the episode is there, so `standardized` only drops the `..` steps.
    public static let fixtureEpisode = URL(filePath: #filePath)
        .deletingLastPathComponent()
        .appending(path: "../../../../.mpv/media/episode.mkv")
        .standardized

    /// The scene `arguments` name, Watching when they name none.
    public static func scene(arguments: [String]) -> FfiScene? {
        guard let index = arguments.firstIndex(of: sceneArgument) else { return .watching }
        guard arguments.indices.contains(index + 1) else { return nil }
        switch arguments[index + 1].lowercased() {
        case "watching":
            guard let index = arguments.firstIndex(of: mediaArgument), arguments.indices.contains(index + 1)
            else { return .watching }
            return .watchingMedia(media: arguments[index + 1])
        case "fresh": return .fresh
        case "suggested": return .suggested
        case "oneleft": return .oneLeft
        case "browsing": return .browsing
        case "continuefull": return .continueFull
        case "notfound": return .notFound
        case "slow": return .slow
        case "failed": return .failed
        case "offline": return .offline
        case "sourceless": return .sourceless
        case "unsure": return .unsure
        case "linkopened": return .linkOpened
        case "demoonly": return .demoOnly
        case "demoplayed": return .demoPlayed
        case "demoandstore": return .demoAndStore
        case "demoremoved": return .demoRemoved
        case "subtitlestyled": return .subtitleStyled
        case "crashoff": return .crashOff
        case "crashsaved": return .crashSaved
        case "crashheld": return .crashHeld
        case "licensed": return .licensed
        case "servers": return .servers
        case "library": return .library
        case "sourcesbusy": return .sourcesBusy
        case "available": return .available
        case "together": return .together
        case "playing": return .playing(media: media(arguments: arguments))
        case "storeowner": return .storeOwner
        case "storeinvited": return .storeInvited
        case "readingfirstrun": return .readingFirstRun
        case "readingupdatesheet": return .readingUpdateSheet
        case "readingjoinask": return .readingJoinAsk
        case "readinghome": return .readingHome
        case "readingdemo": return .readingDemo
        case "readingseries": return .readingSeries
        case "readingdownloads": return .readingDownloads
        case "readermangartl": return .readerMangaRtl
        case "readerstrip": return .readerStrip
        case "readernovel": return .readerNovel
        case "readermoved": return .readerMoved
        case "readerlong": return .readerLong
        case "einkgrey": return .einkGrey
        case "einkkaleidounsure": return .einkKaleidoUnsure
        case "einklong": return .einkLong
        case "readingdark": return .readingDark
        default: return nil
        }
    }

    /// What the Playing scene streams: the file or URL `-KaeriMedia` names, otherwise the
    /// fixture episode.
    static func media(arguments: [String]) -> String {
        if let index = arguments.firstIndex(of: mediaArgument), arguments.indices.contains(index + 1) {
            return arguments[index + 1]
        }
        return fixtureEpisode.path(percentEncoded: false)
    }

    /// The testing engine on the scene the arguments name, in the home override when there is
    /// one and in a new directory under `temporaryParent` otherwise: a scene seeds the database
    /// it starts on, so every launch starts from an empty one, and never from the terminal's.
    /// `temporary` is that directory, for the caller to remove at quit. A killed app never quits,
    /// so each launch first removes the homes earlier launches left behind (`sweepFixturesHomes`).
    public static func fixtures(
        platform: FfiPlatform,
        arguments: [String] = ProcessInfo.processInfo.arguments,
        environment: [String: String] = ProcessInfo.processInfo.environment,
        temporaryParent: URL = FileManager.default.temporaryDirectory
    ) -> (launch: EngineLaunch, temporary: URL?) {
        let scene = scene(arguments: arguments) ?? .watching
        applyHomeWorld(arguments: arguments)
        let root = EngineHome.overrideRoot(environment: environment, arguments: arguments)
        var temporary: URL?
        if root == nil {
            sweepFixturesHomes(in: temporaryParent)
            temporary = makeFixturesHome(in: temporaryParent)
        }
        let launch = EngineLaunch(
            engine: .fixtures(scene),
            home: .override(root: root ?? temporary!),
            platform: platform,
            secrets: .host
        )
        return (launch, temporary)
    }

    /// The name every Fixtures home starts with, under the temporary directory.
    static let fixturesHomePrefix = "kaeri-fixtures-"

    /// The file in a Fixtures home that names the process using it, by its pid.
    static let fixturesOwnerFile = ".owner"

    /// How old a Fixtures home without an owner file has to be before a launch removes it. Every
    /// home made since owner files came has one from the start, so only older homes lack it.
    static let fixturesOwnerlessAge: TimeInterval = 60 * 60

    /// A new Fixtures home under `parent`, made now with this process named as its owner.
    static func makeFixturesHome(in parent: URL) -> URL {
        let home = parent.appending(
            path: "\(fixturesHomePrefix)\(UUID().uuidString)", directoryHint: .isDirectory
        )
        try? FileManager.default.createDirectory(at: home, withIntermediateDirectories: true)
        let owner = Data("\(ProcessInfo.processInfo.processIdentifier)\n".utf8)
        try? owner.write(to: home.appending(path: fixturesOwnerFile), options: .atomic)
        return home
    }

    /// Remove the Fixtures homes under `parent` that no running process owns: those whose owner
    /// file names a process that is gone, and those without one that are older than
    /// `fixturesOwnerlessAge`. A home whose owner is alive stays, so Fixtures apps running side
    /// by side keep theirs.
    static func sweepFixturesHomes(in parent: URL, now: Date = Date()) {
        let files = FileManager.default
        guard let entries = try? files.contentsOfDirectory(
            at: parent, includingPropertiesForKeys: [.contentModificationDateKey, .isDirectoryKey]
        ) else { return }
        for home in entries where home.lastPathComponent.hasPrefix(fixturesHomePrefix) {
            let values = try? home.resourceValues(forKeys: [.contentModificationDateKey, .isDirectoryKey])
            guard values?.isDirectory == true else { continue }
            let owner = (try? String(contentsOf: home.appending(path: fixturesOwnerFile), encoding: .utf8))
                .flatMap { pid_t($0.trimmingCharacters(in: .whitespacesAndNewlines)) }
            let stale: Bool
            if let owner {
                stale = !processIsAlive(owner)
            } else {
                let modified = values?.contentModificationDate ?? .distantPast
                stale = now.timeIntervalSince(modified) > fixturesOwnerlessAge
            }
            if stale { try? files.removeItem(at: home) }
        }
    }

    /// Whether a process with this pid runs. One that exists but may not be signalled counts as
    /// running, so a home is never removed on a doubt.
    static func processIsAlive(_ pid: pid_t) -> Bool {
        guard pid > 0 else { return false }
        return kill(pid, 0) == 0 || errno == EPERM
    }
}
extension AppStore {
    /// The launch argument that marks episodes watched once the engine is up, as if they had just
    /// been watched: `-KaeriWatched 900006:8`, the episodes joined by commas, a run as `1-147`. For a shot of a
    /// title caught up with its broadcasts.
    public static let watchedArgument = "-KaeriWatched"

    /// Mark what `arguments` name watched.
    public func applyWatchedArgument(_ arguments: [String] = ProcessInfo.processInfo.arguments) {
        guard let index = arguments.firstIndex(of: Self.watchedArgument), arguments.indices.contains(index + 1)
        else { return }
        let parts = arguments[index + 1].split(separator: ":", maxSplits: 1)
        guard parts.count == 2, let id = AnilistId(parts[0]) else { return }
        // A run is `1-147`; the rest are numbers, joined by commas.
        let episodes = parts[1].split(separator: ",").flatMap { part -> [String] in
            let ends = part.split(separator: "-").compactMap { Int($0) }
            guard ends.count == 2, ends[0] <= ends[1] else { return [String(part)] }
            return (ends[0]...ends[1]).map(String.init)
        }
        send(.setWatched(id: id, episodes: episodes, watched: true))
        // Asked again after the mark, which the engine takes in order: what the screens asked for
        // while it started went ahead of it, and says the title and its episodes as they were.
        continuing.list.asked()
        send(.loadContinue)
        send(.loadDetail(id))
        send(.loadEpisodes(id))
    }

    /// The launch argument that takes the fake AniList away once the first Home has loaded,
    /// `-KaeriGoOffline 1`: Home's offline line over what it kept, for a shot of it.
    public static let goOfflineArgument = "-KaeriGoOffline"

    /// Go offline when `arguments` ask, once settled, and ask Home again. Whether it did.
    public func applyGoOfflineArgument(_ arguments: [String] = ProcessInfo.processInfo.arguments) -> Bool {
        guard let index = arguments.firstIndex(of: Self.goOfflineArgument), arguments.indices.contains(index + 1),
              arguments[index + 1] == "1"
        else { return false }
        setFakeOffline(true)
        loadHome(force: true)
        return true
    }

    /// The launch arguments that hold answers back, as a slow network gives them, for a motion
    /// check of what a screen does meanwhile: `-KaeriDetailDelay 600` for each title's page and
    /// what the services say of it, `-KaeriEpisodesDelay 600` for each episode table,
    /// `-KaeriListDelay 600` for Home, the calendar, Seasonal and the Library. In milliseconds;
    /// the dev harness's `--detail-delay`, `--episodes-delay` and `--list-delay`.
    public static let detailDelayArgument = "-KaeriDetailDelay"
    public static let episodesDelayArgument = "-KaeriEpisodesDelay"
    public static let listDelayArgument = "-KaeriListDelay"

    /// Hold back what `arguments` ask to.
    public func applyDelayArguments(_ arguments: [String] = ProcessInfo.processInfo.arguments) {
        func delay(_ name: String) -> Duration? {
            guard let index = arguments.firstIndex(of: name), arguments.indices.contains(index + 1),
                  let milliseconds = Int(arguments[index + 1]), milliseconds > 0
            else { return nil }
            return .milliseconds(milliseconds)
        }
        detailDelay = delay(Self.detailDelayArgument)
        episodesDelay = delay(Self.episodesDelayArgument)
        listDelay = delay(Self.listDelayArgument)
    }
}
#endif

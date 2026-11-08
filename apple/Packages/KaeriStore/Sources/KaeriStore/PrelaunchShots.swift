// States of the prelaunch pieces for screenshots, staged without a server, a store or a second
// device: the Fixtures app and the dev harness say `-KaeriPrelaunch <state>`, and the models take
// the answer named. The sheets that show it open with `-KaeriSheet came-in` (Your sources) and
// the first-run pages with `-KaeriSheet welcome-join`, `welcome-sources` and `welcome-both`.

import KaeriKit
import Foundation

/// The states `-KaeriPrelaunch` stages.
///
///     Piece 1  kavita-link, looking, found, unreachable, wrong-password, wrong-key, empty,
///              not-book, folder, folder-nothing, invite-joined, plugins-installed, store-other,
///              store-empty, store-unreachable, afterwards
///     Piece 2  demo-first-read, demo-first-both, demo-home, demo-series, demo-reader, demo-end,
///              demo-about, demo-search
///     Piece 3  join-step, join-code, join-nothing, join-expired, join-arriving, join-paired,
///              join-refused, join-home, join-other
///     Piece 4  progress-saved, progress-sending, progress-offline, progress-retrying,
///              progress-failed (the page between chapters, with reader=end on a reader scene);
///              row-saved, row-waiting, row-retrying, row-failed, row-off (Your sources)
public enum PrelaunchScene: String, Sendable, CaseIterable {
    case kavitaLink = "kavita-link", looking, found, unreachable, wrongPassword = "wrong-password"
    case wrongKey = "wrong-key", empty, notBook = "not-book", folder, folderNothing = "folder-nothing"
    case inviteJoined = "invite-joined", pluginsInstalled = "plugins-installed", storeOther = "store-other"
    case storeEmpty = "store-empty", storeUnreachable = "store-unreachable", afterwards
    case demoFirstRead = "demo-first-read", demoFirstBoth = "demo-first-both", demoHome = "demo-home"
    case demoSeries = "demo-series", demoReader = "demo-reader", demoEnd = "demo-end"
    case demoAbout = "demo-about", demoSearch = "demo-search"
    case joinStep = "join-step", joinCode = "join-code", joinNothing = "join-nothing"
    case joinExpired = "join-expired", joinArriving = "join-arriving", joinPaired = "join-paired"
    case joinRefused = "join-refused", joinHome = "join-home", joinOther = "join-other"
    case progressSaved = "progress-saved", progressSending = "progress-sending", progressOffline = "progress-offline"
    case progressRetrying = "progress-retrying", progressFailed = "progress-failed"
    case rowSaved = "row-saved", rowWaiting = "row-waiting", rowRetrying = "row-retrying", rowFailed = "row-failed"
    case rowOff = "row-off"
    /// For motion clips: the line goes Sending, then Saved; the row goes Not saved, Try again is
    /// pressed, Sending, then Saved. Each holds long enough to film.
    case progressMotion = "progress-motion", rowMotion = "row-motion"

    public static let argument = "-KaeriPrelaunch"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> PrelaunchScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return PrelaunchScene(rawValue: arguments[index + 1])
    }

    /// The first-run page a join state is drawn on.
    public var isJoin: Bool { rawValue.hasPrefix("join-") }
    public var isDemo: Bool { rawValue.hasPrefix("demo-") }
    /// Piece 4 on the page between chapters.
    public var isProgressLine: Bool { rawValue.hasPrefix("progress-") }
    /// A motion clip's scene: it plays by itself after the screen is up.
    public var isMotion: Bool { rawValue.hasSuffix("-motion") }
    /// Piece 4 on the server's row.
    public var isProgressRow: Bool { rawValue.hasPrefix("row-") }
}

/// The made-up server, folder and store the states show. Hosts end in `.example`.
public enum PrelaunchFixtures {
    public static let host = "books.home.example"
    public static let libraries = [
        ServerLibrary(id: "1", name: "Manga", series: 168),
        ServerLibrary(id: "2", name: "Comics", series: 44),
    ]
    public static var found: ServerFound {
        ServerFound(pending: "p1", kind: .kavita, name: "Books", host: host, address: "https://\(host)",
                    series: 212, libraries: libraries, keyLast4: nil)
    }
    public static var foundByKey: ServerFound {
        ServerFound(pending: "p1", kind: .kavita, name: "Books", host: host, address: "https://\(host)",
                    series: 212, libraries: libraries, keyLast4: "3f9a")
    }
    public static var empty: ServerFound {
        ServerFound(pending: "p2", kind: .komga, name: "Books", host: host, address: "https://\(host)",
                    series: 0, libraries: [], keyLast4: nil)
    }
    public static let path = "~/Books/Manga"
    public static func folder(readable: Bool) -> FolderReport {
        FolderReport(
            id: "f1", name: "Manga", path: path, series: readable ? 64 : 0, books: readable ? 1_183 : 0,
            files: readable ? 1_206 : 41, matched: readable ? 58 : 0,
            unsupported: [FormatCount(format: "MOBI", count: readable ? 14 : 29), FormatCount(format: "AZW3", count: readable ? 6 : 12)],
            locked: readable ? [FormatCount(format: "EPUB", count: 3)] : [],
            listed: ["Night Post 1.mobi", "Night Post 2.mobi", "Salt Road 1.azw3"])
    }
    public static func store(reading: Int, watching: Int, otherIsReading: Bool = false) -> StoreReport {
        let names = ["Tsuru", "Lantern Pages", "Moss"]
        return StoreReport(
            store: "shelf.kim.example", name: "Kim’s shelf", owner: "Kim",
            forReading: Array(names.prefix(reading)), forWatching: watching > 0 ? ["Harbour"] : [],
            otherMedium: UInt32(watching), otherIsReading: otherIsReading, installAll: reading >= 2)
    }
}

extension CameInModel {
    /// Stage a piece 1 state.
    public func stage(_ scene: PrelaunchScene) {
        let host = PrelaunchFixtures.host
        switch scene {
        case .kavitaLink: stage(nil)
        case .looking: stage(nil, looking: true)
        case .found: stage(.found(PrelaunchFixtures.found))
        case .unreachable: stage(.empty(.unreachable(host: "comics.home.example"))) // i18n-skip: a made-up host, as the board has it
        case .wrongPassword: stage(.empty(.wrongPassword(host: host, kind: .kavita)))
        case .wrongKey: stage(.empty(.wrongKey(host: host)))
        case .empty: stage(.empty(.emptyCatalogue(found: PrelaunchFixtures.empty)))
        case .notBook: stage(.empty(.notABookServer(host: host)))
        case .folder: stage(.folder(PrelaunchFixtures.folder(readable: true)))
        case .folderNothing: stage(.empty(.unsupportedFiles(folder: PrelaunchFixtures.folder(readable: false))))
        case .inviteJoined: stage(.store(PrelaunchFixtures.store(reading: 2, watching: 1)))
        case .pluginsInstalled:
            stage(.installed(PluginsInstalled(store: "shelf.kim.example", name: "Kim’s shelf", installed: 2, failed: [])))
        case .storeOther: stage(.empty(.otherMedium(store: PrelaunchFixtures.store(reading: 0, watching: 3))))
        case .storeEmpty: stage(.empty(.emptyStore(store: PrelaunchFixtures.store(reading: 0, watching: 0))))
        case .storeUnreachable: stage(.empty(.storeUnreachable(host: "shelf.kim.example")))
        case .afterwards:
            stage(.serverAdded(AddedServer(id: "s1", kind: .kavita, name: "Books", host: host, series: 212, libraries: 2, empty: false)))
        default: break
        }
    }
}

extension SyncModel {
    /// Stage a piece 3 state: the code page's invite, the pairing, and what the other device brings.
    public func stage(prelaunch scene: PrelaunchScene) {
        guard scene.isJoin else { return }
        let device = "Kim’s iPhone"
        let setup = JoinedSetup(
            device: "kim-phone", deviceName: device, medium: .read,
            stores: [SyncStoreOffer(address: "shelf.kim.example", name: "Kim’s shelf", inviteFrom: "kim-phone")],
            servers: [SyncBookServerOffer(from: "kim-phone", id: "s1", kind: .kavita, name: "Books", host: PrelaunchFixtures.host,
                                          address: "https://\(PrelaunchFixtures.host)", user: nil, keyLast4: "3f9a",
                                          libraries: ["Manga", "Comics"], added: false)],
            signedIn: ["anilist", "mangaupdates"], folders: 2, answered: [.medium, .lists])
        switch scene {
        case .joinCode, .joinNothing:
            staged = true
            stage(invite: SyncInviteView(code: "482913", port: 51_742, until: .now.addingTimeInterval(540)))
        case .joinExpired:
            staged = true
            stage(invite: SyncInviteView(code: "482913", port: 51_742, until: .now.addingTimeInterval(-5)))
        case .joinArriving:
            staged = true
            stage(pair: .paired(device))
        case .joinPaired:
            stage(setup: setup)
            stage(pair: .paired(device))
        case .joinRefused:
            stage(setup: setup, refused: "Kim’s shelf")
            stage(pair: .paired(device))
        case .joinOther:
            staged = true
            sheet = .pairEnter
        case .joinHome:
            staged = true
            joinedFrom = device
        default:
            staged = true
        }
    }
}

// MARK: - Piece 4

extension PrelaunchFixtures {
    static let comicsHost = "comics.home.example"

    /// Your sources as the board draws it: books.home.example with its progress as `scene` says,
    /// comics.home.example, and one plugin from Kim's shelf. The Demo row comes from the demo.
    static func sources(_ scene: PrelaunchScene?) -> BookSourcesView {
        let now = Int64(Date.now.timeIntervalSince1970)
        var books = BookServerRow(
            id: "s1", kind: .kavita, name: host, address: "https://\(host)", series: 212, reportProgress: true,
            off: false, signedOut: false, libraries: 2, empty: false, rechecksAt: nil, keyLast4: "3f9a",
            progressPending: 0, progressSending: false, progressSavedAt: now - 120, progressProblem: nil,
            progressProblemAt: nil)
        switch scene {
        case .rowWaiting?:
            books.progressPending = 3
            books.progressProblem = .offline
        case .rowRetrying?:
            books.progressPending = 3
            books.progressProblem = .noAnswer
            books.progressProblemAt = now - 40
        case .rowFailed?, .rowMotion?:
            books.progressPending = 3
            books.progressProblem = .refusedKey
            books.progressProblemAt = now - 40
        case .rowOff?:
            books.reportProgress = false
            books.progressSavedAt = nil
            books.progressProblem = .notSetToReceive
        default:
            break
        }
        let comics = BookServerRow(
            id: "s2", kind: .opds, name: comicsHost, address: "https://\(comicsHost)", series: 38,
            reportProgress: false, off: false, signedOut: false, libraries: 1, empty: false, rechecksAt: nil,
            keyLast4: nil, progressPending: 0, progressSending: false, progressSavedAt: nil, progressProblem: nil,
            progressProblemAt: nil)
        let plugin = BookPluginRow(id: "tsuru", name: "Tsuru", version: "1.4", media: [.manga], languages: ["en"], // i18n-skip: made up
                                   off: false, store: "Kim’s shelf") // i18n-skip: made up
        return BookSourcesView(folders: [], groups: [], servers: [books, comics], plugins: [plugin], order: ["s1", "s2", "tsuru"])
    }

    /// The page between chapters' line for a piece 4 state.
    static func line(_ scene: PrelaunchScene) -> ServerLine? {
        switch scene {
        case .progressSaved: .saved(host: host)
        case .progressSending: .sending(host: host)
        case .progressOffline: .offline(host: host)
        case .progressRetrying: .retrying(host: host)
        case .progressFailed: .failed(host: host, why: .key)
        case .progressMotion: .sending(host: host)
        default: nil
        }
    }
}

extension AppStore {
    /// For shots: Your sources with the board's rows, kept on screen over the engine's own. Every
    /// prelaunch state drawn on Your sources shows them; piece 4's set the server's progress.
    public func stagePrelaunchSources(_ scene: PrelaunchScene) {
        #if FIXTURES
        held.remove("bookSourcesView")
        apply(.bookSources(.view(PrelaunchFixtures.sources(scene.isProgressRow ? scene : nil))))
        held.insert("bookSourcesView")
        #endif
    }

    /// For shots: the page between chapters of the open reader read, AniList updated, with the
    /// server line a piece 4 state names.
    public func stagePrelaunchLine(_ scene: PrelaunchScene) {
        #if FIXTURES
        guard scene.isProgressLine, let chapter = reader.view?.chapter.id else { return }
        reader.stageRead(trackers: ["AniList"]) // i18n-skip: a tracker's name
        reader.stageNextSource(PrelaunchFixtures.host)
        serverProgress.stage(chapter: chapter, line: PrelaunchFixtures.line(scene), server: "s1")
        if scene == .progressMotion {
            let progress = serverProgress
            Task { @MainActor in
                try? await Task.sleep(for: .seconds(3))
                progress.stage(chapter: chapter, line: .saved(host: PrelaunchFixtures.host), server: "s1")
            }
        }
        #endif
    }

    /// For motion clips: the row's Try again pressed, Sending, then Saved, as `row-motion` plays.
    public func playPrelaunchRow(_ scene: PrelaunchScene) {
        #if FIXTURES
        guard scene == .rowMotion else { return }
        let progress = serverProgress
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(3))
            progress.stage(row: "s1", line: .failed(count: 3, why: .key), pressing: true)
            try? await Task.sleep(for: .seconds(1))
            progress.stage(row: "s1", line: .sending(count: 3), pressing: false)
            try? await Task.sleep(for: .seconds(2))
            progress.stage(row: "s1", line: .saved(at: Int64(Date.now.timeIntervalSince1970)))
        }
        #endif
    }
}

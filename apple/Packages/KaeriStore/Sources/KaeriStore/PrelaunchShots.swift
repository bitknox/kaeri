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

    public static let argument = "-KaeriPrelaunch"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> PrelaunchScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return PrelaunchScene(rawValue: arguments[index + 1])
    }

    /// The first-run page a join state is drawn on.
    public var isJoin: Bool { rawValue.hasPrefix("join-") }
    public var isDemo: Bool { rawValue.hasPrefix("demo-") }
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
        case .unreachable: stage(.empty(.unreachable(host: host)))
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
    /// Stage a piece 3 state's setup: what the other device brings.
    public func stage(prelaunch scene: PrelaunchScene) {
        guard scene.isJoin else { return }
        let device = "Kim’s iPhone"
        stage(setup: JoinedSetup(
            device: "kim-phone", deviceName: device, medium: .read,
            stores: [SyncStoreOffer(address: "shelf.kim.example", name: "Kim’s shelf", inviteFrom: "kim-phone")],
            servers: [SyncBookServerOffer(from: "kim-phone", id: "s1", kind: .kavita, name: "Books", host: PrelaunchFixtures.host,
                                          address: "https://\(PrelaunchFixtures.host)", user: nil, keyLast4: "3f9a",
                                          libraries: ["Manga", "Comics"], added: false)],
            signedIn: ["anilist", "mangaupdates"], folders: 2, answered: [.medium, .lists]))
    }
}

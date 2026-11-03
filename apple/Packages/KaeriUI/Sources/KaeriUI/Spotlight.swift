// Spotlight: the titles on your Library lists and on Continue, found from the system's search,
// each opening its page. Kept as those lists change: a title that leaves them leaves the index.

import KaeriKit
import KaeriStore
import CoreGraphics
import CoreSpotlight
import Foundation
import ImageIO
import Observation
import Security
import UniformTypeIdentifiers
#if os(macOS)
import AppKit
#endif

/// One title as Spotlight is given it.
public struct SpotlightItem: Equatable, Sendable {
    public var identifier: String
    /// The list it is on: `watching`, `continue`, after the app's prefix.
    public var domain: String
    public var title: String
    /// Its other names: the other-language title first.
    public var alternateNames: [String]
    /// The line under it: the list and the episode, "Watching · Episode 7 of 24".
    public var line: String
    /// Its genres, format and year.
    public var keywords: [String]
    /// The cover, 120 by 180, as JPEG.
    public var thumbnail: Data?

    public init(_ title: ShelfTitle, prefix: String, thumbnail: Data? = nil) {
        identifier = Self.identifier(title.id)
        domain = prefix + (title.list ?? .continue).rawValue
        self.title = title.title
        alternateNames = title.otherTitles
        line = title.line
        keywords = title.keywords
        self.thumbnail = thumbnail
    }

    /// Every domain the app's titles go under.
    public static func domains(prefix: String) -> [String] {
        Shelf.Source.allCases.map { prefix + $0.rawValue }
    }

    /// A title's identifier in the index: `title/900001`.
    public static func identifier(_ id: AnilistId) -> String {
        "title/\(id)"
    }

    /// The title an identifier names.
    public static func title(_ identifier: String) -> AnilistId? {
        guard identifier.hasPrefix("title/") else { return nil }
        return AnilistId(identifier.dropFirst("title/".count))
    }

    /// The title a Spotlight result the app was opened from names.
    public static func title(in activity: NSUserActivity) -> AnilistId? {
        guard activity.activityType == CSSearchableItemActionType,
              let identifier = activity.userInfo?[CSSearchableItemActivityIdentifier] as? String
        else { return nil }
        return title(identifier)
    }

    /// The item as CoreSpotlight takes it. It does not expire: it stays while the title is on
    /// the shelf, and goes when the title leaves.
    public var searchable: CSSearchableItem {
        let attributes = CSSearchableItemAttributeSet(contentType: .content)
        attributes.title = title
        attributes.displayName = title
        attributes.alternateNames = alternateNames
        attributes.contentDescription = line.isEmpty ? nil : line
        attributes.keywords = keywords
        attributes.thumbnailData = thumbnail
        let item = CSSearchableItem(uniqueIdentifier: identifier, domainIdentifier: domain, attributeSet: attributes)
        item.expirationDate = .distantFuture
        return item
    }
}

/// Where the indexer finds covers: the store's artwork, or a test's.
@MainActor
public protocol SpotlightCovers: AnyObject {
    /// The cover at `url`, when it is here.
    func image(_ url: String) -> CGImage?
    /// Ask for the cover at `url`, small.
    func request(cover url: String)
    /// Whether the cover at `url` has been asked for and not answered.
    func isPending(_ url: String) -> Bool
}

extension ArtworkStore: SpotlightCovers {
    public func request(cover url: String) {
        request(url, maxPixelSize: SpotlightIndexer.coverPixels)
    }
}

/// What the indexer asks of Spotlight, so the tests can stand in for it.
public protocol SpotlightIndex: Sendable {
    func index(_ items: [SpotlightItem]) async throws
    func delete(_ identifiers: [String]) async throws
    func deleteAll(domains: [String]) async throws
    /// The other copies of this app signed with a certificate that the system knows of.
    /// Spotlight turns the app away while there are any (see `SpotlightIndexer.isTurnedAway`).
    func otherSignedCopies() async -> [String]
}

/// The app's own index in Spotlight.
public struct SystemSpotlightIndex: SpotlightIndex {
    public init() {}

    @concurrent public func otherSignedCopies() async -> [String] {
        #if os(macOS)
        guard let id = Bundle.main.bundleIdentifier else { return [] }
        let own = Bundle.main.bundleURL.standardizedFileURL
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        return NSWorkspace.shared.urlsForApplications(withBundleIdentifier: id)
            .filter { $0.standardizedFileURL != own && Self.isSignedWithACertificate($0) }
            .map { $0.path.hasPrefix(home + "/") ? "~" + $0.path.dropFirst(home.count) : $0.path }
        #else
        return []
        #endif
    }

    #if os(macOS)
    /// Whether the app at `url` is signed with a certificate, not ad hoc.
    static func isSignedWithACertificate(_ url: URL) -> Bool {
        var code: SecStaticCode?
        guard SecStaticCodeCreateWithPath(url as CFURL, [], &code) == errSecSuccess, let code else { return false }
        var info: CFDictionary?
        guard SecCodeCopySigningInformation(code, SecCSFlags(rawValue: kSecCSSigningInformation), &info) == errSecSuccess,
              let info = info as? [String: Any]
        else { return false }
        return (info[kSecCodeInfoCertificates as String] as? [Any])?.isEmpty == false
    }
    #endif

    public func index(_ items: [SpotlightItem]) async throws {
        let searchable = items.map(\.searchable)
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
            CSSearchableIndex.default().indexSearchableItems(searchable) { error in
                if let error { done.resume(throwing: error) } else { done.resume() }
            }
        }
    }

    public func delete(_ identifiers: [String]) async throws {
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
            CSSearchableIndex.default().deleteSearchableItems(withIdentifiers: identifiers) { error in
                if let error { done.resume(throwing: error) } else { done.resume() }
            }
        }
    }

    public func deleteAll(domains: [String]) async throws {
        try await withCheckedThrowingContinuation { (done: CheckedContinuation<Void, any Error>) in
            CSSearchableIndex.default().deleteSearchableItems(withDomainIdentifiers: domains) { error in
                if let error { done.resume(throwing: error) } else { done.resume() }
            }
        }
    }
}

/// Keeps Spotlight's titles the shelf's: indexes the titles that join it or change, with their
/// covers once those are here, and removes the ones that leave.
///
/// It watches the store's lists and titles, and a moment after they settle compares the shelf
/// with the one it saved: only what differs goes to Spotlight. With nothing saved (a first
/// launch, or the Fixtures app's every launch) it clears its domains first, so nothing indexed
/// before outlives it. "Show titles in Spotlight" off clears them and indexes nothing.
///
/// A pass Spotlight fails keeps the saved shelf as it was, so the next pass sends the same
/// change. That pass waits a minute, the one after it two, and so on up to an hour, however
/// often the lists change meanwhile, and the same failure is logged once.
@MainActor
public final class SpotlightIndexer {
    /// The switch, in the app's defaults: on unless turned off.
    public static let preference = "kaeri.showInSpotlight"
    /// The long side of the cover in the index, in pixels.
    nonisolated static let coverPixels = 180
    /// How many covers are asked for at once.
    static let coversAtOnce = 8

    private let store: AppStore
    private let index: any SpotlightIndex
    /// What the index's domains start with: nothing for the app, whose domains are its lists'
    /// names, and `fixtures.` for the Fixtures app, so its titles never mix with the app's.
    public let prefix: String
    private let file: URL?
    private let defaults: UserDefaults
    private let delay: Duration
    /// How long a failed pass holds the next one off, doubled for each failure in a row.
    private let retry: Duration
    private let covers: any SpotlightCovers
    /// The failures in a row since a pass last went through.
    private var failures = 0
    /// No pass before this, after a failure: the lists change every few seconds while the app
    /// loads, and each pass would draw the covers again for Spotlight to refuse again.
    private var resumeAt: ContinuousClock.Instant?
    /// The failure last logged, so the same one is logged once, not on every pass.
    private var reported: String?
    private var saved: Shelf?
    /// Whether the domain was cleared this session, for a start with nothing saved.
    private var cleared = false
    private var scheduled: Task<Void, Never>?
    private var running = false
    private var again = false
    /// The covers asked for and not here yet.
    private var asked: Set<String> = []
    /// The covers that did not come, not asked for again this session.
    private var missed: Set<String> = []
    private var defaultsObserver: (any NSObjectProtocol)?
    /// The newest watch on the store.
    private var generation = 0
    /// Called after Spotlight was told of a change to the shelf.
    public var onUpdate: (@MainActor () -> Void)?

    /// - Parameters:
    ///   - file: where the shelf is saved; `nil` keeps nothing between launches.
    ///   - delay: how long the lists stay still before Spotlight is told.
    ///   - retry: how long the first failure holds off the next pass; each failure after it
    ///     doubles that, up to `longestRetry`.
    ///   - covers: where covers come from; the store's artwork by default.
    public init(
        store: AppStore,
        index: any SpotlightIndex = SystemSpotlightIndex(),
        prefix: String = SpotlightIndexer.prefix(),
        file: URL?,
        defaults: UserDefaults = .standard,
        delay: Duration = .seconds(1),
        retry: Duration = .seconds(60),
        covers: (any SpotlightCovers)? = nil
    ) {
        self.store = store
        self.index = index
        self.prefix = prefix
        self.file = file
        self.defaults = defaults
        self.delay = delay
        self.retry = retry
        self.covers = covers ?? store.artwork
        defaults.register(defaults: [Self.preference: true])
        saved = file.flatMap(Shelf.load)
    }

    /// The prefix for this app: none for `app.kaeri.app`, and the rest of another's bundle
    /// id after `app.kaeri.`: `fixtures.`.
    public nonisolated static func prefix(bundle: Bundle = .main) -> String {
        prefix(bundleIdentifier: bundle.bundleIdentifier)
    }

    nonisolated static func prefix(bundleIdentifier: String?) -> String {
        guard let id = bundleIdentifier, id != "app.kaeri.app" else { return "" }
        let name = id.hasPrefix("app.kaeri.") ? String(id.dropFirst("app.kaeri.".count)) : id
        return name + "."
    }

    /// Whether titles go to Spotlight.
    public var isOn: Bool { defaults.bool(forKey: Self.preference) }

    /// The shelf as last told to Spotlight.
    public var shelf: Shelf? { saved }

    /// Watch the store, and the switch, from now on.
    public func start() {
        if CSSearchableIndex.isIndexingAvailable() == false {
            AppLog.info("spotlight", "indexing is not available here")
            return
        }
        observe()
        defaultsObserver = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification, object: defaults, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.schedule() }
        }
        schedule()
    }

    /// Note what the shelf is made of, and the covers on their way, and look again once any of
    /// it changes. Only the newest watch looks again, so watches do not pile up.
    private func observe() {
        generation += 1
        let watch = generation
        withObservationTracking {
            _ = Shelf.current(store, over: saved ?? Shelf())
            for url in asked { _ = covers.image(url) }
        } onChange: { [weak self] in
            Task { @MainActor in
                guard let self else { return }
                // Every change asks for a pass, an older watch's too: a pass that ends between
                // the change and this task sets the newer watch after the change, and that
                // watch never sees it.
                self.schedule()
                guard self.generation == watch else { return }
                self.observe()
            }
        }
    }

    /// Tell Spotlight once the lists have been still for `delay`, and not before a failure's
    /// wait is over.
    func schedule() {
        scheduled?.cancel()
        var wait = delay
        if let resumeAt { wait = max(wait, resumeAt - .now) }
        scheduled = Task { [weak self, wait] in
            try? await Task.sleep(for: wait)
            guard !Task.isCancelled, let self else { return }
            // Only the wait is called off by a later change: a pass under way is not, or what
            // it was telling Spotlight would stop halfway. The change is the next pass's.
            self.scheduled = nil
            await self.pass()
        }
    }

    /// Bring Spotlight to the shelf as it stands. One at a time: a change during a pass is
    /// taken by another pass right after it.
    func pass() async {
        guard !running else {
            again = true
            return
        }
        running = true
        defer { running = false }
        repeat {
            again = false
            await update()
        } while again
        // The covers asked for in the pass are watched from now on.
        observe()
    }

    private func update() async {
        // After a failure, nothing until its wait is over: schedule() comes back then.
        if let resumeAt, ContinuousClock.now < resumeAt { return }
        guard isOn else {
            if saved != nil || !cleared {
                do {
                    try await index.deleteAll(domains: SpotlightItem.domains(prefix: prefix))
                    cleared = true
                    saved = nil
                    if let file { Shelf.remove(file) }
                    succeeded()
                    onUpdate?()
                } catch {
                    await failed("could not clear the index", error)
                }
            }
            return
        }
        // Nothing an answered list says yet, and nothing saved: wait, rather than clear.
        let before = saved ?? Shelf()
        var shelf = Shelf.current(store, over: before)
        if saved == nil, shelf.titles.isEmpty, !store.continuing.list.state.isAnswered { return }

        do {
            if saved == nil, !cleared {
                try await index.deleteAll(domains: SpotlightItem.domains(prefix: prefix))
                cleared = true
            }
            let gone = Set(before.titles.keys).subtracting(shelf.titles.keys)
            if !gone.isEmpty {
                try await index.delete(gone.sorted().map(SpotlightItem.identifier))
            }
            var items: [SpotlightItem] = []
            var thumbnails: [AnilistId: Data] = [:]
            for id in shelf.order {
                guard let title = shelf.titles[id] else { continue }
                let changed = saved == nil || before.titles[id] != title
                // Indexed again, an item is replaced whole: its cover goes with it again.
                if changed { shelf.covered.remove(id) }
                var thumbnail: Data?
                if !shelf.covered.contains(id), let url = title.coverUrl, !missed.contains(url) {
                    if let image = covers.image(url) {
                        thumbnail = await Self.jpeg(Picture(image: image))
                        thumbnails[id] = thumbnail
                        asked.remove(url)
                    } else if asked.contains(url) {
                        // Answered with nothing: the plate stays, and it is not asked again.
                        if !covers.isPending(url) {
                            asked.remove(url)
                            missed.insert(url)
                        }
                    } else if asked.count < Self.coversAtOnce {
                        asked.insert(url)
                        covers.request(cover: url)
                    }
                }
                guard changed || thumbnail != nil else { continue }
                items.append(SpotlightItem(title, prefix: prefix, thumbnail: thumbnail))
            }
            if !items.isEmpty {
                try await index.index(items)
            }
            shelf.covered.formUnion(thumbnails.keys)
            saved = shelf
            if let file { try shelf.save(file) }
            succeeded()
            if !items.isEmpty || !gone.isEmpty {
                AppLog.info("spotlight", "indexed \(items.count), removed \(gone.count), \(shelf.titles.count) in all")
                onUpdate?()
            }
        } catch {
            await failed("could not update the index", error)
        }
    }

    /// The longest a run of failures holds off the next pass.
    nonisolated static let longestRetry: Duration = .seconds(3600)

    /// How long the `failures`th failure in a row holds off the next pass: `retry`, doubled
    /// for each failure before it, `longestRetry` at most.
    nonisolated static func wait(after failures: Int, retry: Duration) -> Duration {
        min(retry * (1 << min(max(failures - 1, 0), 16)), longestRetry)
    }

    /// Whether Spotlight refused the connection itself (CSIndexErrorDomain -1003). On macOS it
    /// does so while more than one copy of the app signed with a certificate is known to the
    /// system: the installed app and a mounted disk image, or a local build. It then takes the
    /// app for an ad hoc one that claims a signed app's bundle id, and refuses every call until
    /// the other copies are gone; nothing the app sends can change that.
    nonisolated static func isTurnedAway(_ error: any Error) -> Bool {
        let error = error as NSError
        return error.domain == CSIndexErrorDomain && error.code == CSIndexError.Code.remoteConnectionError.rawValue
    }

    /// Whether this session has looked for the copies Spotlight weighs the app against.
    private var lookedForCopies = false

    /// Hold off the next pass, for longer with each failure in a row, and log the failure
    /// unless it is the one logged last.
    private func failed(_ what: String, _ error: any Error) async {
        failures += 1
        let wait = Self.wait(after: failures, retry: retry)
        resumeAt = .now + wait
        let message = "\(what): \(error.localizedDescription)"
        if message != reported {
            reported = message
            AppLog.error("spotlight", "\(message); trying again in \(wait.components.seconds) s, and later each time it fails")
        }
        // Turned away, the cause is outside the app: say once which copies cause it. The
        // passes go on trying, further apart, and the first one after the copies are gone
        // goes through.
        if Self.isTurnedAway(error), !lookedForCopies {
            lookedForCopies = true
            let copies = await index.otherSignedCopies()
            if !copies.isEmpty {
                AppLog.error(
                    "spotlight",
                    "Spotlight turns this copy of the app away while another signed copy is on this Mac: "
                        + copies.joined(separator: ", ")
                        + ". Eject the disk image or remove the other copies, and titles go to Spotlight again."
                )
            }
        }
        // A pass when the wait is over, whether or not the lists change by then.
        schedule()
    }

    private func succeeded() {
        if failures > 0 {
            AppLog.info("spotlight", "the index takes titles again, after \(failures) failed passes")
        }
        failures = 0
        resumeAt = nil
        reported = nil
    }

    /// A cover drawn for the index: JPEG, its long side `coverPixels` at most.
    @concurrent nonisolated static func jpeg(_ picture: Picture) async -> Data? {
        let image = picture.image
        let scale = min(1, Double(coverPixels) / Double(max(image.width, image.height)))
        let width = max(1, Int((Double(image.width) * scale).rounded()))
        let height = max(1, Int((Double(image.height) * scale).rounded()))
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
        ) else { return nil }
        context.interpolationQuality = .high
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
        guard let small = context.makeImage() else { return nil }
        let data = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(data, UTType.jpeg.identifier as CFString, 1, nil)
        else { return nil }
        CGImageDestinationAddImage(destination, small, [kCGImageDestinationLossyCompressionQuality: 0.8] as CFDictionary)
        guard CGImageDestinationFinalize(destination) else { return nil }
        return data as Data
    }

    /// A decoded cover, carried off the main actor to be drawn small. A CGImage does not change
    /// once made.
    struct Picture: @unchecked Sendable {
        let image: CGImage
    }
}

extension ListState {
    /// Whether the engine has answered: with titles, or that there are none.
    var isAnswered: Bool {
        switch self {
        case .loaded, .empty: true
        case .idle, .loading, .failed: false
        }
    }
}

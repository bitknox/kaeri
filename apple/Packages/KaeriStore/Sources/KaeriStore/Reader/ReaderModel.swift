// The open reader: one session at a time, what the engine said of it, and what the screens ask.

import KaeriKit
import KaeriStrings
import Foundation
import Observation

/// A page of a chapter as the engine names it. A split spread has a file for each half.
public struct ReaderPageKey: Hashable, Sendable {
    public let chapter: String
    public let page: UInt32
    public let half: PageHalf?

    public init(chapter: String, page: UInt32, half: PageHalf? = nil) {
        self.chapter = chapter
        self.page = page
        self.half = half
    }
}

/// What the end page says when there is nothing to continue to.
public struct ReaderEnd: Equatable, Sendable {
    public var next: ChapterRow?
    public var why: NoNext?
}

/// The notice over the page, with the action it offers.
public struct ReaderNoticeLine: Equatable, Sendable {
    public let id: Int
    public let text: String
    public let action: ReaderAction?
}

@Observable @MainActor
public final class ReaderModel {
    public enum Phase: Equatable, Sendable {
        case closed
        /// Asked for; the engine has not answered.
        case opening
        case open
        /// The chapter would not open.
        case failed(ReadingFailure)
    }

    public private(set) var phase: Phase = .closed
    /// The window or cover shows while this is set; the work and chapter asked for.
    public private(set) var request: Request?
    public private(set) var view: ReaderView?
    public private(set) var leaves: [Leaf] = []
    public private(set) var pages: [ReaderPageKey: PageFile] = [:]
    public private(set) var failures: [ReaderPageKey: ReadingFailure] = [:]
    public private(set) var novel: NovelOpenView?
    public private(set) var novelStyle = ""
    public private(set) var end: ReaderEnd?
    public private(set) var settings: ReaderSettingsView?
    public internal(set) var notice: ReaderNoticeLine?
    /// "Back to page 8", waiting in the corner.
    public internal(set) var backTo: ReaderNoticeLine?
    /// The leaf shown now.
    public internal(set) var current = 0
    /// Where the person was before the last jump, as a leaf, for the ring on the scrubber.
    public private(set) var ring: Int?
    /// Where a strip is asked to scroll to once its images are in.
    public private(set) var stripRestore: StripPos?
    /// The chapters of the work, once the engine has listed them.
    public private(set) var chapters: [VolumeGroup] = []
    public private(set) var chaptersLoaded = false
    /// When the open began, for the opening line's wait.
    public private(set) var openedAt: ContinuousClock.Instant?
    /// Whether the chapter counted as read.
    public private(set) var read = false
    /// A place the paged reader is told to show (Back to page, Start over, another device's).
    public private(set) var goTo: (leaf: Int, tick: Int)?
    /// A novel place the web view is told to show.
    public private(set) var novelGoTo: (pos: NovelPos, tick: Int)?
    /// The sheet the screen shows: settings at This series, or the chapters.
    public var sheet: Sheet?
    /// Whether the control group shows.
    public var chromeShown = false
    /// A screen reader is running: the group stays shown, as the board has it.
    public var pinChrome = false {
        didSet { if pinChrome { chromeShown = true } }
    }

    /// A turn or a scroll hides the group, unless a sheet is up or a screen reader needs it.
    public func hideChrome() {
        guard !pinChrome, sheet == nil, chromeShown else { return }
        chromeShown = false
    }

    /// The paginator has laid a page out and said where it is.
    public private(set) var novelPainted = false
    /// Screenshots only: pages stay unloaded, to show the plate.
    public var holdPages = false
    /// Screenshots only: the scrubber starts held at this index.
    public var shotScrub: Int?

    /// Screenshots only: the phase held as a state the engine rarely stays in.
    func hold(_ phase: Phase) {
        self.phase = phase
        openedAt = .now
    }

    /// Go to page is open.
    public var askingPage = false
    /// Paper mode the person's Focus turned on, for this open.
    public var focusPaper = false

    public enum Sheet: Equatable, Sendable { case settings, chapters, more }

    public struct Request: Equatable, Sendable {
        public var work: String
        public var chapter: String?
        public var at: OpenAt
        public var tick: Int
    }

    @ObservationIgnored public var send: (@MainActor (Command) -> Void)?
    /// The engine's synchronous answer for the web view's scheme handler.
    @ObservationIgnored public var bookResource: (@Sendable (String) -> BookResource?)?
    @ObservationIgnored private var surface: ReaderSurface?
    @ObservationIgnored private var sentSurface: ReaderSurface?
    @ObservationIgnored private var session: UInt64 = 0
    @ObservationIgnored private var noticeCount = 0
    @ObservationIgnored private var noticeTask: Task<Void, Never>?
    @ObservationIgnored private var backTask: Task<Void, Never>?
    @ObservationIgnored private var ticks = 0
    @ObservationIgnored private var lastStrip: ContinuousClock.Instant?
    @ObservationIgnored private var pendingStrip: Task<Void, Never>?
    @ObservationIgnored private var rereadAfterClose: Request?

    public init() {
        focusPaper = Self.defaults.bool(forKey: Self.focusKey)
        focusWatch = NotificationCenter.default.addObserver(
            forName: UserDefaults.didChangeNotification, object: nil, queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.focusChanged() }
        }
    }

    /// The Focus filter's switch: it is written by the app's intent and read here.
    public nonisolated static let focusKey = "reader.focusPaper"
    nonisolated(unsafe) static var defaults = UserDefaults.standard
    @ObservationIgnored private var focusWatch: NSObjectProtocol?

    private func focusChanged() {
        let on = Self.defaults.bool(forKey: Self.focusKey)
        guard on != focusPaper else { return }
        focusPaper = on
        // Said once, over the page, when the Focus turns Paper mode on under a reader.
        guard on, request != nil else { return }
        let line = ReaderNoticeLine(id: bump(), text: L.readerFocusPaper, action: nil)
        notice = line
        noticeTask?.cancel()
        noticeTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(4))
            guard !Task.isCancelled, self?.notice?.id == line.id else { return }
            self?.notice = nil
        }
    }

    // MARK: Asking

    /// Open `work` at `chapter`, or where the person stopped. The screen that shows it reports
    /// its size with `surfaceChanged`, and the engine hears of the open then.
    public func open(work: String, chapter: String? = nil, at: OpenAt = .resume) {
        let next = Request(work: work, chapter: chapter, at: at, tick: bump())
        if session != 0 {
            // One at a time: the first closes, then this opens.
            send?(.reader(.close(session: session)))
            rereadAfterClose = next
            reset()
            request = nil
            return
        }
        begin(next)
    }

    private func begin(_ next: Request) {
        reset()
        request = next
        phase = .opening
        openedAt = .now
        sendOpen()
        send?(.reader(.loadSettings(medium: .manga, work: WorkId(next.work))))
    }

    private func sendOpen() {
        guard let request, let surface, phase == .opening, session == 0, !opened else { return }
        opened = true
        send?(.reader(.open(
            work: WorkId(request.work), chapter: request.chapter.map { ChapterId($0) },
            at: request.at, surface: surface)))
        send?(.reading(.loadChapters(work: WorkId(request.work), edition: nil)))
    }

    @ObservationIgnored private var opened = false

    /// The reader's screen has a size: the engine lays out for it, and opens on it the first time.
    public func surfaceChanged(_ surface: ReaderSurface) {
        guard self.surface != surface else { return }
        self.surface = surface
        if request != nil, !opened {
            sendOpen()
            return
        }
        guard session != 0, sentSurface != surface else { return }
        sentSurface = surface
        send?(.reader(.surface(session: session, surface: surface)))
    }

    public func close() {
        guard request != nil || session != 0 else { return }
        if session != 0 { send?(.reader(.close(session: session))) }
        reset()
        request = nil
        phase = .closed
    }

    /// A leaf was painted.
    public func shown(_ leaf: Int, how: TurnHow) {
        guard session != 0, let chapter = view?.chapter.id, leaves.indices.contains(leaf) else { return }
        if how == .jump, leaf != current { ring = current }
        current = leaf
        if how == .turn || how == .swipe || how == .key { hideChrome() }
        if end != nil, !leaves[leaf].endPage { end = end }
        send?(.reader(.shown(session: session, chapter: chapter, leaf: UInt32(leaves[leaf].index), how: how)))
    }

    /// A strip settled: the image and how far down it, at most every 500 ms.
    public func stripAt(_ at: StripPos, footVisible: Bool) {
        guard session != 0, let chapter = view?.chapter.id else { return }
        pendingStrip?.cancel()
        let now = ContinuousClock.now
        let send = { [weak self] in
            guard let self else { return }
            lastStrip = .now
            self.send?(.reader(.stripAt(session: session, chapter: chapter, at: at, footVisible: footVisible)))
        }
        if let lastStrip, now - lastStrip < .milliseconds(500) {
            pendingStrip = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(500) - (now - lastStrip))
                guard !Task.isCancelled, self != nil else { return }
                send()
            }
        } else {
            send()
        }
    }

    /// The paginator turned a page.
    public func novelAt(_ pos: NovelPos, page: Int, pages: Int, how: TurnHow) {
        guard session != 0, let chapter = view?.chapter.id else { return }
        novelPainted = true
        send?(.reader(.novelAt(
            session: session, chapter: chapter, pos: pos, page: UInt32(max(page, 0)),
            pages: UInt32(max(pages, 0)), how: how)))
    }

    public func openChapter(_ id: ChapterId) {
        guard session != 0 else { return }
        end = nil
        read = false
        send?(.reader(.openChapter(session: session, chapter: id)))
        phase = .opening
        openedAt = .now
        sheet = nil
    }

    /// A strip reached its foot and the next chapter follows: it opens without the opening screen,
    /// so the reader goes on from the divider.
    public func flowOn() {
        guard session != 0, let next = view?.next else { return }
        end = nil
        read = false
        send?(.reader(.openChapter(session: session, chapter: next.id)))
    }

    public func retry(page: UInt32?) {
        guard session != 0, let chapter = view?.chapter.id else { return }
        if let page { failures = failures.filter { $0.key.page != page } }
        send?(.reader(.retry(session: session, chapter: chapter, page: page)))
        if page == nil { phase = .opening; openedAt = .now }
    }

    public func needPage(_ page: UInt32) {
        guard session != 0, let chapter = view?.chapter.id else { return }
        send?(.reader(.needPage(session: session, chapter: chapter, page: page)))
    }

    public func act(_ action: ReaderAction) {
        guard session != 0 else { return }
        switch action {
        case .startOver: send?(.reader(.startOver(session: session)))
        case .backTo: send?(.reader(.backTo(session: session)))
        case .goBack: send?(.reader(.goBack(session: session)))
        case .changeMode: sheet = .settings
        }
        if action != .changeMode { dismissNotice(action) }
    }

    /// The person went back to the page; the pill leaves.
    public func dismissNotice(_ action: ReaderAction?) {
        if action == .backTo { backTo = nil; backTask?.cancel() } else { notice = nil; noticeTask?.cancel() }
    }

    /// A turn the menu or a key asked for; the reader on screen answers it.
    public struct TurnRequest: Equatable, Sendable {
        public var forward: Bool
        public var line: Bool
        public var tick: Int
    }

    public private(set) var turnRequest: TurnRequest?

    /// Where the reader on screen is: its leaf, image or page, and how many there are. The
    /// scrubber and the group line read it.
    public var position = (index: 0, count: 1)

    public private(set) var jump: (index: Int, tick: Int)?

    /// The scrubber or Go to page asked for `index`; the reader on screen answers it.
    public func requestJump(_ index: Int) {
        jump = (index, bump())
    }

    public func requestTurn(forward: Bool, line: Bool = false) {
        turnRequest = TurnRequest(forward: forward, line: line, tick: bump())
    }

    // MARK: Settings

    public func row(_ key: ReaderSettingKey) -> ReaderSettingRow? {
        settings?.rows.first { $0.key == key }
    }

    public func choice(_ key: ReaderSettingKey) -> String? {
        if case .choice(let name)? = row(key)?.value { return name }
        return nil
    }

    public func amount(_ key: ReaderSettingKey) -> Double? {
        if case .amount(let value)? = row(key)?.value { return value }
        return nil
    }

    public func isOn(_ key: ReaderSettingKey) -> Bool {
        if case .toggle(let on)? = row(key)?.value { return on }
        return false
    }

    public var medium: Medium { view?.medium ?? .manga }

    /// Change a setting for the medium's default, or for the series with `.series`.
    public func set(_ key: ReaderSettingKey, _ value: ReaderValue, scope: SettingScope? = nil) {
        let scope = scope ?? Self.homeScope(key)
        let work = (scope == .series || scope == .seriesHere) ? view.map { $0.work } : nil
        send?(.reader(.setSetting(
            session: session == 0 ? nil : session, medium: medium, work: work, key: key, scope: scope, value: value)))
    }

    public func useDefault(_ key: ReaderSettingKey) {
        guard let work = view?.work else { return }
        send?(.reader(.useDefault(medium: medium, work: work, key: key)))
    }

    /// The reading mode picker: Auto drops the series' choice; the others set mode and direction
    /// for the series, which follows the person.
    public func chooseMode(_ mode: ReadingModeChoice) {
        switch mode {
        case .auto:
            useDefault(.mode)
            useDefault(.direction)
        case .rightToLeft, .leftToRight:
            set(.mode, .choice(name: "paged"), scope: .series)
            set(.direction, .choice(name: mode == .rightToLeft ? "rtl" : "ltr"), scope: .series)
        case .strip:
            set(.mode, .choice(name: "strip"), scope: .series)
        }
    }

    /// Whether the series has a choice of its own for the reading mode.
    public var modeOverridden: Bool {
        (row(.mode)?.overridden ?? false) || (row(.direction)?.overridden ?? false)
    }

    public enum ReadingModeChoice: Sendable { case auto, rightToLeft, leftToRight, strip }

    /// Where a setting is kept when the sheet changes it: the choices follow the person, the
    /// layout stays on the device.
    nonisolated public static func homeScope(_ key: ReaderSettingKey) -> SettingScope {
        switch key {
        case .mode, .direction, .colour, .look, .font: .medium
        default: .device
        }
    }

    // MARK: Reading what the engine said

    /// Called with every event the store applies.
    public func observe(_ event: Event) {
        switch event {
        case .reader(let event): apply(event)
        case .reading(.chapters(let work, _, let volumes, _)):
            guard work == view?.work || request.map({ WorkId($0.work) }) == work else { return }
            chapters = volumes
            chaptersLoaded = true
        default: break
        }
    }

    func apply(_ event: ReaderEvent) {
        switch event {
        case .opened(let opened):
            session = opened.session
            view = opened
            phase = .open
            leaves = []
            pages = [:]
            failures = [:]
            novel = nil
            end = nil
            read = false
            ring = nil
            backTo = nil
            notice = nil
            current = 0
            if let settings = opened.settings { self.settings = settings }
            send?(.reader(.loadSettings(medium: opened.medium, work: opened.work)))
            if opened.mode == .strip { stripRestore = opened.start.strip }
        case .leaves(let session, _, let leaves):
            guard session == self.session else { return }
            self.leaves = leaves
            phase = .open
            if !leaves.isEmpty { current = min(max(current, startLeaf(leaves)), leaves.count - 1) }
            if let goTo = pendingGoTo, goTo < leaves.count { show(goTo) }
        case .pageReady(let session, let chapter, let page, let half, let file):
            guard session == self.session else { return }
            let key = ReaderPageKey(chapter: chapter, page: page, half: half)
            pages[key] = file
            failures[key] = nil
            if half == nil { failures[ReaderPageKey(chapter: chapter, page: page)] = nil }
        case .pageFailed(let session, let chapter, let page, let failure):
            guard session == self.session else { return }
            if leaves.isEmpty, page == 0 {
                phase = .failed(failure)
            }
            failures[ReaderPageKey(chapter: chapter, page: page)] = failure
        case .novelOpen(let open):
            guard open.session == session else { return }
            novel = open
            novelStyle = open.style
            phase = .open
            if let restore = open.restore { novelGoTo = (restore, bump()) }
        case .novelStyle(let session, let style, _, _):
            guard session == self.session else { return }
            novelStyle = style
        case .chapterRead(_, let chapter, _):
            if chapter == view?.chapter.id { read = true }
        case .endPage(let session, let next, let why, _):
            guard session == self.session else { return }
            end = ReaderEnd(next: next, why: why)
        case .nextPreview, .opening:
            // The Android reader's end page preview and opening screen; this reader has neither.
            break
        case .notice(let notice):
            show(notice)
        case .moved(let session, _, _, _):
            guard session == self.session else { return }
            // The notice that comes with it says where, and offers the way back.
        case .settings(let view):
            // Settings' Reading pane asks for each medium's defaults: an open book keeps its own.
            if let open = self.view, view.medium != open.medium { return }
            settings = view
        case .goTo(let session, let place):
            guard session == self.session else { return }
            goTo(place)
        case .closed(let session):
            guard session == self.session || self.session == 0 else { return }
            self.session = 0
            opened = false
            if let next = rereadAfterClose {
                rereadAfterClose = nil
                begin(next)
            }
        }
    }

    private var pendingGoTo: Int?

    private func startLeaf(_ leaves: [Leaf]) -> Int {
        guard let page = view?.start.page else { return 0 }
        return leaves.firstIndex { $0.parts.contains { $0.page >= page } } ?? 0
    }

    private func goTo(_ place: ReadPlace) {
        if let pos = place.novel {
            novelGoTo = (pos, bump())
            return
        }
        if let image = place.strip {
            stripRestore = image
            self.goTo = (Int(image.image), bump())
            return
        }
        guard let page = place.page else { return }
        if let index = leaves.firstIndex(where: { $0.parts.contains { $0.page >= page } }) {
            show(index)
        } else {
            pendingGoTo = Int(page)
        }
    }

    private func show(_ leaf: Int) {
        pendingGoTo = nil
        current = leaf
        goTo = (leaf, bump())
    }

    private func show(_ incoming: ReaderNotice) {
        noticeCount += 1
        let line = ReaderNoticeLine(id: noticeCount, text: incoming.msg.text, action: incoming.action)
        if incoming.action == .backTo {
            backTo = line
            backTask?.cancel()
            if incoming.lastsMs > 0 {
                backTask = Task { [weak self] in
                    try? await Task.sleep(for: .milliseconds(Int(incoming.lastsMs)))
                    guard !Task.isCancelled else { return }
                    self?.backTo = nil
                }
            }
            return
        }
        notice = line
        noticeTask?.cancel()
        if incoming.lastsMs > 0 {
            noticeTask = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(Int(incoming.lastsMs)))
                guard !Task.isCancelled else { return }
                if self?.notice?.id == line.id { self?.notice = nil }
            }
        }
    }

    func bump() -> Int {
        ticks += 1
        return ticks
    }

    private func reset() {
        noticeTask?.cancel()
        backTask?.cancel()
        pendingStrip?.cancel()
        view = nil
        leaves = []
        pages = [:]
        failures = [:]
        novel = nil
        novelPainted = false
        novelStyle = ""
        end = nil
        notice = nil
        backTo = nil
        current = 0
        ring = nil
        stripRestore = nil
        goTo = nil
        novelGoTo = nil
        read = false
        chapters = []
        chaptersLoaded = false
        sheet = nil
        chromeShown = false
        opened = false
        pendingGoTo = nil
        lastStrip = nil
        phase = .closed
    }

    // MARK: Derived

    /// The pages of the chapter, as the leaves count them.
    public var pageCount: Int {
        let last = leaves.compactMap { leaf in leaf.parts.map(\.page).max() }.max() ?? 0
        return leaves.isEmpty ? 0 : Int(last) + 1
    }

    /// The page number, from 1, a leaf starts at.
    public func pageNumber(of leaf: Int) -> Int {
        guard leaves.indices.contains(leaf), let page = leaves[leaf].parts.map(\.page).min() else { return leaf + 1 }
        return Int(page) + 1
    }

    /// The leaf a page, from 1, is on.
    public func leaf(ofPage number: Int) -> Int {
        let page = UInt32(max(number - 1, 0))
        return leaves.firstIndex { $0.parts.contains { $0.page >= page } } ?? max(leaves.count - 1, 0)
    }

    /// The file a part shows, once it has landed.
    public func file(of part: LeafPart, chapter: String) -> PageFile? {
        if holdPages { return nil }
        return part.file ?? pages[ReaderPageKey(chapter: chapter, page: part.page, half: part.half)]
            ?? pages[ReaderPageKey(chapter: chapter, page: part.page)]
    }

    public func failure(of part: LeafPart, chapter: String) -> ReadingFailure? {
        failures[ReaderPageKey(chapter: chapter, page: part.page)]
    }

    public var chapterTitle: String {
        guard let chapter = view?.chapter else { return "" }
        return chapter.label
    }

}

extension ReadingFailure {
    /// The failure in plain words, from the engine's own messages.
    public var words: String {
        let msg: ReadingMsg = switch self {
        case .offline: .readingFailedOffline
        case .sourceDown(let source): .readingFailedSourceDown(source: source)
        case .notFound: .readingFailedNotFound
        case .blocked(let source, let reason): .readingFailedBlocked(source: source, reason: reason)
        case .parse(let source): .readingFailedParse(source: source)
        case .pageFetch(let status): .readingFailedPage(status: Int64(status))
        case .signInNeeded(let what): .readingFailedSignIn(what: what)
        case .fileMissing(let path): .readingFailedFileMissing(path: path)
        case .unreadable(let path, let format): .readingFailedUnreadable(path: path, format: format)
        case .unsupported(let kind): .readingFailedUnsupported(kind: kind)
        case .protected: .readingFailedProtected
        case .diskFull: .readingFailedDiskFull
        case .webViewTooOld(let version): .readingFailedWebview(version: Int64(version))
        case .needsNewerKaeri(let world): .readingFailedNeedsNewer(world: world)
        case .readingOff: .readingFailedOff
        case .other(let reason): .readingFailedOther(reason: reason)
        // The send checks carry sizes, which the engine puts at human scale.
        case .sendTooLarge, .sendCheck: readingFailureMsg(failure: self)
        }
        return msg.text
    }
}

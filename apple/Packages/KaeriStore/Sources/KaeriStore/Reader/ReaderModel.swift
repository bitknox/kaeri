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
    /// The source the next chapter comes from, by name: "from books.home.example". None for a
    /// file in a folder.
    public var nextSource: String?
}

/// The notice over the page, with the action it offers.
public struct ReaderNoticeLine: Equatable, Sendable {
    public let id: Int
    public let text: String
    public let action: ReaderAction?

    public init(id: Int, text: String, action: ReaderAction?) {
        self.id = id
        self.text = text
        self.action = action
    }
}

/// The chapters a strip ran on from, kept above the open one so the scroll goes on: each one's
/// images with their files, and the marker into the chapter after it.
public struct StripTrail: Equatable, Sendable {
    public struct Chapter: Equatable, Sendable {
        public let id: String
        public let label: String
        public let parts: [LeafPart]
        /// The chapter it ran into, which its marker names.
        public let next: ChapterRow
        public let nextSource: String?
    }

    public var chapters: [Chapter] = []
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
    /// The next chapter's first page, made while this one is read, for the page between chapters.
    public private(set) var previews: [ChapterId: PageFile] = [:]
    public private(set) var settings: ReaderSettingsView?
    public internal(set) var notice: ReaderNoticeLine?
    /// "Back to page 8", waiting in the corner.
    public internal(set) var backTo: ReaderNoticeLine?
    /// The reader is out of sight: the app is inactive or in the background. Notices wait.
    public var away = false
    /// VoiceOver rests on the notice, or the pointer is over it. It waits.
    public var noticeHeld = false
    private var noticesPaused: Bool { away || noticeHeld }
    /// The chapters a strip ran on from, above the open one.
    public private(set) var trail = StripTrail()
    /// The strip is scrolled back above the open chapter's marker, into the trail.
    public var inTrail: StripTrail.Chapter?
    /// The leaf shown now.
    public internal(set) var current = 0
    /// Where the person was before the last jump, as a leaf, for the ring on the scrubber.
    public private(set) var ring: Int?
    /// Where a strip is asked to scroll to once its images are in.
    public private(set) var stripRestore: StripPos?
    /// Where the strip is, as it last said: kept through a second layout of the chapter.
    @ObservationIgnored private var stripHere: StripPos?
    /// Counts the open chapter's layouts after its first, so the strip puts the place back even
    /// when it has not moved since the last restore.
    public private(set) var stripRelaid = 0
    /// The chapters of the work, once the engine has listed them.
    public private(set) var chapters: [VolumeGroup] = []
    public private(set) var chaptersLoaded = false
    /// When the open began, for the opening line's wait.
    public private(set) var openedAt: ContinuousClock.Instant?
    /// What the open waits on, as the engine last said: the chapter list or the chapter's
    /// pages, from which source. `nil` before it says, and for a chapter on this device.
    public private(set) var openingWait: ReaderWait?
    /// Whether the chapter counted as read.
    public private(set) var read = false
    /// The trackers the read went to, by name: "Read · AniList updated".
    public private(set) var readTrackers: [String] = []
    /// The reader closed: the page between chapters' server lines are forgotten.
    @ObservationIgnored var onClosed: (() -> Void)?
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

    /// The end page a paged chapter was left from, as it was when Continue was pressed (or a
    /// swipe past it, or a chapter picked on it). The screen keeps it over the next chapter until
    /// that chapter's first page has drawn, then crossfades it away: never a cut to an empty
    /// screen.
    public struct HeldEnd {
        public let tick: Int
        public let view: ReaderView
        public let end: ReaderEnd?
        public let read: Bool
        public let readTrackers: [String]
        public let previews: [ChapterId: PageFile]
        /// The chapter being opened under it.
        public let to: ChapterId
        /// The first page is slower than the opening's 600 ms: a small cue shows on the held page.
        public internal(set) var slow = false
        /// The first page has drawn, or the chapter failed: the held page can go.
        public internal(set) var landed = false
    }

    public private(set) var held: HeldEnd?
    /// How long the held page waits before it shows its cue: the opening plate's 600 ms.
    @ObservationIgnored var heldCue: Duration = .milliseconds(600)
    @ObservationIgnored private var heldTask: Task<Void, Never>?
    /// The next chapter's pages that have drawn while the end page is held over them.
    @ObservationIgnored private var heldPainted: Set<UInt32> = []

    /// A page of the open chapter is on screen. The held end page lands once every part of the
    /// leaf that shows has drawn.
    public func pagePainted(chapter: String, page: UInt32) {
        guard held != nil, chapter == view?.chapter.id else { return }
        heldPainted.insert(page)
        settleHeld()
    }

    /// The held page has faded away; a newer hold is kept.
    public func releaseHeld(tick: Int) {
        guard held?.tick == tick else { return }
        dropHeld()
    }

    private func holdEnd(to id: ChapterId) {
        dropHeld()
        guard phase == .open, novel == nil, let view, view.mode != .strip,
              leaves.indices.contains(current), leaves[current].endPage else { return }
        let tick = bump()
        held = HeldEnd(
            tick: tick, view: view, end: end, read: read, readTrackers: readTrackers, previews: previews, to: id)
        let cue = heldCue
        heldTask = Task { [weak self] in
            try? await Task.sleep(for: cue)
            guard !Task.isCancelled, let self, held?.tick == tick, held?.landed == false else { return }
            held?.slow = true
        }
    }

    private func dropHeld() {
        heldTask?.cancel()
        heldTask = nil
        heldPainted = []
        held = nil
    }

    /// Whether the chapter under the held end page is ready to show: its first page has drawn,
    /// or it failed, or it opened as something with no pages to wait for.
    private func settleHeld() {
        guard let held, !held.landed else { return }
        let ready: Bool
        if case .failed = phase {
            ready = true
        } else if phase != .open || view?.chapter.id != held.to {
            ready = false
        } else if novel != nil {
            ready = true
        } else if leaves.indices.contains(current) {
            let chapter = held.to
            ready = leaves[current].parts.allSatisfy { part in
                heldPainted.contains(part.page) || failures[ReaderPageKey(chapter: chapter, page: part.page)] != nil
            }
        } else {
            ready = false
        }
        guard ready else { return }
        heldTask?.cancel()
        self.held?.landed = true
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
        clearNotices()
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
    /// Closed after the open was sent and before the engine said it opened.
    @ObservationIgnored private var closedOpening = false

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
        noticeTask = countdown(holdFor(4_000, text: line.text, action: false)) { [weak self] in
            if self?.notice?.id == line.id { self?.notice = nil }
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
            // The reader stays up, opening, while the first book closes.
            request = next
            phase = .opening
            return
        }
        begin(next)
    }

    private func begin(_ next: Request) {
        reset()
        closedOpening = false
        request = next
        phase = .opening
        openedAt = .now
        openingWait = nil
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

    #if FIXTURES
    /// For shots: the chapter counts as read, sent to `trackers`.
    public func stageRead(trackers: [String]) {
        read = true
        readTrackers = trackers
    }

    /// For shots: the next chapter comes from `source`, as a server's chapter does.
    public func stageNextSource(_ source: String) {
        end = ReaderEnd(next: end?.next ?? view?.next, why: end?.why, nextSource: source)
    }
    #endif

    public func close() {
        // A book waiting on the one before to close is dropped with it: closing always closes.
        rereadAfterClose = nil
        guard request != nil || session != 0 else { return }
        if session != 0 {
            send?(.reader(.close(session: session)))
        } else if opened {
            // Asked for, not opened yet: the session the engine opens is closed as it comes.
            closedOpening = true
        }
        onClosed?()
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
        stripHere = at
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

    /// The chapter opened by going back from the first page of the one after it: it shows its
    /// last page, or the top of its last image in a strip, once its leaves come.
    private var openAtEnd: ChapterId?

    /// Open a chapter of the book; `atEnd` shows its last page, for going back into it.
    public func openChapter(_ id: ChapterId, atEnd: Bool = false) {
        guard session != 0 else { return }
        openAtEnd = atEnd ? id : nil
        holdEnd(to: id)
        end = nil
        read = false
        readTrackers = []
        trail = StripTrail()
        inTrail = nil
        flowing = nil
        clearNotices()
        send?(.reader(.openChapter(session: session, chapter: id)))
        phase = .opening
        openedAt = .now
        openingWait = nil
        sheet = nil
    }

    /// A strip's marker into the next chapter crossed the middle of the screen: that chapter
    /// opens in the same scroll, under the marker, with no opening screen. The images read so
    /// far stay above it.
    public func flowOn() {
        guard session != 0, flowing == nil, let view, view.mode == .strip, let next = end?.next ?? view.next else { return }
        let chapter = view.chapter.id
        let parts = leaves.filter { !$0.endPage }.flatMap(\.parts).map { part in
            var part = part
            if part.file == nil { part.file = file(of: part, chapter: chapter) }
            return part
        }
        trail.chapters.append(StripTrail.Chapter(
            id: chapter, label: view.chapter.label, parts: parts, next: next, nextSource: end?.nextSource))
        flowing = next.id
        end = nil
        read = false
        readTrackers = []
        send?(.reader(.openChapter(session: session, chapter: next.id)))
    }

    /// The chapter a strip is flowing into, until it has opened.
    public private(set) var flowing: ChapterId?

    public func retry(page: UInt32?) {
        // An open that failed before the engine named a session is asked for again whole.
        if session == 0, page == nil, let request {
            begin(Request(work: request.work, chapter: request.chapter, at: request.at, tick: bump()))
            return
        }
        // A strip that failed to flow on tries the chapter it was flowing into.
        guard session != 0, let chapter = flowing ?? view?.chapter.id else { return }
        if let page { failures = failures.filter { $0.key.page != page } }
        send?(.reader(.retry(session: session, chapter: chapter, page: page)))
        if page == nil { phase = .opening; openedAt = .now; clearNotices() }
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

    /// Notices belong to an open chapter: opening and failing screens show none.
    private func clearNotices() {
        noticeTask?.cancel()
        backTask?.cancel()
        notice = nil
        backTo = nil
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
        case .reading(.failed(.reader, let failure)):
            // The open failed: the chapter, the source or the book. The reader says so, with
            // Try again and Close, rather than wait on an open that will not come. A strip that
            // was flowing on, with the chapter before still on screen, says it the same way.
            guard request != nil else { return }
            phase = .failed(failure)
            clearNotices()
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
            // Closed while it opened: the engine's session goes too.
            if closedOpening {
                closedOpening = false
                send?(.reader(.close(session: opened.session)))
                return
            }
            // A strip that flowed on keeps the chapters above; any other open starts clean.
            if flowing != opened.chapter.id || opened.mode != .strip {
                trail = StripTrail()
                inTrail = nil
            }
            flowing = nil
            session = opened.session
            view = opened
            phase = .open
            leaves = []
            pages = [:]
            failures = [:]
            novel = nil
            end = nil
            previews = [:]
            read = false
            readTrackers = []
            ring = nil
            backTo = nil
            notice = nil
            current = 0
            if let settings = opened.settings { self.settings = settings }
            send?(.reader(.loadSettings(medium: opened.medium, work: opened.work)))
            stripHere = nil
            if opened.mode == .strip { stripRestore = opened.start.strip }
        case .leaves(let session, let chapter, let leaves):
            guard session == self.session else { return }
            // A source's chapter is laid out again once all its pages have landed, when their
            // shapes differ from the guess: a strip keeps the place it is at, under the same
            // image, rather than move with the heights above it.
            let relaid = !self.leaves.isEmpty && chapter == view?.chapter.id && view?.mode == .strip && novel == nil
            self.leaves = leaves
            if relaid, let here = stripHere {
                stripRestore = here
                stripRelaid += 1
            }
            phase = .open
            if !leaves.isEmpty { current = min(max(current, startLeaf(leaves)), leaves.count - 1) }
            if let goTo = pendingGoTo, goTo < leaves.count { show(goTo) }
            if let chapter = openAtEnd, chapter == view?.chapter.id, !leaves.isEmpty {
                openAtEnd = nil
                showEnd(of: leaves)
            }
            settleHeld()
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
                clearNotices()
            }
            failures[ReaderPageKey(chapter: chapter, page: page)] = failure
            settleHeld()
        case .novelOpen(let open):
            guard open.session == session else { return }
            novel = open
            novelStyle = open.style
            phase = .open
            if let restore = open.restore { novelGoTo = (restore, bump()) }
            settleHeld()
        case .novelStyle(let session, let style, _, _):
            guard session == self.session else { return }
            novelStyle = style
        case .chapterRead(_, let chapter, let trackers):
            if chapter == view?.chapter.id {
                read = true
                readTrackers = trackers
            }
        case .endPage(let session, let next, let why, let source):
            guard session == self.session else { return }
            end = ReaderEnd(next: next, why: why, nextSource: source ?? end?.nextSource)
        case .progressSent:
            // The page between chapters' server line: AppStore hands it to ServerProgressModel.
            break
        case .nextPreview(let session, let chapter, let file):
            guard session == self.session else { return }
            previews[chapter] = file
        case .opening(_, let wait):
            // The opening line says what the open waits on.
            guard phase == .opening || phase == .closed else { return }
            openingWait = wait
        case .notice(let notice):
            // A notice belongs to the chapter on screen; the opening and failing screens have none.
            guard phase == .open else { return }
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

    /// The chapter's last page, before its end page; in a strip, the top of its last image.
    private func showEnd(of leaves: [Leaf]) {
        if view?.mode == .strip, novel == nil {
            let images = leaves.filter { !$0.endPage }.reduce(0) { $0 + $1.parts.count }
            stripRestore = StripPos(image: UInt32(max(images - 1, 0)), offset: 0)
            return
        }
        current = leaves.lastIndex { !$0.endPage } ?? leaves.count - 1
    }

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
        let hold = holdFor(incoming.lastsMs, text: line.text, action: incoming.action != nil)
        if incoming.action == .backTo {
            backTo = line
            backTask?.cancel()
            backTask = countdown(hold) { [weak self] in
                if self?.backTo?.id == line.id { self?.backTo = nil }
            }
            return
        }
        notice = line
        noticeTask?.cancel()
        noticeTask = countdown(hold) { [weak self] in
            if self?.notice?.id == line.id { self?.notice = nil }
        }
    }

    /// How long a notice stays, by the engine's rule: longer for long text, and until it is
    /// dismissed when it carries a button and VoiceOver runs. Nil when it waits.
    private func holdFor(_ lastsMs: UInt32, text: String, action: Bool) -> UInt32? {
        let words = text.split(whereSeparator: \.isWhitespace).count
        let hold = readerNoticeHold(lastsMs: lastsMs, words: UInt32(words), hasAction: action, screenReader: pinChrome)
        return hold == 0 ? nil : hold
    }

    /// Counts `ms` down, only while notices are not paused, then calls `done`. Nothing for nil.
    private func countdown(_ ms: UInt32?, done: @escaping @MainActor () -> Void) -> Task<Void, Never>? {
        guard let ms else { return nil }
        return Task { [weak self] in
            var left = Int(ms)
            while left > 0 {
                try? await Task.sleep(for: .milliseconds(100))
                guard !Task.isCancelled, let self else { return }
                if !self.noticesPaused { left -= 100 }
            }
            done()
        }
    }

    func bump() -> Int {
        ticks += 1
        return ticks
    }

    private func reset() {
        dropHeld()
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
        previews = [:]
        notice = nil
        backTo = nil
        current = 0
        ring = nil
        stripRestore = nil
        goTo = nil
        novelGoTo = nil
        read = false
        readTrackers = []
        chapters = []
        chaptersLoaded = false
        sheet = nil
        chromeShown = false
        opened = false
        pendingGoTo = nil
        lastStrip = nil
        trail = StripTrail()
        inTrail = nil
        flowing = nil
        phase = .closed
    }

    // MARK: Derived

    /// The pages of the chapter, as the leaves count them.
    public var pageCount: Int {
        let last = leaves.compactMap { leaf in leaf.parts.map(\.page).max() }.max() ?? 0
        return leaves.isEmpty ? 0 : Int(last) + 1
    }

    /// The page number, from 1, a leaf starts at. The end page, and a leaf past what is laid
    /// out, count as the last page: a leaf's own index runs past the pages wherever a spread was
    /// split in two, so it never stands in for a page number.
    public func pageNumber(of leaf: Int) -> Int {
        let last = max(pageCount, 1)
        guard leaves.indices.contains(leaf), let page = leaves[leaf].parts.map(\.page).min() else {
            return leaves.isEmpty ? leaf + 1 : last
        }
        return min(Int(page) + 1, last)
    }

    /// The leaf at `index` is the end page, between this chapter and the next.
    public func isEndPage(_ index: Int) -> Bool {
        leaves.indices.contains(index) && leaves[index].endPage
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
        case .sourceOff(let source): .readingFailedSourceOff(source: source)
        case .notFound: .readingFailedNotFound
        case .noSource: .readingFailedNoSource
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

    /// The failure as the cause of a send that failed: offline says to try again online rather
    /// than the reader's line about downloaded chapters.
    public var sendWords: String {
        if case .offline = self { return L.sendFailedOffline }
        return words
    }
}

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
    /// When `next` is an extra: the first whole chapter after it, for "Skip to chapter 17".
    public var skipTo: ChapterRow? = nil
    /// How many extras the skip passes over.
    public var extras: UInt32 = 0

    /// The chapter an end page goes on to: the engine's word once it has one, the view's next
    /// chapter until then. Offline the engine leaves out a next chapter that is not on the device.
    public static func next(_ end: ReaderEnd?, view: ReaderView?) -> ChapterRow? {
        end != nil ? end?.next : view?.next
    }

    /// What a screen reader adds after the skip: the extra's number, or with a run of extras or
    /// one with no number, that the extras stay unread.
    public var skipHint: String? {
        guard skipTo != nil, let next else { return nil }
        if extras > 1 || next.number == nil { return L.readerEndSkipHintExtras }
        return L.readerEndSkipHint(chapter: next.number ?? next.label)
    }
}

/// The notice over the page, with the action it offers: the engine's, or one of the app's own.
public struct ReaderNoticeLine: Equatable, Sendable {
    public let id: Int
    public let text: String
    public let action: ReaderAction?
    public let local: ReaderLocalAction?

    public init(id: Int, text: String, action: ReaderAction?, local: ReaderLocalAction? = nil) {
        self.id = id
        self.text = text
        self.action = action
        self.local = local
    }
}

/// What a notice's button does when the app answers it, not the engine.
public enum ReaderLocalAction: Equatable, Sendable {
    /// Not a credit page, for the credit page on screen.
    case keepCredit
    /// Undo, after Not a credit page.
    case undoKeep
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
    public internal(set) var pages: [ReaderPageKey: PageFile] = [:]
    public internal(set) var failures: [ReaderPageKey: ReadingFailure] = [:]
    public private(set) var novel: NovelOpenView?
    public private(set) var novelStyle = ""
    public private(set) var end: ReaderEnd?
    /// The chapter the end page goes on to.
    public var endNext: ChapterRow? { ReaderEnd.next(end, view: view) }
    /// The next chapter's first page, made while this one is read, for the page between chapters.
    public private(set) var previews: [ChapterId: PageFile] = [:]
    public private(set) var settings: ReaderSettingsView?
    public internal(set) var notice: ReaderNoticeLine?
    /// "Back to page 8" in the slider bar: the page the engine offers to go back to after the
    /// person's own jump of more than two pages, counted from 1.
    public internal(set) var backToPage: Int?
    /// The reader is out of sight: the app is inactive or in the background. Notices wait.
    /// A page kept as not a credit page goes to the engine as the reader leaves sight: the app
    /// may be ended in the background before its Undo runs out.
    public var away = false {
        didSet { if away, !oldValue { commitKeep() } }
    }
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
    @ObservationIgnored var stripHere: StripPos?
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
    /// A place the paged reader is told to show (Back to page, Start over, another device's, the
    /// page kept across a change of mode), and how it got there: the app's own move, never the
    /// person's jump.
    public private(set) var goTo: (leaf: Int, tick: Int, how: TurnHow)?
    /// A novel place the web view is told to show (Back to page, Start over, another
    /// device's); the novel opens at its own restore.
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
        if page == 0, chapter == view?.chapter.id { startCreditLine() }
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
    /// The window the book is open in has the keys, or no window of the app does. Another of the
    /// app's windows in front keeps its own keys: the Go menu binds the reader's only while this
    /// holds. The Mac's reader follows its window; on iPad and iPhone it stays true.
    public var windowInFront = true
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
    @ObservationIgnored var session: UInt64 = 0
    @ObservationIgnored var noticeCount = 0
    @ObservationIgnored var noticeTask: Task<Void, Never>?
    @ObservationIgnored private var ticks = 0
    @ObservationIgnored private var lastStrip: ContinuousClock.Instant?
    @ObservationIgnored private var pendingStrip: Task<Void, Never>?
    /// The strip report `pendingStrip` will send.
    @ObservationIgnored private var heldStrip: (@MainActor () -> Void)?
    /// Whether the last strip report had the chapter's foot in sight.
    @ObservationIgnored private var lastFoot = false
    @ObservationIgnored private var rereadAfterClose: Request?

    // MARK: Credit pages and parts (what they do is in ReaderCredits.swift)

    /// The credit pages the engine left out, by chapter: the open one's, and those of the
    /// chapters a strip ran on from, which stay above it.
    public internal(set) var credits: [String: [CreditPage]] = [:]
    /// The open chapter's parts, when its source lists it in parts the reader runs through as one.
    public internal(set) var parts: ReaderParts?
    /// The credit pages the person brought back. They stay until their chapter closes.
    public internal(set) var revealed: Set<CreditKey> = []
    /// The credit page the paged reader shows in place of its leaf, by its page in the chapter.
    public internal(set) var onCredit: UInt32?
    /// The quiet line on page 1 is up: it holds 4 seconds and leaves with the first turn.
    public internal(set) var creditLine = false
    /// Not a credit page was used: the page is kept once its Undo has run out.
    public internal(set) var keeping: CreditKeep?
    /// The chapter whose line has shown, so it shows once each time the chapter opens.
    @ObservationIgnored var creditLineFor: String?
    @ObservationIgnored var creditLineTask: Task<Void, Never>?
    @ObservationIgnored var keepTask: Task<Void, Never>?
    /// A kept page's chapter is laid out again with it: the pages after it move up one.
    @ObservationIgnored var relayKeep: CreditKeep?
    /// Books closed after their open was sent and before the engine said they opened, whose
    /// close the engine has not answered yet.
    @ObservationIgnored private var staleOpens = 0
    /// The chapter asked for in the open book, until it opens.
    private var asked: ChapterId?

    /// The chapter the opening screen names: the one asked for while it opens, not the one on
    /// screen that it is left from.
    public var openingChapter: ChapterRow? {
        guard let asked, view?.chapter.id != asked else { return view?.chapter }
        return chapterRow(asked)
    }

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
        if session == 0, opened, phase == .opening {
            // A book still opening is dropped, as a close would drop it.
            staleOpens += 1
            send?(.reader(.close(session: 0)))
        }
        if session != 0 {
            // One at a time: the first closes, then this opens.
            flushStrip()
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
        sentSurface = surface
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
        end = ReaderEnd(
            next: end?.next ?? view?.next, why: end?.why, nextSource: source, skipTo: end?.skipTo,
            extras: end?.extras ?? 0
        )
    }

    /// For shots: the end of chapter 16, whose next chapter is an extra, 16.5, with chapter 17
    /// after it to skip to. The extra's first page is one of the open chapter's pages.
    /// The labels and title are a shot's made-up chapter data, marked `i18n-skip`.
    public func stageSkipExtra() {
        guard var next = end?.next ?? view?.next else { return }
        view?.chapter.number = "16"
        view?.chapter.label = "Chapter 16" // i18n-skip
        next.number = "16.5"
        next.label = "Chapter 16.5" // i18n-skip
        next.title = "Volume 1 extras" // i18n-skip
        let skip = ChapterRow(
            id: ChapterId("shot-skip-17"),
            label: "Chapter 17", // i18n-skip
            state: .unread,
            number: "17"
        )
        if previews[next.id] == nil,
           let first = pages.sorted(by: { $0.key.page < $1.key.page }).first?.value {
            previews[next.id] = first
        }
        end = ReaderEnd(next: next, why: end?.why, nextSource: end?.nextSource, skipTo: skip, extras: 1)
    }
    #endif

    public func close() {
        // A book waiting on the one before to close is dropped with it: closing always closes.
        rereadAfterClose = nil
        guard request != nil || session != 0 else { return }
        commitKeep()
        // A strip place held back by the pacing is the place shown at the close.
        flushStrip()
        if session != 0 {
            send?(.reader(.close(session: session)))
        } else if opened, phase == .opening {
            // Asked for, not opened yet: the engine drops the open still waiting, and a session
            // it opened before hearing so is closed as it comes, whatever opens next.
            staleOpens += 1
            send?(.reader(.close(session: 0)))
        }
        onClosed?()
        reset()
        request = nil
        phase = .closed
    }

    /// A leaf was painted.
    public func shown(_ leaf: Int, how: TurnHow) {
        guard session != 0, let chapter = view?.chapter.id, leaves.indices.contains(leaf) else { return }
        // A turn off the credit page keeps a page waiting on its Undo, and the line on page 1 goes.
        leftCredit()
        if leaf != 0 { endCreditLine() }
        if how == .jump, leaf != current, view?.mode != .strip { ring = current }
        // A strip's scroll report from before the jump would read as a turn after it.
        if how == .jump { dropHeldStrip() }
        if leaf != current { heldPage = nil }
        current = leaf
        if how == .turn || how == .swipe || how == .key { hideChrome() }
        if end != nil, !leaves[leaf].endPage { end = end }
        send?(.reader(.shown(session: session, chapter: chapter, leaf: UInt32(leaves[leaf].index), how: how)))
    }

    /// A strip settled: the image and how far down it, at most every 500 ms.
    public func stripAt(_ at: StripPos, footVisible: Bool) {
        guard session != 0, let chapter = view?.chapter.id else { return }
        // The foot of a chapter whose last image has not landed is only where a guess put it: it
        // starts no read.
        let footVisible = footVisible && stripEndIn
        stripHere = at
        dropHeldStrip()
        let now = ContinuousClock.now
        let session = session
        let send: @MainActor () -> Void = { [weak self] in
            guard let self else { return }
            lastStrip = .now
            lastFoot = footVisible
            self.send?(.reader(.stripAt(session: session, chapter: chapter, at: at, footVisible: footVisible)))
        }
        // The foot coming into sight goes at once: it starts the read rule, and the strip may
        // flow into the next chapter before a held report would go.
        if let lastStrip, now - lastStrip < .milliseconds(500), footVisible == lastFoot {
            heldStrip = send
            pendingStrip = Task { [weak self] in
                try? await Task.sleep(for: .milliseconds(500) - (now - lastStrip))
                guard !Task.isCancelled, let self else { return }
                flushStrip()
            }
        } else {
            send()
        }
    }

    /// The strip report the pacing holds back goes now: before the reader closes or leaves the
    /// chapter, so the engine keeps the place shown last.
    private func flushStrip() {
        guard let send = heldStrip else { return }
        dropHeldStrip()
        send()
    }

    private func dropHeldStrip() {
        pendingStrip?.cancel()
        pendingStrip = nil
        heldStrip = nil
    }

    /// The paginator turned a page.
    public func novelAt(_ pos: NovelPos, page: Int, pages: Int, how: TurnHow) {
        guard session != 0, let chapter = view?.chapter.id else { return }
        novelPainted = true
        novelHere = NovelHere(session: session, chapter: chapter, pos: pos, pages: pages)
        send?(.reader(.novelAt(
            session: session, chapter: chapter, pos: pos, page: UInt32(max(page, 0)),
            pages: UInt32(max(pages, 0)), how: how)))
    }

    /// The novel's end page showed, past the last page of its last document. The engine hears
    /// it as the page after the last, which counts the chapter read at once, as a paged
    /// chapter's end page does.
    public func novelEnded() {
        guard session != 0, let chapter = view?.chapter.id, let here = novelHere,
              here.session == session, here.chapter == chapter
        else { return }
        let pages = UInt32(max(here.pages, 1))
        send?(.reader(.novelAt(session: session, chapter: chapter, pos: here.pos, page: pages, pages: pages, how: .turn)))
    }

    /// The last page the paginator reported: the end page goes to the engine from there.
    private struct NovelHere {
        var session: UInt64
        var chapter: ChapterId
        var pos: NovelPos
        var pages: Int
    }

    @ObservationIgnored private var novelHere: NovelHere?

    /// The chapter opened by going back from the first page of the one after it: it shows its
    /// last page, or the top of its last image in a strip, once its leaves come.
    private var openAtEnd: ChapterId?

    /// Open a chapter of the book; `atEnd` shows its last page, for going back into it.
    public func openChapter(_ id: ChapterId, atEnd: Bool = false) {
        guard session != 0 else { return }
        commitKeep()
        flushStrip()
        asked = id
        openAtEnd = atEnd ? id : nil
        holdEnd(to: id)
        end = nil
        read = false
        readTrackers = []
        trail = StripTrail()
        inTrail = nil
        flowing = nil
        flowFrom = nil
        flowFailure = nil
        clearNotices()
        send?(.reader(.openChapter(session: session, chapter: id)))
        phase = .opening
        openedAt = .now
        openingWait = nil
        sheet = nil
    }

    /// A strip's marker into the next chapter crossed the middle of the screen: that chapter
    /// opens in the same scroll, under the marker, with no opening screen. The images read so
    /// far stay above it. Only once the chapter's end is where the strip draws it: a chapter whose
    /// last image has not landed is never passed, since opening the next chapter counts this one
    /// as read.
    public func flowOn() {
        guard session != 0, flowing == nil, flowFailure == nil, view?.mode == .strip, stripEndIn,
              let next = endNext else { return }
        flow(into: next, .reader(.openChapter(session: session, chapter: next.id)))
    }

    /// The chapter read goes into the trail and `next` opens under it with `command`.
    private func flow(into next: ChapterRow, _ command: Command) {
        guard let view else { return }
        commitKeep()
        flushStrip()
        let chapter = view.chapter.id
        let parts = leaves.filter { !$0.endPage }.flatMap(\.parts).map { part in
            var part = part
            if part.file == nil { part.file = file(of: part, chapter: chapter) }
            return part
        }
        trail.chapters.append(StripTrail.Chapter(
            id: chapter, label: view.chapter.label, parts: parts, next: next, nextSource: end?.nextSource))
        flowFrom = FlowFrom(next: next, end: end, read: read, readTrackers: readTrackers)
        flowing = next.id
        end = nil
        read = false
        readTrackers = []
        send?(command)
    }

    /// The chapter a strip is flowing into, until it has opened.
    public private(set) var flowing: ChapterId?

    /// A chapter a strip could not flow into, and why.
    public struct FlowFailure: Equatable, Sendable {
        public let next: ChapterRow
        public let failure: ReadingFailure
    }

    /// The chapter the strip could not flow into. The chapter read stays open, and the strip
    /// ends under it on its end page, which names that chapter and why, with Try again.
    public private(set) var flowFailure: FlowFailure?

    /// What the chapter read had when the strip flowed on from it, put back when the flow fails.
    private struct FlowFrom {
        let next: ChapterRow
        let end: ReaderEnd?
        let read: Bool
        let readTrackers: [String]
    }

    @ObservationIgnored private var flowFrom: FlowFrom?

    /// The chapter the strip was flowing into would not open: the chapter read comes back out of
    /// the trail as the open one, its rows where they were, and its end page says why.
    private func flowFailed(_ failure: ReadingFailure) {
        guard flowing != nil else { return }
        let from = flowFrom
        var next = from?.next
        if let chapter = view?.chapter.id, let last = trail.chapters.last, last.id == chapter {
            next = next ?? last.next
            trail.chapters.removeLast()
            if inTrail?.id == chapter { inTrail = nil }
        }
        self.flowing = nil
        flowFrom = nil
        end = end ?? from?.end
        // The engine may have said the chapter read was read as the flow began.
        read = read || from?.read == true
        if readTrackers.isEmpty { readTrackers = from?.readTrackers ?? [] }
        // With no next chapter known, the end page is the plain one.
        if let next = next ?? endNext { flowFailure = FlowFailure(next: next, failure: failure) }
    }

    /// Try again on the end page of a strip that could not flow on: the strip flows into that
    /// chapter again. The engine's session is on it already, so it is asked for again there.
    public func retryFlow() {
        guard session != 0, flowing == nil, view?.mode == .strip, let failed = flowFailure else { return }
        flowFailure = nil
        flow(into: failed.next, .reader(.retry(session: session, chapter: failed.next.id, page: nil)))
    }

    /// Where the strip puts the place once its images are in: where it was asked to go, or else
    /// where the chapter opened. A place kept only as a page, as when the chapter was last read
    /// page by page, is the top of the image that holds that page, the one the Resumed notice
    /// names.
    public var stripStart: StripPos? {
        if let stripRestore { return stripRestore }
        guard let start = view?.start else { return nil }
        if let strip = start.strip { return strip }
        guard let page = start.page else { return nil }
        let parts = leaves.filter { !$0.endPage }.flatMap(\.parts)
        guard !parts.isEmpty else { return nil }
        let image = parts.firstIndex { $0.page >= page } ?? parts.count - 1
        return StripPos(image: UInt32(image), offset: 0)
    }

    /// An image of `chapter` is in: its shape known from the book, its file landed, or its
    /// failure said. Until then its height in a strip is a guess.
    public func isIn(_ part: LeafPart, chapter: String) -> Bool {
        (part.ratio ?? 0) > 0 || file(of: part, chapter: chapter) != nil
            || failure(of: part, chapter: chapter) != nil
    }

    /// The open chapter's last image is in. Until then its foot is only where a guess put it:
    /// the chapter is not passed, and its foot is not seen.
    public var stripEndIn: Bool {
        guard let chapter = view?.chapter.id,
              let last = leaves.last(where: { !$0.endPage && !$0.parts.isEmpty })?.parts.last else { return false }
        return isIn(last, chapter: chapter)
    }

    public func retry(page: UInt32?) {
        // An open that failed before the engine named a session is asked for again whole.
        if session == 0, page == nil, let request {
            begin(Request(work: request.work, chapter: request.chapter, at: request.at, tick: bump()))
            return
        }
        // A strip that could not flow on tries the chapter it was flowing into, under the one read.
        if page == nil, flowFailure != nil {
            retryFlow()
            return
        }
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
        case .goBack: send?(.reader(.goBack(session: session)))
        case .changeMode: sheet = .settings
        }
        if action != .changeMode { dismissNotice(action) }
    }

    /// The notice's own button, which the app answers.
    public func act(_ action: ReaderLocalAction) {
        switch action {
        case .keepCredit: keepCredit()
        case .undoKeep: undoKeep()
        }
    }

    /// The notice's button was used; the notice leaves.
    public func dismissNotice(_ action: ReaderAction?) {
        notice = nil
        noticeTask?.cancel()
    }

    /// Back to page: the place before the jump. The engine takes the offer away as it goes.
    public func back() {
        guard session != 0, backToPage != nil else { return }
        backToPage = nil
        send?(.reader(.backTo(session: session)))
    }

    /// Notices belong to an open chapter: opening and failing screens show none.
    private func clearNotices() {
        noticeTask?.cancel()
        notice = nil
        backToPage = nil
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
            // was flowing on keeps the chapter read open and says it on that chapter's end page,
            // whatever the reason: the reader is never replaced by the chapter it could not open.
            // The answer to a book closed while it opened belongs to that book, not this one.
            // With reading off, the close is answered with a failure rather than `closed`.
            if staleOpens > 0 {
                if case .readingOff = failure { staleOpens -= 1 }
                return
            }
            guard request != nil else { return }
            if flowing != nil, phase == .open {
                flowFailed(failure)
                return
            }
            phase = .failed(failure)
            clearNotices()
            // The failure names the chapter that would not open, not the one it was left from:
            // the engine sends no `opened` for it.
            if let id = asked ?? flowing, view?.chapter.id != id, let row = chapterRow(id) {
                view?.chapter = row
            }
            // A held end page goes, so the failure and its buttons show and take taps.
            settleHeld()
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
            // Closed while it opened: the engine's session goes too. The engine answers the
            // close of the open before anything of a book asked for after it, so whatever opens
            // until then is the closed book's.
            if staleOpens > 0 {
                send?(.reader(.close(session: opened.session)))
                return
            }
            asked = nil
            // A strip that flowed on keeps the chapters above; any other open starts clean.
            if flowing != opened.chapter.id || opened.mode != .strip {
                trail = StripTrail()
                inTrail = nil
                // A chapter opened again skips its credit pages again.
                credits = [:]
                revealed = []
            }
            parts = nil
            onCredit = nil
            endCreditLine()
            creditLineFor = nil
            flowing = nil
            flowFrom = nil
            flowFailure = nil
            session = opened.session
            // The screen changed size while the book opened: the engine laid it out for the
            // size the open carried, so it hears of the new one now the session exists.
            if let surface, sentSurface != surface {
                sentSurface = surface
                send?(.reader(.surface(session: opened.session, surface: surface)))
            }
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
            backToPage = nil
            notice = nil
            current = 0
            heldPage = nil
            if let settings = opened.settings { self.settings = settings }
            send?(.reader(.loadSettings(medium: opened.medium, work: opened.work)))
            stripHere = nil
            if opened.mode == .strip { stripRestore = opened.start.strip }
        case .leaves(let session, let chapter, let leaves):
            guard session == self.session else { return }
            // The credit pages and parts come again after every layout.
            credits[chapter] = nil
            if chapter == view?.chapter.id { parts = nil }
            // A layout with a kept page sets the page to stay on, before the page shown is taken.
            relaidWithKept(chapter)
            // A source's chapter is laid out again once all its pages have landed, when their
            // shapes differ from the guess: a strip keeps the place it is at, under the same
            // image, rather than move with the heights above it.
            let again = !self.leaves.isEmpty && chapter == view?.chapter.id && novel == nil
            let relaid = again && view?.mode == .strip
            // Pages laid out again, as when the screen turns between single pages and spreads:
            // the page on screen stays, in whichever leaf holds it now, and so does the ring's.
            let paged = again && view?.mode == .paged && keepPage == nil
            let shownPage = paged ? pageKept(in: current) : nil
            let ringPage = paged ? ring.flatMap(firstPage(of:)) : nil
            let atEnd = paged && self.leaves.indices.contains(current) && self.leaves[current].endPage
            self.leaves = leaves
            if relaid, let here = stripHere {
                stripRestore = here
                stripRelaid += 1
            }
            phase = .open
            if let shownPage, let index = Self.leaf(holding: shownPage, in: leaves) {
                current = index
                // The page stays the one kept while its leaf is on screen, so a spread that
                // holds it gives it back when the pages are single again.
                heldPage = shownPage
                ring = ringPage.flatMap { Self.leaf(holding: $0, in: leaves) }
            } else if atEnd, let end = leaves.lastIndex(where: \.endPage) {
                current = end
            } else if !leaves.isEmpty {
                current = min(max(current, startLeaf(leaves)), leaves.count - 1)
            }
            if let goTo = pendingGoTo, goTo < leaves.count { show(goTo) }
            if let chapter = openAtEnd, chapter == view?.chapter.id, !leaves.isEmpty {
                openAtEnd = nil
                showEnd(of: leaves)
            }
            if let page = keepPage, let index = leaves.firstIndex(where: { $0.parts.contains { $0.page >= page } }) {
                keepPage = nil
                show(index)
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
            settleHeld()
        case .novelStyle(let session, let style, _, _):
            guard session == self.session else { return }
            novelStyle = style
        case .chapterRead(_, let chapter, let trackers):
            if chapter == view?.chapter.id {
                read = true
                readTrackers = trackers
            }
        case .endPage(let session, let next, let why, let source, let skipTo, let extras):
            guard session == self.session else { return }
            end = ReaderEnd(
                next: next, why: why, nextSource: source ?? end?.nextSource, skipTo: skipTo,
                extras: extras
            )
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
            // A mode or direction changed in the reader lays the chapter out again without
            // opening it anew: the view follows, so the tap zones and the slider turn with it.
            if let open = self.view, view.work == open.work {
                let followed = Self.following(open, view)
                // The page on screen stays on screen in the other mode.
                if followed.mode != open.mode, let page = pageOnScreen(open.mode) {
                    if followed.mode == .strip {
                        stripRestore = StripPos(image: page, offset: 0)
                    } else {
                        keepPage = page
                    }
                }
                self.view = followed
            }
        case .backToPage(let session, let page):
            guard session == self.session else { return }
            backToPage = page.map(Int.init)
        case .goTo(let session, let place):
            guard session == self.session else { return }
            goTo(place)
        case .creditPages(let session, let chapter, let pages, _):
            guard session == self.session else { return }
            creditsCame(chapter, pages)
        case .chapterParts(let session, let chapter, let parts, let total, let missing):
            guard session == self.session, chapter == view?.chapter.id else { return }
            self.parts = ReaderParts(parts: parts, total: total, missing: missing)
        case .closed(let session):
            // Session 0 answers a close sent while a book opened: its answers have all come.
            if session == 0 {
                staleOpens = max(staleOpens - 1, 0)
                return
            }
            // A close of a closed book's session that opened late changes nothing here.
            guard session == self.session else { return }
            self.session = 0
            opened = false
            if let next = rereadAfterClose {
                rereadAfterClose = nil
                begin(next)
            }
        }
    }

    private var pendingGoTo: Int?
    /// The page a switch from the strip to pages keeps, until the pages are laid out.
    @ObservationIgnored var keepPage: UInt32?
    /// The page a layout kept, while the leaf that holds it stays on screen: a spread's second
    /// page shown alone before the spread.
    @ObservationIgnored private var heldPage: UInt32?

    /// The page on screen in `mode`: the first of the spread, or the strip's image at the top.
    private func pageOnScreen(_ mode: ReaderMode) -> UInt32? {
        switch mode {
        case .paged:
            return firstPage(of: current)
        default:
            return inTrail == nil ? UInt32(max(position.index, 0)) : nil
        }
    }

    /// The open book's view with the mode and direction `settings` give, as the engine lays the
    /// chapter out again with them. A novel's pages are its own.
    nonisolated static func following(_ view: ReaderView, _ settings: ReaderSettingsView) -> ReaderView {
        guard view.medium != .novel, settings.medium == view.medium else { return view }
        func word(_ key: ReaderSettingKey) -> String? {
            if case .choice(let name)? = settings.rows.first(where: { $0.key == key })?.value { return name }
            return nil
        }
        var view = view
        switch word(.direction) {
        case "ltr": view.direction = .leftToRight
        case "rtl": view.direction = .rightToLeft
        default: break
        }
        switch word(.mode) {
        case "strip": view.mode = .strip
        case "paged": view.mode = .paged
        default: break
        }
        return view
    }

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
        return Self.leaf(holding: page, in: leaves) ?? 0
    }

    /// The page a new layout keeps of `leaf`: the one shown alone before a spread came to hold
    /// it, while that spread is still on screen; otherwise the leaf's first.
    private func pageKept(in leaf: Int) -> UInt32? {
        if let held = heldPage, leaves.indices.contains(leaf), leaves[leaf].parts.contains(where: { $0.page == held }) {
            return held
        }
        return firstPage(of: leaf)
    }

    /// The first page a leaf shows: the lower of a spread's two.
    private func firstPage(of leaf: Int) -> UInt32? {
        leaves.indices.contains(leaf) ? leaves[leaf].parts.map(\.page).min() : nil
    }

    /// The leaf that shows `page`, or the first after it when no leaf does.
    nonisolated static func leaf(holding page: UInt32, in leaves: [Leaf]) -> Int? {
        leaves.firstIndex { $0.parts.contains { $0.page >= page } }
    }

    private func goTo(_ place: ReadPlace) {
        if let pos = place.novel {
            novelGoTo = (pos, bump())
            return
        }
        if let image = place.strip {
            stripRestore = image
            self.goTo = (Int(image.image), bump(), .placed)
            return
        }
        guard let page = place.page else { return }
        // A place by page alone, in a strip: the top of the image that holds the page.
        if view?.mode == .strip, novel == nil {
            let parts = leaves.filter { !$0.endPage }.flatMap(\.parts)
            let image = parts.firstIndex { $0.page >= page } ?? Int(page)
            stripRestore = StripPos(image: UInt32(image), offset: 0)
            self.goTo = (image, bump(), .placed)
            return
        }
        if let index = leaves.firstIndex(where: { $0.parts.contains { $0.page >= page } }) {
            show(index)
        } else {
            pendingGoTo = Int(page)
        }
    }

    func show(_ leaf: Int, how: TurnHow = .placed) {
        pendingGoTo = nil
        if leaf != current { heldPage = nil }
        current = leaf
        goTo = (leaf, bump(), how)
    }

    private func show(_ incoming: ReaderNotice) {
        noticeCount += 1
        let line = ReaderNoticeLine(id: noticeCount, text: incoming.msg.text, action: incoming.action)
        let hold = holdFor(incoming.lastsMs, text: line.text, action: incoming.action != nil)
        notice = line
        noticeTask?.cancel()
        noticeTask = countdown(hold) { [weak self] in
            if self?.notice?.id == line.id { self?.notice = nil }
        }
    }

    /// How long a notice stays, by the engine's rule: longer for long text, and until it is
    /// dismissed when it carries a button and VoiceOver runs. Nil when it waits.
    func holdFor(_ lastsMs: UInt32, text: String, action: Bool) -> UInt32? {
        let words = text.split(whereSeparator: \.isWhitespace).count
        let hold = readerNoticeHold(lastsMs: lastsMs, words: UInt32(words), hasAction: action, screenReader: pinChrome)
        return hold == 0 ? nil : hold
    }

    /// Counts `ms` down, only while notices are not paused, then calls `done`. Nothing for nil.
    func countdown(_ ms: UInt32?, done: @escaping @MainActor () -> Void) -> Task<Void, Never>? {
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
        dropHeldStrip()
        asked = nil
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
        backToPage = nil
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
        keepPage = nil
        heldPage = nil
        lastStrip = nil
        credits = [:]
        parts = nil
        revealed = []
        onCredit = nil
        endCreditLine()
        creditLineFor = nil
        keepTask?.cancel()
        keeping = nil
        relayKeep = nil
        trail = StripTrail()
        inTrail = nil
        flowing = nil
        flowFrom = nil
        flowFailure = nil
        phase = .closed
    }

    // MARK: Derived

    /// The row of a chapter of the open book: a neighbour the view or end page names, or one
    /// from the listed chapters.
    private func chapterRow(_ id: ChapterId) -> ChapterRow? {
        let near = [view?.next, view?.previous, end?.next, end?.skipTo, held?.end?.next, held?.end?.skipTo]
        if let row = near.lazy.compactMap({ $0 }).first(where: { $0.id == id }) { return row }
        return chapters.lazy.flatMap(\.chapters).first { $0.id == id }
    }

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
        // The engine words these: sizes at human scale, a message per write problem.
        case .sendTooLarge, .sendCheck, .cannotWrite: readingFailureMsg(failure: self)
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

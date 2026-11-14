// Credit pages and chapter parts in the reader: the pages a group repeats at the start or end of
// its chapters, which the engine leaves out, and a chapter the source lists in parts, which reads
// as one. The engine counts without the credit pages; the app shows them again when asked, and
// keeps one as a page with Not a credit page, which waits out its Undo before the engine hears.

import KaeriKit
import KaeriStrings
import Foundation

/// The open chapter's parts: one count of pages, with a tick where each later part starts.
public struct ReaderParts: Equatable, Sendable {
    public let parts: [ChapterPart]
    public let total: UInt32
    public let missing: [UInt32]

    public init(parts: [ChapterPart], total: UInt32, missing: [UInt32]) {
        self.parts = parts
        self.total = total
        self.missing = missing
    }
}

/// A credit page of a chapter, by its place among all the chapter's pages.
public struct CreditKey: Hashable, Sendable {
    public let chapter: String
    public let page: UInt32

    public init(chapter: String, page: UInt32) {
        self.chapter = chapter
        self.page = page
    }
}

/// A credit page kept as a page, waiting out its Undo.
public struct CreditKeep: Equatable, Sendable {
    public let chapter: String
    public let credit: CreditPage
    /// The page it becomes, from 0, among the pages the chapter shows.
    public let anchor: Int
}

/// A mark on the scrubber where a later part starts, or where a missing one would be.
public struct PartTick: Equatable, Sendable, Identifiable {
    /// The part's number, from 1.
    public let part: UInt32
    /// Its first page, from 1.
    public let page: Int
    /// The leaf that page is on.
    public let leaf: Int
    /// No group lists the part: the mark is where it would be.
    public let missing: Bool

    public var id: String { "\(part)\(missing ? "m" : "")" } // i18n-skip: an id
}

extension ReaderModel {
    /// What the paged reader shows next across credit pages: a credit page over a leaf, or the
    /// leaf itself.
    public enum CreditStep: Equatable, Sendable {
        /// The credit page `page`, shown over the leaf `leaf`.
        case credit(UInt32, leaf: Int)
        /// The leaf, with no credit page over it.
        case leaf(Int)
    }

    // MARK: What the chapter skipped

    /// A chapter's credit pages in the order the source had them.
    public func chapterCredits(_ chapter: String) -> [CreditPage] {
        (credits[chapter] ?? []).sorted { $0.page < $1.page }
    }

    /// The open chapter's credit pages.
    public var openCredits: [CreditPage] {
        guard let chapter = view?.chapter.id else { return [] }
        return chapterCredits(chapter)
    }

    /// The page, from 0 among the pages the chapter shows, a credit page sits before. The
    /// chapter's page count when it comes after the last page.
    public func anchor(of credit: CreditPage, in chapter: String) -> Int {
        let before = (credits[chapter] ?? []).filter { $0.page < credit.page }.count
        return Int(credit.page) - before
    }

    /// The leaf a credit page of the open chapter sits before: the end page for those after the
    /// last page.
    public func creditLeaf(_ credit: CreditPage) -> Int {
        guard let chapter = view?.chapter.id else { return 0 }
        let anchor = anchor(of: credit, in: chapter)
        if credit.atEnd || anchor >= pageCount {
            return leaves.firstIndex(where: \.endPage) ?? max(leaves.count - 1, 0)
        }
        return leaves.firstIndex { $0.parts.contains { Int($0.page) >= anchor } } ?? max(leaves.count - 1, 0)
    }

    public func isRevealed(_ credit: CreditPage, chapter: String) -> Bool {
        revealed.contains(CreditKey(chapter: chapter, page: credit.page))
    }

    /// The open chapter's credit pages brought back that sit before the leaf at `index`.
    public func revealedBefore(leaf index: Int) -> [CreditPage] {
        guard let chapter = view?.chapter.id, view?.mode != .strip, novel == nil else { return [] }
        return chapterCredits(chapter).filter { isRevealed($0, chapter: chapter) && creditLeaf($0) == index }
    }

    /// The credit page on screen, while the paged reader shows one.
    public var shownCredit: CreditPage? {
        guard let onCredit else { return nil }
        return openCredits.first { $0.page == onCredit }
    }

    /// The open chapter's credit pages at its start that are still skipped, which page 1's line
    /// names.
    public var startSkipped: [CreditPage] {
        guard let chapter = view?.chapter.id else { return [] }
        return chapterCredits(chapter).filter { !$0.atEnd && anchor(of: $0, in: chapter) == 0 && !isRevealed($0, chapter: chapter) }
    }

    /// A chapter's credit pages at its end that are still skipped, which the end page and the
    /// strip's marker name.
    public func endSkipped(_ chapter: String) -> [CreditPage] {
        chapterCredits(chapter).filter { $0.atEnd && !isRevealed($0, chapter: chapter) }
    }

    /// "Chapter 87 · credit page 1 of 2": where the credit page on screen is among those shown
    /// before the same page. Nil off a credit page.
    public var creditPlace: (n: Int, of: Int)? {
        guard let credit = shownCredit else { return nil }
        let group = revealedBefore(leaf: creditLeaf(credit))
        let n = (group.firstIndex { $0.page == credit.page } ?? 0) + 1
        return (n, max(group.count, 1))
    }

    // MARK: Counting while a page waits on its Undo

    /// The kept page counts as soon as it is kept: page 1 of 38 while its Undo runs.
    private var keptHere: CreditKeep? {
        guard let keeping, keeping.chapter == view?.chapter.id else { return nil }
        return keeping
    }

    /// The page count the counter shows.
    public var shownPageCount: Int {
        pageCount + (keptHere == nil ? 0 : 1)
    }

    /// The page number the counter shows for a leaf, counting a kept page from the moment it is kept.
    public func shownPageNumber(of leaf: Int) -> Int {
        if let keep = keptHere, onCredit == keep.credit.page { return keep.anchor + 1 }
        let number = pageNumber(of: leaf)
        guard let keep = keptHere else { return number }
        return number > keep.anchor ? number + 1 : number
    }

    // MARK: Parts

    /// The marks on the scrubber: where each part after the first starts, and where a missing
    /// part would be.
    public var partTicks: [PartTick] {
        guard let parts, view?.mode != .strip, novel == nil, !leaves.isEmpty else { return [] }
        var ticks: [PartTick] = []
        for part in parts.parts.dropFirst() {
            let page = Int(part.firstPage)
            ticks.append(PartTick(part: part.number, page: page, leaf: leaf(ofPage: page), missing: false))
        }
        let last = leaves.lastIndex { !$0.endPage } ?? 0
        for number in parts.missing where number > 1 {
            // Where it would start: the first page of the part after it, or after the last page.
            if let after = parts.parts.first(where: { $0.number > number }) {
                let page = Int(after.firstPage)
                ticks.append(PartTick(part: number, page: page, leaf: leaf(ofPage: page), missing: true))
            } else {
                ticks.append(PartTick(part: number, page: pageCount + 1, leaf: last, missing: true))
            }
        }
        return ticks
    }

    /// The part a page, from 1, starts, if it is the first page of a later part.
    public func partStarting(atPage page: Int) -> UInt32? {
        parts?.parts.dropFirst().first { Int($0.firstPage) == page }?.number
    }

    /// The missing part just before the part a page starts, for "Page 14 · part 4 · part 3
    /// missing". With more than one missing in a row, the nearest.
    public func partMissing(beforePage page: Int) -> UInt32? {
        guard let parts, let part = partStarting(atPage: page) else { return nil }
        let before = parts.parts.last { $0.number < part }?.number ?? 0
        return parts.missing.filter { $0 > before && $0 < part }.max()
    }

    // MARK: Stepping through them in pages

    /// Where a turn goes when credit pages brought back are in its way. Nil when the turn is an
    /// ordinary one from leaf to leaf.
    public func creditStep(forward: Bool) -> CreditStep? {
        guard leaves.indices.contains(current) else { return nil }
        if let credit = shownCredit {
            let group = revealedBefore(leaf: current)
            let at = group.firstIndex { $0.page == credit.page } ?? 0
            if forward {
                if at + 1 < group.count { return .credit(group[at + 1].page, leaf: current) }
                return .leaf(current)
            }
            if at > 0 { return .credit(group[at - 1].page, leaf: current) }
            return current > 0 ? .leaf(current - 1) : nil
        }
        if forward {
            let next = current + 1
            guard leaves.indices.contains(next), let first = revealedBefore(leaf: next).first else { return nil }
            // Past the end page is the next chapter, never its credit pages.
            if leaves[current].endPage { return nil }
            return .credit(first.page, leaf: next)
        }
        guard let last = revealedBefore(leaf: current).last else { return nil }
        return .credit(last.page, leaf: current)
    }

    /// The paged reader shows the credit page `page` over the leaf `leaf`, or the leaf alone
    /// with nil. A leaf that changes is reported as shown.
    public func showCredit(_ page: UInt32?, leaf: Int, how: TurnHow) {
        if leaf != current { shown(leaf, how: how) }
        onCredit = page
        if leaf == 0, page != nil { endCreditLine() }
        if page != nil { hideChrome() }
        sayCredit()
    }

    /// The line on page 1 or the end page was tapped: the credit pages it names come back, and
    /// the paged reader shows the first of them.
    public func showStartCredits() {
        guard let chapter = view?.chapter.id else { return }
        let group = startSkipped
        guard let first = group.first else { return }
        for credit in group { revealed.insert(CreditKey(chapter: chapter, page: credit.page)) }
        endCreditLine()
        if view?.mode != .strip { showCredit(first.page, leaf: creditLeaf(first), how: .turn) }
    }

    /// Show on the end page, or in a strip's marker: the pages skipped at the chapter's end come back.
    public func showEndCredits(_ chapter: String? = nil) {
        guard let chapter = chapter ?? view?.chapter.id else { return }
        let group = endSkipped(chapter)
        guard let first = group.first else { return }
        for credit in group { revealed.insert(CreditKey(chapter: chapter, page: credit.page)) }
        if view?.mode != .strip, chapter == view?.chapter.id { showCredit(first.page, leaf: creditLeaf(first), how: .turn) }
    }

    /// A strip's line brought the pages it names back in place.
    public func reveal(_ pages: [CreditPage], chapter: String) {
        for credit in pages { revealed.insert(CreditKey(chapter: chapter, page: credit.page)) }
    }

    // MARK: Not a credit page, and Undo

    /// Not a credit page, for the credit page on screen.
    func keepCredit() {
        guard let credit = shownCredit, let chapter = view?.chapter.id else { return }
        keep(credit, chapter: chapter)
    }

    /// How long Undo stays after Not a credit page: 8 s, doubled while a screen reader runs. It
    /// always runs out, so the engine hears of the keep even when no one turns away.
    static func keepHold(screenReader: Bool) -> UInt32 { screenReader ? 16_000 : 8_000 }

    /// Not a credit page, for a credit page of a strip.
    public func keepCredit(_ credit: CreditPage, chapter: String) {
        keep(credit, chapter: chapter)
    }

    private func keep(_ credit: CreditPage, chapter: String) {
        if keeping != nil { commitKeep() }
        let keep = CreditKeep(chapter: chapter, credit: credit, anchor: anchor(of: credit, in: chapter))
        keeping = keep
        // The page counts from now; the engine hears once the Undo has run out.
        noticeCount += 1
        let line = ReaderNoticeLine(id: noticeCount, text: L.readerCreditKept, action: nil, local: .undoKeep)
        notice = line
        noticeTask?.cancel()
        noticeTask = nil
        keepTask?.cancel()
        // Undo outlasts a turn: the keep goes to the engine when its notice runs out, on a second
        // keep, or when the chapter closes, never because the person read on.
        keepTask = countdown(Self.keepHold(screenReader: pinChrome)) { [weak self] in
            guard let self, self.keeping == keep else { return }
            self.commitKeep()
            if self.notice?.id == line.id { self.notice = nil }
        }
    }

    /// Undo, while the kept page waits: it is a credit page again, still shown.
    public func undoKeep() {
        guard keeping != nil else { return }
        keepTask?.cancel()
        keepTask = nil
        keeping = nil
        notice = nil
        sayCredit()
    }

    /// The kept page goes to the engine, which keeps it in the series from now on and lays the
    /// chapter out again with it.
    func commitKeep() {
        guard let keep = keeping else { return }
        keepTask?.cancel()
        keepTask = nil
        keeping = nil
        if notice?.local == .undoKeep { notice = nil }
        // Read on to another credit page while Undo ran: that page gets its own notice now.
        if onCredit != keep.credit.page { sayCredit() }
        guard session != 0 else { return }
        relayKeep = keep
        send?(.reader(.notACreditPage(session: session, fingerprint: keep.credit.fingerprint)))
    }

    /// The chapter came laid out again. When it is the layout with a kept page, the pages from
    /// that page on move up one, so what has landed keeps its place, and the screen stays on the
    /// page it showed.
    func relaidWithKept(_ chapter: String) {
        guard let keep = relayKeep, keep.chapter == chapter else { return }
        relayKeep = nil
        let anchor = UInt32(keep.anchor)
        var moved: [ReaderPageKey: PageFile] = [:]
        for (key, file) in pages {
            guard key.chapter == chapter, key.page >= anchor else { moved[key] = file; continue }
            moved[ReaderPageKey(chapter: chapter, page: key.page + 1, half: key.half)] = file
        }
        moved[ReaderPageKey(chapter: chapter, page: anchor)] = keep.credit.file
        pages = moved
        failures = failures.filter { $0.key.chapter != chapter }
        revealed = revealed.filter { $0.chapter != chapter || $0.page != keep.credit.page }
        if chapter == view?.chapter.id {
            if view?.mode == .strip {
                if let here = stripHere, here.image > anchor {
                    stripHere = StripPos(image: here.image + 1, offset: here.offset)
                }
            } else if onCredit == keep.credit.page {
                keepPage = anchor
            } else if leaves.indices.contains(current), let page = leaves[current].parts.map(\.page).min() {
                keepPage = page >= anchor ? page + 1 : page
            }
            if onCredit == keep.credit.page { onCredit = nil }
        }
    }

    /// The engine said which pages it skipped in a layout of `chapter`.
    func creditsCame(_ chapter: String, _ pages: [CreditPage]) {
        credits[chapter] = pages
        // A page brought back that is no longer skipped is no longer on screen as a credit page.
        if chapter == view?.chapter.id, let onCredit, !pages.contains(where: { $0.page == onCredit }) {
            self.onCredit = nil
            sayCredit()
        }
    }

    /// Off a credit page. A page kept from it waits out its Undo.
    func leftCredit() {
        if onCredit != nil {
            onCredit = nil
            sayCredit()
        }
    }

    /// The notice follows the credit page on screen: Skipped as a credit page with Not a credit
    /// page while one shows, gone when none does.
    private func sayCredit() {
        if keeping != nil, view?.mode != .strip { return }
        if shownCredit != nil {
            guard notice?.local != .keepCredit else { return }
            noticeCount += 1
            notice = ReaderNoticeLine(id: noticeCount, text: L.readerCreditShown, action: nil, local: .keepCredit)
            noticeTask?.cancel()
            noticeTask = nil
        } else if notice?.local == .keepCredit {
            notice = nil
        }
    }

    // MARK: The line on page 1

    /// Page 1 has drawn: the line names the credit pages skipped before it, once each time the
    /// chapter opens, and holds 4 seconds.
    func startCreditLine() {
        guard let chapter = view?.chapter.id, view?.mode != .strip, novel == nil, current == 0, onCredit == nil,
              creditLineFor != chapter, !startSkipped.isEmpty else { return }
        creditLineFor = chapter
        creditLine = true
        creditLineTask?.cancel()
        creditLineTask = countdown(4_000) { [weak self] in self?.creditLine = false }
    }

    func endCreditLine() {
        creditLineTask?.cancel()
        creditLineTask = nil
        creditLine = false
    }

    #if FIXTURES
    /// For shots: the engine skipped `pages` in the open chapter, and its parts are `parts`.
    public func stageCredits(_ pages: [CreditPage], parts: ReaderParts? = nil) {
        guard let chapter = view?.chapter.id else { return }
        credits[chapter] = pages
        self.parts = parts
        creditLineFor = nil
    }

    /// For shots: the line on page 1 stays up.
    public func stageCreditLine() {
        guard let chapter = view?.chapter.id, !startSkipped.isEmpty else { return }
        creditLineFor = chapter
        creditLine = true
    }
    #endif
}

extension ReaderModel {
    /// `-KaeriReader chapterlist` in the Fixtures app: the chapters sheet opens on its chapters
    /// rather than its pages.
    public static var chapterListShot: Bool {
        #if FIXTURES
        let arguments = ProcessInfo.processInfo.arguments
        guard let at = arguments.firstIndex(of: argument), arguments.indices.contains(at + 1) else { return false }
        return arguments[at + 1] == "chapterlist"
        #else
        false
        #endif
    }
}

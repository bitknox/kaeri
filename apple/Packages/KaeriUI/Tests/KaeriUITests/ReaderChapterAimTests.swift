// While a chapter opens, and after it would not open, the reader is on that chapter: the opening
// line and the failure name it, and ] and [ step from it, not from the chapter left (iPad R7P-2,
// Mac R7-M3). The model's view stays the chapter left until the engine says the new one opened.

@testable import KaeriStore
@testable import KaeriUI
import KaeriKit
import Testing

@MainActor
private final class Sent {
    var all: [Command] = []
    /// The chapters asked to open, in order.
    var opens: [ChapterId] {
        all.compactMap { if case .reader(.openChapter(_, let chapter)) = $0 { chapter } else { nil } }
    }
}

@MainActor
struct ReaderChapterAimTests {
    private let work = "a:900301"

    private func row(_ number: Int) -> ChapterRow {
        ChapterRow(
            id: ChapterId("c\(number)"), label: "Chapter \(number)", state: .unread, number: "\(number)",
            title: nil, volume: nil, published: nil, pages: 3, newChapter: false, download: nil, versions: 1,
            bookmarked: false)
    }

    /// Chapter 6 open, between 5 and 7, as the hunt had it.
    private func onSix(_ sent: Sent) -> ReaderModel {
        let model = ReaderModel()
        model.send = { sent.all.append($0) }
        model.open(work: work, chapter: "c6")
        model.surfaceChanged(ReaderSurface(widthPx: 1600, heightPx: 1000, scale: 2, eink: false, twoPages: false, dark: true))
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId(work), title: "The Lantern Ferry", medium: .manga, chapter: row(6),
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId(work), chapter: ChapterId("c6"), page: 0, strip: nil, novel: nil),
            next: row(7), previous: row(5), settings: nil)))
        return model
    }

    private func listed(_ model: ReaderModel) {
        model.observe(.reading(.chapters(
            work: WorkId(work), edition: nil, volumes: [VolumeGroup(chapters: (1...9).map(row))], failure: nil)))
    }

    private func failOffline(_ model: ReaderModel) {
        model.observe(.reading(.failed(about: .reader, failure: .offline)))
    }

    @Test func theNextChapterOpeningIsTheOneNamed() {
        let sent = Sent()
        let model = onSix(sent)
        let actions = ReaderActions(model: model, close: {})
        actions.chapter(true)
        #expect(sent.opens == [ChapterId("c7")])
        #expect(model.phase == .opening)
        #expect(ReaderOpening.label(model) == "7", "the plate names the chapter opening, not chapter 6")
    }

    @Test func afterTheNextChapterFailsPreviousGoesBackToTheChapterLeft() {
        let sent = Sent()
        let model = onSix(sent)
        ReaderActions(model: model, close: {}).chapter(true)
        failOffline(model)
        #expect(ReaderOpening.label(model) == "7", "the failure names chapter 7")
        ReaderActions(model: model, close: {}).chapter(false)
        #expect(sent.opens == [ChapterId("c7"), ChapterId("c6")], "[ on the failed chapter 7 opens 6, not 5")
        #expect(ReaderOpening.label(model) == "6", "and the plate names 6")
    }

    @Test func afterTheNextChapterFailsNextGoesOnFromIt() {
        let sent = Sent()
        let model = onSix(sent)
        listed(model)
        ReaderActions(model: model, close: {}).chapter(true)
        failOffline(model)
        ReaderActions(model: model, close: {}).chapter(true)
        #expect(sent.opens == [ChapterId("c7"), ChapterId("c8")], "] on the failed chapter 7 opens 8, not 7 again")
    }

    @Test func withNoChapterListNextFromAFailedChapterWaits() {
        let sent = Sent()
        let model = onSix(sent)
        ReaderActions(model: model, close: {}).chapter(true)
        failOffline(model)
        ReaderActions(model: model, close: {}).chapter(true)
        #expect(sent.opens == [ChapterId("c7")], "the chapter after 7 is not known, so nothing opens, least of all 7 again")
    }

    @Test func afterThePreviousChapterFailsNextComesBackToTheChapterLeft() {
        let sent = Sent()
        let model = onSix(sent)
        listed(model)
        ReaderActions(model: model, close: {}).chapter(false)
        #expect(ReaderOpening.label(model) == "5")
        failOffline(model)
        ReaderActions(model: model, close: {}).chapter(true)
        #expect(sent.opens == [ChapterId("c5"), ChapterId("c6")])
        ReaderActions(model: model, close: {}).chapter(false)
        #expect(sent.opens == [ChapterId("c5"), ChapterId("c6"), ChapterId("c5")], "[ while 6 opens again steps from 6")
    }

    @Test func aSecondStepWhileTheFirstOpensGoesOnFromIt() {
        let sent = Sent()
        let model = onSix(sent)
        listed(model)
        ReaderActions(model: model, close: {}).chapter(true)
        ReaderActions(model: model, close: {}).chapter(true)
        #expect(sent.opens == [ChapterId("c7"), ChapterId("c8")])
        #expect(ReaderOpening.label(model) == "8")
    }

    @Test func onceTheChapterOpensItsOwnNeighboursCount() {
        let sent = Sent()
        let model = onSix(sent)
        ReaderActions(model: model, close: {}).chapter(true)
        model.apply(.opened(ReaderView(
            session: 1, work: WorkId(work), title: "The Lantern Ferry", medium: .manga, chapter: row(7),
            mode: .paged, direction: .rightToLeft,
            start: ReadPlace(work: WorkId(work), chapter: ChapterId("c7"), page: 0, strip: nil, novel: nil),
            next: row(8), previous: row(6), settings: nil)))
        #expect(ReaderOpening.label(model) == "7")
        ReaderActions(model: model, close: {}).chapter(true)
        #expect(sent.opens == [ChapterId("c7"), ChapterId("c8")])
    }
}

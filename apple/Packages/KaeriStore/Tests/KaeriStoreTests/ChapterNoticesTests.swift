// New-chapter notifications: one per series in the words the Android app uses, how the system is
// told to group them, where Read and a tap take the app, the switch asking for permission, and
// the background check over a fake engine. Nothing here reaches the system's notification
// centre: the centre is a quiet one throughout.

@testable import KaeriStore
import KaeriKit
import Foundation
import Synchronization
import Testing
import UserNotifications

private func chapter(
    _ work: String, _ id: String, label: String, published: Int64? = nil, next: String? = nil,
    cover: String? = nil, banner: String? = nil
) -> NewChapter {
    NewChapter(
        work: work, title: "Series \(work)", chapter: id, label: label, cover: cover, banner: banner,
        published: published, nextForYou: next, sourceName: nil, chapterTitle: nil)
}

private func quietScheduler(_ centre: QuietNoticeCentre) -> NoticeScheduler {
    let suite = "kaeri.tests.\(UUID().uuidString)"
    return NoticeScheduler(centre: centre, ledger: NoticeLedger(defaults: UserDefaults(suiteName: suite)!, key: "notices")) { _ in nil }
}

@Suite struct ChapterNoticePlan {
    @Test func oneNotificationPerSeriesWordedByItsNewestChapter() {
        let notices = ChapterNotices.plan([
            chapter("w1", "c11", label: "Chapter 11", published: 100, cover: "https://img.example/w1.png"),
            chapter("w2", "c3", label: "Chapter 3", next: "c2", cover: "https://img.example/w2.png", banner: "https://img.example/w2-banner.jpg"),
            chapter("w1", "c12", label: "Chapter 12", published: 200, next: "c9", cover: "https://img.example/w1.png"),
        ])
        #expect(notices.map(\.identifier) == ["ch.w1", "ch.w2"], "in the order the series first come")
        #expect(notices[0].title == "Series w1")
        #expect(notices[0].body == "2 new chapters")
        #expect(notices[0].opens == "c9", "Read opens the chapter the person is up to")
        #expect(notices[0].artURL == "https://img.example/w1.png")
        #expect(notices[1].body == "Chapter 3 is new.")
        #expect(notices[1].opens == "c2")
        #expect(notices[1].artURL == "https://img.example/w2-banner.jpg", "the banner before the cover")

        let caughtUp = ChapterNotices.plan([chapter("w3", "c5", label: "Chapter 5")])
        #expect(caughtUp.first?.opens == "c5", "caught up, Read opens the new one")
    }

    @Test func theyAreInTheirOwnThreadWithRead() throws {
        let notice = ChapterNotices.plan([chapter("w1", "c12", label: "Chapter 12")])[0]
        let content = SystemNoticeCentre.content(notice)
        #expect(content.threadIdentifier == "new-chapters")
        #expect(content.summaryArgument == "Series w1")
        #expect(content.title == "Series w1")
        #expect(content.body == "Chapter 12 is new.")
        #expect(content.categoryIdentifier == "new-chapter")

        let category = try #require(SystemNoticeCentre.categories().first { $0.identifier == "new-chapter" })
        #expect(category.actions.map(\.identifier) == ["read"])
        #expect(category.actions.map(\.title) == ["Read"])
        #expect(category.actions[0].options.contains(.foreground), "Read opens the app")
        #expect(category.hiddenPreviewsBodyPlaceholder == "New chapter")
    }
}

@MainActor
@Suite struct ChapterNoticeRouting {
    @Test func aTapOpensTheSeriesAndReadTheChapter() {
        let notice = ChapterNotices.plan([chapter("w1", "c12", label: "Chapter 12", next: "c9")])[0]
        let info = NoticeRoute.userInfo(notice)
        #expect(NoticeRoute(action: UNNotificationDefaultActionIdentifier, userInfo: info) == .series("w1"))
        #expect(NoticeRoute(action: "read", userInfo: info) == .read("w1", chapter: "c9"))
        #expect(NoticeRoute(action: "play", userInfo: info) == nil, "no episode's action")
        #expect(NoticeRoute(action: UNNotificationDismissActionIdentifier, userInfo: info) == nil)
        #expect(NoticeRoute(action: "read", userInfo: ["title": 7, "episode": 10]) == nil, "not a new chapter's")
        #expect(NoticeRoute.read("w1", chapter: "c9").needsForeground)
        #expect(NoticeRoute.series("w1").needsForeground)
    }

    @Test func theStoreLeavesThemToTheNavigator() {
        let store = AppStore()
        store.perform(.series("w1"))
        store.perform(.read("w1", chapter: "c9"))
        #expect(store.sentLog.isEmpty)
    }
}

@MainActor
@Suite struct ChapterNoticePosting {
    @Test func theSchedulerPostsEachSeriesOnceAllowed() async {
        let allowed = QuietNoticeCentre(permission: .allowed)
        let posted = await quietScheduler(allowed).post(chapters: [
            chapter("w1", "c1", label: "Chapter 1"), chapter("w2", "c4", label: "Chapter 4"),
        ])
        #expect(posted.map(\.identifier) == ["ch.w1", "ch.w2"])
        #expect(allowed.chapterNotices.map(\.identifier) == ["ch.w1", "ch.w2"])

        let denied = QuietNoticeCentre(permission: .denied)
        #expect(await quietScheduler(denied).post(chapters: [chapter("w1", "c1", label: "Chapter 1")]).isEmpty)
        #expect(denied.postedNow.isEmpty)
        #expect(denied.timesAsked == 0, "a post never asks")
    }

    @Test func whatTheEngineSaysIsPosted() async throws {
        let centre = QuietNoticeCentre(permission: .allowed)
        let model = NotificationsModel()
        model.scheduler = quietScheduler(centre)
        model.observe(.reading(.newChapters([chapter("w1", "c12", label: "Chapter 12")])))
        for _ in 0..<100 where centre.chapterNotices.isEmpty {
            try await Task.sleep(for: .milliseconds(20))
        }
        #expect(centre.chapterNotices.map(\.body) == ["Chapter 12 is new."])
    }

    @Test func turningOnAsksOnceAndADeclineLeavesItOff() async {
        let store = AppStore()
        let declining = QuietNoticeCentre(permission: .notAsked, answer: false)
        store.notifications.scheduler = quietScheduler(declining)
        await store.setChapterNotifications(true)
        #expect(declining.timesAsked == 1)
        #expect(store.notifications.declined)
        #expect(!store.sentLog.contains("setSetting"), "declined, the switch stays off")

        let store2 = AppStore()
        let allowing = QuietNoticeCentre(permission: .notAsked, answer: true)
        store2.notifications.scheduler = quietScheduler(allowing)
        await store2.setChapterNotifications(true)
        await store2.setChapterNotifications(false)
        #expect(allowing.timesAsked == 1, "turning off asks nothing")
        #expect(store2.sentLog.filter { $0 == "setSetting" } == ["setSetting"])
    }

    @Test func newChaptersOnKeepsTheBackgroundRefresh() {
        func view(chapters: Bool) -> SettingsView {
            settingsView(order: [], rows: [
                SettingRow(key: .chapterNotifications, value: chapters ? "on" : "off", change: .steps(back: chapters, forward: !chapters)),
            ])
        }
        #expect(NotificationsModel.notifies(view(chapters: true)))
        #expect(!NotificationsModel.notifies(view(chapters: false)))

        let model = NotificationsModel()
        var told: [Bool] = []
        model.onRefreshes = { told.append($0) }
        model.observe(.settings(view(chapters: true)))
        #expect(model.chaptersNotifying == true)
        model.observe(.settings(view(chapters: false)))
        #expect(model.chaptersNotifying == false)
        #expect(told == [true, false])
    }
}

/// An engine that answers the refresh's questions as the real one does, from `found`: the
/// check's Updates while it runs and once done, then the chapters, when `reads`.
private final class FakeEngine: Sendable {
    let answers = EngineAnswers()
    let found: [NewChapter]
    let reads: Bool
    let sent = Mutex<[String]>([])

    init(found: [NewChapter], reads: Bool = true) {
        self.found = found
        self.reads = reads
    }

    var questions: EngineQuestions {
        EngineQuestions(answers: answers) { [self] command in
            sent.withLock { $0.append(String(String(describing: command).prefix(40))) }
            switch command {
            case .checkNewEpisodes:
                answers.deliver([.newEpisodes([])])
            case .upcomingNotices:
                answers.deliver([.upcomingNotices(UpcomingNotices(
                    enabled: true, generatedAt: 0, airings: [], watching: [], overrides: 0))])
            case .reading(.checkUpdates):
                guard reads else {
                    answers.deliver([.reading(.updates(UpdatesView(checking: false)))])
                    return
                }
                answers.deliver([.reading(.updates(UpdatesView(checking: true)))])
                Task { [answers, found] in
                    try? await Task.sleep(for: .milliseconds(50))
                    answers.deliver([.reading(.updates(UpdatesView(checking: false)))])
                    if !found.isEmpty { answers.deliver([.reading(.newChapters(found))]) }
                }
            default:
                break
            }
        }
    }

    var checks: Int { sent.withLock { $0.filter { $0.contains("checkUpdates") }.count } }
}

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct ChapterNoticeRefresh {
    @Test func theQuestionWaitsForTheCheckAndTakesWhatItFound() async {
        let engine = FakeEngine(found: [chapter("w1", "c12", label: "Chapter 12")])
        let found = await engine.questions.newChapters(timeout: .seconds(10), settle: .milliseconds(200))
        #expect(found?.map(\.chapter) == ["c12"])
    }

    @Test func withReadingOffTheQuestionIsAnsweredAtOnce() async {
        let engine = FakeEngine(found: [], reads: false)
        let clock = ContinuousClock()
        let start = clock.now
        let found = await engine.questions.newChapters(timeout: .seconds(10), settle: .seconds(10))
        #expect(found == [])
        #expect(clock.now - start < .seconds(5), "no wait for chapters that never come")
    }

    @Test func aCancelledQuestionEndsAtOnce() async {
        // An engine that never answers.
        let questions = EngineQuestions(answers: EngineAnswers()) { _ in }
        let asking = Task { await questions.newChapters(timeout: .seconds(50)) }
        try? await Task.sleep(for: .milliseconds(50))
        asking.cancel()
        #expect(await asking.value == nil)
    }

    @Test func onItsOwnEngineARefreshPostsTheChapters() async {
        let engine = FakeEngine(found: [chapter("w1", "c12", label: "Chapter 12"), chapter("w2", "c2", label: "Chapter 2")])
        let centre = QuietNoticeCentre(permission: .allowed)
        let outcome = await NoticeRefresh(
            questions: engine.questions, scheduler: quietScheduler(centre), widgets: nil, chapters: .checkAndPost
        ).run()
        #expect(outcome.finished)
        #expect(outcome.postedChapters.map(\.identifier) == ["ch.w1", "ch.w2"])
        #expect(centre.chapterNotices.map(\.identifier) == ["ch.w1", "ch.w2"])
    }

    @Test func onTheStoresEngineItChecksAndLeavesThePostingToTheStore() async {
        let engine = FakeEngine(found: [chapter("w1", "c12", label: "Chapter 12")])
        let centre = QuietNoticeCentre(permission: .allowed)
        let outcome = await NoticeRefresh(
            questions: engine.questions, scheduler: quietScheduler(centre), widgets: nil, chapters: .check
        ).run()
        #expect(outcome.chapters?.count == 1)
        #expect(centre.chapterNotices.isEmpty)
    }

    @Test func withNothingNotifyingOrNewChaptersOffThereIsNoCheck() async {
        for refresh in [
            { (q: EngineQuestions, s: NoticeScheduler) in NoticeRefresh(questions: q, scheduler: s, widgets: nil, notifying: false, chapters: .checkAndPost) },
            { (q: EngineQuestions, s: NoticeScheduler) in NoticeRefresh(questions: q, scheduler: s, widgets: nil, chapters: .skip) },
        ] {
            let engine = FakeEngine(found: [chapter("w1", "c12", label: "Chapter 12")])
            let centre = QuietNoticeCentre(permission: .allowed)
            let outcome = await refresh(engine.questions, quietScheduler(centre)).run()
            #expect(outcome.chapters == nil)
            #expect(engine.checks == 0)
            #expect(centre.chapterNotices.isEmpty)
        }
    }
}

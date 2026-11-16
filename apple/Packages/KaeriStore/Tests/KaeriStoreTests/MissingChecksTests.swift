// Check again on a missing episode: the command, the wait before Checking shows, the least time
// it stays, and what each answer leaves and says.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
struct MissingChecksTests {
    private let id: AnilistId = 900_002

    private func check(_ outcome: CheckOutcome, asked: Bool = true) -> EpisodeCheck {
        EpisodeCheck(id: id, episode: 6, outcome: outcome, asked: asked)
    }

    /// A store whose waits are short, so the tests run in a moment.
    private func store(showAfter: Duration = .milliseconds(40), holdFor: Duration = .milliseconds(80)) -> AppStore {
        let store = AppStore()
        store.missing.showAfter = showAfter
        store.missing.holdFor = holdFor
        return store
    }

    private func wait(until done: () -> Bool) async {
        let deadline = ContinuousClock.now + .seconds(2)
        while !done(), ContinuousClock.now < deadline {
            try? await Task.sleep(for: .milliseconds(5))
        }
    }

    @Test func aPressAsksTheEngineOnceAndAnotherWhileItRunsAddsNothing() {
        let store = store()
        store.missing.checkAgain(id, episode: 6)
        store.missing.checkAgain(id, episode: 6)
        #expect(store.sentLog == ["missing"])
        #expect(store.missing.isRunning(id))
        #expect(!store.missing.showsChecking(id), "nothing shows before the wait")
    }

    @Test func aQuickAnswerNeverShowsChecking() {
        let store = store(showAfter: .seconds(5))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.missing)))
        #expect(!store.missing.isRunning(id))
        #expect(!store.missing.showsChecking(id))
        #expect(store.missing.answer(id, episode: 6) == .still)
        #expect(store.missing.spoken?.kind == .still, "the answer is said once")
    }

    @Test func checkingShowsAfterTheWaitAndStaysItsLeastTime() async {
        let store = store()
        // The clock stands still from the start: the answer comes the moment Checking shows,
        // however late a busy machine wakes the test, and still waits out its least time.
        let start = ContinuousClock.now
        store.missing.now = { start }
        store.missing.checkAgain(id, episode: 6)
        await wait { store.missing.showsChecking(id) }
        #expect(store.missing.showsChecking(id))
        #expect(store.missing.spoken?.kind == .checking, "Checking is said once it shows")
        store.apply(.episodeChecked(check(.noAnswer)))
        #expect(store.missing.showsChecking(id), "held until it has stayed its least time")
        #expect(store.missing.answer(id, episode: 6) == nil)
        await wait { !store.missing.isRunning(id) }
        #expect(!store.missing.showsChecking(id))
        #expect(store.missing.answer(id, episode: 6) == .noAnswer)
        #expect(store.missing.spoken?.kind == .noAnswer)
    }

    @Test func aFindClearsTheAnswerAndIsSaid() {
        let store = store(showAfter: .seconds(5))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.missing)))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.found)))
        #expect(store.missing.answer(id, episode: 6) == nil)
        #expect(store.missing.spoken?.kind == .found(episode: 6))
    }

    @Test func offlineAnswersAtOnceAndLeavesTheWordsAsTheyWere() {
        let store = store(showAfter: .seconds(5))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.missing)))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.offline)))
        #expect(!store.missing.isRunning(id))
        #expect(store.missing.answer(id, episode: 6) == .still)
        #expect(store.missing.spoken?.kind == .offline)
    }

    @Test func aQuietRecheckIsSaidOnlyWhenItFindsTheEpisode() {
        let store = store()
        store.apply(.episodeChecked(check(.missing, asked: false)))
        #expect(store.missing.spoken == nil, "an empty quiet recheck changes nothing")
        #expect(store.missing.answer(id, episode: 6) == nil)
        store.apply(.episodeChecked(check(.found, asked: false)))
        #expect(store.missing.spoken?.kind == .found(episode: 6))
        #expect(!store.missing.showsChecking(id), "a quiet recheck never shows Checking")
    }

    @Test func anAnswerBelongsToItsEpisode() {
        let store = store(showAfter: .seconds(5))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.missing)))
        #expect(store.missing.answer(id, episode: 7) == nil, "the next episode starts with not yet")
    }

    @Test func twoOfTheSameWordsAreTwoThingsToSay() {
        let store = store(showAfter: .seconds(5))
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.missing)))
        let first = store.missing.spoken
        store.missing.checkAgain(id, episode: 6)
        store.apply(.episodeChecked(check(.missing)))
        #expect(store.missing.spoken != first)
    }
}

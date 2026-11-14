// The store over a real engine: the testing one, on a scene, in a directory of the test's own.

@testable import KaeriStore
import KaeriKit
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct LaunchTests {
    /// A store on the testing engine started on `scene` under `root`, with every event applied.
    func started(_ scene: FfiScene, root: URL) async throws -> (AppStore, Recorder) {
        let store = AppStore()
        let recorder = Recorder()
        store.onApplied = { recorder.events += $0 }
        let launch = EngineLaunch(
            engine: .fixtures(scene),
            home: .override(root: root),
            platform: .macOs,
            secrets: .host
        )
        await store.start(launch, host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        return (store, recorder)
    }

    @Test func launchSaysTheSettingsFirst() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, recorder) = try await started(.fresh, root: root)
        defer { store.shutdown() }

        try await waitFor { store.settings.view != nil }
        guard case .settings = recorder.events.first else {
            Issue.record("launch's settings first: \(recorder.events.prefix(3))")
            return
        }
        // The files went where the override says, and nowhere else.
        #expect(FileManager.default.fileExists(atPath: root.appending(path: "data/kaeri.db").path))
    }

    @Test func aFreshProfileSavesNoCrashReportsAndSubscribesToNothing() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.fresh, root: root)
        defer { store.shutdown() }
        try await waitFor { store.settings.view != nil }

        #expect(store.settings.row(.saveCrashReports) != nil)
        #expect(!store.settings.isOn(.saveCrashReports))
        var calls = 0
        let follower = CrashFollower(
            isOn: { store.settings.isOn(.saveCrashReports) }, subscribe: { calls += 1 }, unsubscribe: { calls += 1 })
        #expect(!follower.subscribed)
        #expect(calls == 0)
    }

    @Test func aFreshLaunchHasNothingToContinue() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.fresh, root: root)
        defer { store.shutdown() }

        store.loadContinue()
        #expect(store.continuing.list.state == .loading)
        try await waitFor { store.continuing.list.state != .loading }
        #expect(store.continuing.list.state == .empty)
    }

    @Test func theWatchingSceneFillsContinue() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.watching, root: root)
        defer { store.shutdown() }

        store.loadContinue()
        // The list, then the broadcast line that follows it for the airing titles.
        try await waitFor {
            !store.continuing.queue(store.titles).newThisWeek.isEmpty
        }
        let queue = store.continuing.queue(store.titles)
        #expect(queue.inProgress.map(\.entry.secondary) == [
            "Hoshikuzu no Tabibito", "Hagane no Niwashi", "Kumo no Ue no Toshokan",
        ])
        // The Steel Gardener is watched dubbed, so its row is marked, as the engine's queue is.
        #expect(queue.inProgress.map(\.action) == [
            "Resume E7 · 10:06", "Resume E2 dubbed · 13:30", "Play E4",
        ])
        #expect(queue.inProgress.map(\.dubbed) == [false, true, false])
        #expect(queue.newThisWeek.map(\.entry.secondary) == ["Natsukaze Relay"])
        #expect(queue.newThisWeek.first?.action.hasPrefix("Play E8 · out 19 h") == true)
    }

    @Test func aTitleWatchedUpToItsNextBroadcastIsCaughtUpAndOffersNothingToPlay() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.watching, root: root)
        defer { store.shutdown() }

        // Natsukaze Relay's episode 8 is out and 9 airs in six days: watch 8, and it is caught up.
        store.send(.setWatched(id: 900_006, episodes: ["8"], watched: true))
        store.loadContinue(force: true)
        try await waitFor {
            store.continuing.queue(store.titles).all.first { $0.id == 900_006 }?.episode == 9
        }
        let item = try #require(store.continuing.queue(store.titles).all.first { $0.id == 900_006 })
        #expect(!item.playable)
        #expect(item.action.hasPrefix("Caught up · E9 "), "\(item.action)")
        #expect(item.buttonTitle.hasPrefix("Episode 9 airs "), "\(item.buttonTitle)")
        #expect(!item.isNew)
        // The others still play.
        let others = store.continuing.queue(store.titles).all.filter { $0.id != 900_006 }
        #expect(others.count == 3 && others.filter(\.playable).count == 3)
    }

    @Test func shutdownStopsTheStore() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, _) = try await started(.fresh, root: root)
        store.shutdown()
        try await waitFor { store.phase == .stopped }
        #expect(store.send(.loadContinue) == nil)
    }

    @Test func aConfigThatDoesNotLoadFailsTheStartWithItsReason() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let config = root.appending(path: "config", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: config, withIntermediateDirectories: true)
        try "[playback]\nquality = \"lots\"\n".write(to: config.appending(path: "config.toml"), atomically: true, encoding: .utf8)

        let store = AppStore()
        await store.start(
            EngineLaunch(engine: .fixtures(.fresh), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        guard case .failed(let reason) = store.phase else {
            Issue.record("a config that does not load fails the start: \(store.phase)")
            return
        }
        #expect(reason.contains("config.toml has a setting"), "\(reason)")
        #expect(reason.contains("line 2:"), "\(reason)")
    }
}

/// Every event the store applied, in order.
@MainActor
final class Recorder {
    var events: [Event] = []
}

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct SettleTests {
    @Test func aStoreSettlesOnceContinueHasLoadedAndTheArtHasArrived() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore(artworkCache: KeptCache())
        await store.start(
            EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        defer { store.shutdown() }
        store.loadContinue()
        #expect(!store.isSettled)
        #expect(await store.settle(quiet: .milliseconds(300), timeout: .seconds(20)))
        #expect(store.continuing.queue(store.titles).all.count == 4)
        // The testing engine's fake serves the fixtures' art, so a cover arrives as an image, in
        // the cover's own colour beneath it until then.
        let entry = try #require(store.titles[900_001])
        #expect(entry.coverColor == "#e4a15d")
        let cover = try #require(entry.coverUrl)
        store.artwork.request(cover, maxPixelSize: 108)
        #expect(await store.settle(quiet: .milliseconds(300), timeout: .seconds(20)))
        #expect(store.artwork.image(cover)?.height == 108)
    }
}

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct EarlyTests {
    @Test func whatIsAskedForWhileTheEngineStartsIsSentOnceItHas() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        store.loadContinue()
        #expect(store.continuing.list.state == .loading)
        await store.start(
            EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        defer { store.shutdown() }
        try await waitFor { store.continuing.list.state.ids.count == 4 }
    }

    @Test func artAskedForWhileTheEngineStartsIsAskedForOnceItHas() async throws {
        // A cover on screen before the engine is there asks once, and never again by itself.
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        let answered = Box<[String]>([])
        store.onApplied = { events in
            for case .imageFailed(let url, _) in events { answered.value.append(url) }
        }
        // Nothing listens there, so the engine's answer is that it failed.
        let url = "http://127.0.0.1:1/cover/900001.jpg"
        store.artwork.request(url, maxPixelSize: 108)
        await store.start(
            EngineLaunch(engine: .fixtures(.watching), home: .override(root: root), platform: .macOs, secrets: .host),
            host: quietHost()
        )
        defer { store.shutdown() }
        try await waitFor { answered.value == [url] }
    }
}

/// Reset settings on the start-failed screen, over the testing engine.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct ResetSettingsTests {
    @Test func settingsTheEngineCannotStartOnAreResetAfterAskingAndKept() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let config = root.appending(path: "config", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: config, withIntermediateDirectories: true)
        try Data("[network\nproxy = ".utf8).write(to: config.appending(path: "config.toml"))

        let store = AppStore()
        let launch = EngineLaunch(engine: .fixtures(.fresh), home: .override(root: root), platform: .ios, secrets: .host)
        await store.start(launch, host: quietHost())
        guard case .failed = store.phase else {
            Issue.record("a config it cannot use stops the start: \(store.phase)")
            return
        }
        #expect(store.settingsUnusable)

        // Try again changes nothing: the file is as it was.
        await store.restart()
        #expect(store.settingsUnusable)

        await store.resetSettings()
        defer { store.shutdown() }
        #expect(store.phase == .running, "\(store.phase)")
        #expect(!store.settingsUnusable)
        let kept = try FileManager.default.contentsOfDirectory(atPath: config.path(percentEncoded: false))
            .filter { $0.hasPrefix("config.damaged-") }
        #expect(kept.count == 1, "\(kept)")
        #expect(store.notices.log.contains { $0.notice.text.contains(kept.first ?? "?") })
    }

    @Test func aStartThatFailedOnSomethingElseOffersNoReset() async throws {
        let store = AppStore()
        #expect(!store.settingsUnusable)
        await store.resetSettings()
        #expect(store.phase == .starting, "nothing started, nothing reset")
    }
}

/// Try again on the start-failed and engine-stopped screens: the engine started again is told
/// how the app and the device stand, though their tellers say them only when they change.
@MainActor
@Suite(.timeLimit(.minutes(1)))
struct StartAgainTests {
    /// A store whose system phases send through it, as the app wires them, and every
    /// connectivity event it applied.
    func wired() -> (AppStore, SystemPhases, Box<[Bool]>) {
        let store = AppStore()
        let heard = Box<[Bool]>([])
        store.onApplied = { events in
            for case .connectivity(let offline) in events { heard.value.append(offline) }
        }
        let phases = SystemPhases(send: { [weak store] in store?.send($0) }, playing: { false })
        return (store, phases, heard)
    }

    func launch(_ root: URL) -> EngineLaunch {
        EngineLaunch(engine: .fixtures(.fresh), home: .override(root: root), platform: .macOs, secrets: .host)
    }

    @Test func anEngineStartedAgainAfterAFailedStartIsToldTheNetwork() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let config = root.appending(path: "config", directoryHint: .isDirectory)
        try FileManager.default.createDirectory(at: config, withIntermediateDirectories: true)
        let file = config.appending(path: "config.toml")
        try "[playback]\nquality = \"lots\"\n".write(to: file, atomically: true, encoding: .utf8)

        let (store, phases, heard) = wired()
        // Said as the app starts the engine, and not again: the path does not change.
        phases.scene(.active)
        phases.network(expensive: false, constrained: false, offline: true)
        await store.start(launch(root), host: quietHost())
        guard case .failed = store.phase else {
            Issue.record("the start fails on the config: \(store.phase)")
            return
        }

        // The cause goes away, and Try again starts an engine that has heard nothing yet.
        try FileManager.default.removeItem(at: file)
        await store.restart()
        defer { store.shutdown() }
        try #require(store.phase == .running, "\(store.phase)")
        try await waitFor({ heard.value.contains(true) }, within: .seconds(30))
    }

    @Test func anEngineStartedAgainAfterItStoppedIsToldTheNetwork() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let (store, phases, heard) = wired()
        phases.network(expensive: false, constrained: false, offline: true)
        await store.start(launch(root), host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        try await waitFor({ heard.value == [true] }, within: .seconds(30))
        store.shutdown()
        try await waitFor { store.phase == .stopped }

        heard.value = []
        await store.restart()
        defer { store.shutdown() }
        try #require(store.phase == .running, "\(store.phase)")
        try await waitFor({ heard.value == [true] }, within: .seconds(30))
    }

    @Test func whatIsAskedForWhileTheEngineStartsAgainReachesTheNewOne() async throws {
        let root = temporaryRoot()
        defer { try? FileManager.default.removeItem(at: root) }
        let store = AppStore()
        await store.start(launch(root), host: quietHost())
        try #require(store.phase == .running, "\(store.phase)")
        store.shutdown()
        try await waitFor { store.phase == .stopped }

        let again = Task { await store.restart() }
        for _ in 0..<1000 where store.phase != .starting { await Task.yield() }
        try #require(store.phase == .starting, "\(store.phase)")
        store.loadContinue(force: true)
        await again.value
        defer { store.shutdown() }
        try #require(store.phase == .running, "\(store.phase)")
        try await waitFor({ store.continuing.list.state == .empty }, within: .seconds(30))
    }
}

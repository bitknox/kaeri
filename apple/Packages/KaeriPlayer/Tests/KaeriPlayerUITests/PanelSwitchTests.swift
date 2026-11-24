// The Episodes and Sources panel's switch of the playing episode to another release: what is
// sent, how the picked row follows the engine's PlayerSwitch, the picture held while it loads,
// and the notice when it lands with the panel closed.

@testable import KaeriPlayerUI
import KaeriKit
import KaeriStrings
import Foundation
import Testing

@MainActor
struct PanelSwitchTests {
    static let release = SourceCandidate(
        id: "r2", providerId: "harbor", providerName: "Harbor Archive", title: "Stardust Wayfarer 07 [1080p]",
        quality: 1080, label: "lantern-archive", size: "1.4 GiB", dualAudio: false, dubbed: false, autoPick: false)
    static let to = SwitchTo(providerId: "harbor", name: "Harbor Archive", release: "r2", stream: nil)

    private func playing() async throws -> Rig {
        let rig = Rig()
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
        try await rig.opened()
        return rig
    }

    @Test func openingThePanelAsksForTheEpisodesReleasesOnce() async throws {
        let rig = try await playing()
        rig.model.loadPanelReleases()
        rig.model.loadPanelReleases()
        let asked = rig.sent.filter { if case .loadSources(900_001, "7") = $0 { true } else { false } }
        #expect(asked.count == 1)
        rig.model.apply(.sources(id: 900_001, episode: "7", candidates: [Self.release], missed: []))
        #expect(rig.model.panelReleases?.candidates == [Self.release])
        #expect(rig.model.panelReleases?.of("harbor") == [Self.release])
        #expect(rig.model.panelReleases?.of("elsewhere") == [])
    }

    @Test func aReleasePickedSwitchesThisEpisodeFromThePlayhead() async throws {
        let rig = try await playing()
        rig.model.switchTo(Self.release)
        #expect(rig.sent.contains { $0 == .switchSource(id: 900_001, episode: "7", providerId: "harbor", sourceId: "r2") })
        #expect(rig.model.panelSwitch?.phase == .asked)
        let row = SwitchTarget(providerId: "harbor", name: "Harbor Archive", release: "r2")
        #expect(rig.model.panelSwitch?.on(row) == true)
        #expect(rig.model.panelSwitch?.on(SwitchTarget(providerId: "harbor", name: "Harbor Archive", release: "r1")) == false)
    }

    @Test func theRowFollowsFindingLoadingAndLanded() async throws {
        let rig = try await playing()
        rig.model.panelOpen = true
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .finding(to: Self.to)))
        #expect(rig.model.panelSwitch?.phase == .finding)
        #expect(!rig.model.holding)

        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.to, at: 312)))
        #expect(rig.model.panelSwitch?.phase == .loading)
        #expect(rig.model.holding, "the picture is held while the release loads")
        try await until("the picture taken") { rig.model.heldPicture != nil }

        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 312)))
        #expect(rig.model.panelSwitch == nil)
        #expect(!rig.model.holding)
        #expect(rig.model.playingSource == "Harbor Archive")
        #expect(rig.model.playingRelease == Self.release)
        #expect(rig.model.alert == nil, "the open panel shows the landing itself")
    }

    @Test func aFailedSwitchStaysOnItsRowAndTriesAgain() async throws {
        let rig = try await playing()
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.to, at: 40)))
        let text = Msg.playerPanelSwitchFailed(source: "Harbor Archive", current: "Quiet Shelf")
        rig.model.apply(.playerSwitch(playback: 1, state: .failed(to: Self.to, at: 40, text: text, reason: nil)))
        #expect(rig.model.panelSwitch?.failed == L.playerPanelSwitchFailed(source: "Harbor Archive", current: "Quiet Shelf"))
        #expect(!rig.model.holding, "the episode plays on from the one before")

        let before = rig.sent.count
        rig.model.retrySwitch()
        #expect(rig.sent[before...].contains { $0 == .switchSource(id: 900_001, episode: "7", providerId: "harbor", sourceId: "r2") })
        #expect(rig.model.panelSwitch?.phase == .asked)
    }

    @Test func aSwitchThatLandsWithThePanelClosedSaysSo() async throws {
        let rig = try await playing()
        rig.model.panelOpen = false
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 312)))
        #expect(rig.model.alert?.text == L.noticePlayingFromSource(source: "Harbor Archive", clock: PlayerClock.label(312)))
    }

    static let dub = SwitchTo(providerId: "", name: "", release: nil, stream: nil, translation: .dub)
    static let dubFound = SwitchTo(providerId: "quiet", name: "Quiet Shelf", release: nil, stream: nil, translation: .dub)

    @Test func theOtherVersionFollowsFindingLoadingAndLanded() async throws {
        let rig = try await playing()
        rig.model.panelOpen = true
        rig.model.loadPanelReleases()
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 30)))
        rig.model.switchTo(version: .dub)
        #expect(rig.controls.last == .choose(.translation(.dub)))
        #expect(rig.model.versionSwitch?.phase == .asked)
        #expect(rig.model.versionSwitch?.to == .dub)

        rig.model.apply(.playerSwitch(playback: 1, state: .finding(to: Self.dub)))
        #expect(rig.model.versionSwitch?.phase == .finding)
        #expect(rig.model.panelSwitch == nil, "no source row shows the version's switch")
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.dubFound, at: 312)))
        #expect(rig.model.versionSwitch?.phase == .loading)
        #expect(rig.model.holding, "the picture is held while the other version loads")

        let asked = rig.sent.count
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.dubFound, at: 312)))
        #expect(rig.model.versionSwitch == nil)
        #expect(!rig.model.holding)
        #expect(rig.model.playingSource == "Quiet Shelf")
        #expect(rig.model.playingRelease == nil, "the release picked was the other version's")
        #expect(rig.sent[asked...].contains { $0 == .loadSources(id: 900_001, episode: "7") }, "the dub's releases are listed")
    }

    @Test func aVersionWaitsInSilenceThenSpinsThenCounts() {
        let start = ContinuousClock.now
        let version = VersionSwitch(to: .dub, phase: .finding, since: start)
        #expect(!version.spins(at: start + .milliseconds(500)))
        #expect(version.spins(at: start + .milliseconds(700)))
        #expect(version.note(episode: "4", at: start + .seconds(2)) == nil)
        #expect(version.note(episode: "4", at: start + .seconds(4)) == L.playerFindingVersionSeconds(episode: "4", seconds: 4))
        let sub = VersionSwitch(to: .sub, phase: .finding, since: start)
        #expect(sub.note(episode: "4", at: start + .seconds(3)) == L.playerFindingVersionSubbedSeconds(episode: "4", seconds: 3))
        let failed = VersionSwitch(to: .dub, phase: .failed("no"), since: start)
        #expect(!failed.spins(at: start + .seconds(5)))
        #expect(failed.note(episode: "4", at: start + .seconds(5)) == nil)
    }

    @Test func aVersionThatFailsSaysSoUnderTheSwitchAndPlaysOn() async throws {
        let rig = try await playing()
        rig.model.panelOpen = true
        rig.model.switchTo(version: .dub)
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.dubFound, at: 40)))
        let text = Msg.playerPanelDubFailed(episode: "7")
        rig.model.apply(.playerSwitch(playback: 1, state: .failed(to: Self.dubFound, at: 40, text: text, reason: nil)))
        #expect(rig.model.versionSwitch?.failed == L.playerPanelDubFailed(episode: "7"))
        #expect(!rig.model.holding)
        #expect(rig.model.alert == nil, "the open panel says it")
        // Tried again from the switch.
        rig.model.switchTo(version: .dub)
        #expect(rig.model.versionSwitch?.phase == .asked)
    }

    @Test func aVersionThatFailsWithThePanelClosedSaysSo() async throws {
        let rig = try await playing()
        rig.model.panelOpen = false
        rig.model.switchTo(version: .dub)
        let text = Msg.playerPanelDubFailed(episode: "7")
        rig.model.apply(.playerSwitch(playback: 1, state: .failed(to: Self.dub, at: 40, text: text, reason: nil)))
        #expect(rig.model.alert?.text == L.playerPanelDubFailed(episode: "7"))
    }

    @Test func anotherSourceTheEngineMovedToIsWhatPlaysNow() async throws {
        let rig = try await playing()
        rig.model.panelOpen = true
        rig.model.loadPanelReleases()
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 30)))
        #expect(rig.model.playingRelease == Self.release)
        // The release's stream broke off and another source took over.
        let other = SwitchTo(providerId: "quiet", name: "Quiet Shelf", release: nil, stream: "3")
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: other, at: 400)))
        #expect(rig.model.playingSource == "Quiet Shelf")
        #expect(rig.model.playingRelease == nil)
        #expect(rig.model.alert == nil, "the engine's own notice says it")
    }

    @Test func anotherStreamOfTheReleasePlayingKeepsTheRelease() async throws {
        let rig = try await playing()
        rig.model.loadPanelReleases()
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 30)))
        let stream = SwitchTo(providerId: "harbor", name: "Harbor Archive", release: nil, stream: "2")
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: stream, at: 60)))
        #expect(rig.model.playingRelease == Self.release)
    }

    @Test func anotherEpisodeForgetsTheReleasesAndThePick() async throws {
        let rig = try await playing()
        rig.model.loadPanelReleases()
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 30)))
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, speed: 1))
        #expect(rig.model.panelReleases == nil)
        #expect(rig.model.playingRelease == nil)
    }

    @Test func aFileAfterThePictureStartsItsOwnWaitWithNoneOfTheTorrentsPeers() async throws {
        let rig = try await playing()
        let step = { (step: StartStep, line: Msg?) in
            rig.model.apply(.startingStep(id: 900_001, episode: "7", step: step, line: line))
        }
        step(.findingPeers(name: "Harbor Archive"), .startFindingPeers)
        step(.found(count: 1, source: "Harbor Archive", quality: 1080, translation: .sub), .startOpeningStream)
        step(.swarm(peers: 12, bytesPerSecond: 3_400_000), nil)
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }
        #expect(rig.model.walk?.pictured == true)
        rig.step(.seconds(300))
        let first = rig.model.walk?.began

        // The switch's file, or the way back from one, or a failover: another file in mpv.
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.to, at: 312)))
        rig.ops(1, [.load(url: "https://cdn-b.example/7.m3u8", options: [])])
        await rig.handedOver()
        rig.mpv.startFile()
        // mpv's events come on their own stream, after the load's own reset.
        try await until("the file started") { rig.model.walk?.began != first }

        let walk = try #require(rig.model.walk)
        let now = rig.model.clock
        #expect(walk.began == now, "its wait counts from its own load")
        #expect(walk.stepLine(at: now) == Msg.startOpeningStream.text)
        let later = now + .seconds(4)
        #expect(!walk.detail(at: later).contains("12"), "\(walk.detail(at: later))")
        #expect(walk.detail(at: later).hasSuffix(L.playerStartSeconds(seconds: 4)), "\(walk.detail(at: later))")
        #expect(walk.speedFact == nil)
    }

    @Test func aSwitchPausesForNothingBeforeItsSpinner() {
        let start = ContinuousClock.now
        let pick = PanelSwitch(target: SwitchTarget(providerId: "harbor", name: "Harbor Archive", release: "r2"), phase: .finding, since: start)
        #expect(pick.waited(at: start + .milliseconds(2900)) == 2)
        #expect(pick.waited(at: start + .seconds(4)) == 4)
        #expect(PanelSwitch.silent == .milliseconds(600))
        #expect(PanelSwitch.counting == .seconds(3))
    }

    @Test func aReleasesLineIsItsOwnFieldsOnlyEachKeptWhole() {
        // Each part's own spaces are kept from breaking, so a part wraps as a whole.
        let whole = { (part: String) in part.replacingOccurrences(of: " ", with: "\u{00A0}") }
        #expect(ReleaseWords.detail(Self.release) == "1080p · \(whole("1.4 GiB")) · lantern-archive")
        var auto = Self.release
        auto.dubbed = true
        auto.autoPick = true
        #expect(ReleaseWords.detail(auto) == "1080p · \(whole("1.4 GiB")) · lantern-archive · \(whole(L.playerReleaseDub)) · \(whole(L.playerReleaseAutomatic))")
    }
}

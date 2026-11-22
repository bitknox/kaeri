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

    @Test func anotherEpisodeForgetsTheReleasesAndThePick() async throws {
        let rig = try await playing()
        rig.model.loadPanelReleases()
        rig.model.switchTo(Self.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.to, at: 30)))
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "8", episodeTitle: nil, speed: 1))
        #expect(rig.model.panelReleases == nil)
        #expect(rig.model.playingRelease == nil)
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

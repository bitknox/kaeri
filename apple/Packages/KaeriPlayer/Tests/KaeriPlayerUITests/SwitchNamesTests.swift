// What the starting screen names after a switch: a later file of the play, one the panel picked
// or the engine moved to, shows its own source and version while it loads, never the first
// file's.

@testable import KaeriPlayerUI
import KaeriKit
import KaeriStrings
import Foundation
import Testing

@MainActor
struct SwitchNamesTests {
    static let first = "Lantern Index"
    static let harbor = SwitchTo(providerId: "harbor", name: "Harbor Index", release: "harbor-subs", stream: nil)
    static let ember = SwitchTo(providerId: "lantern", name: "Lantern Index", release: "ember", stream: nil)
    static let dub = SwitchTo(providerId: "harbor", name: "Harbor Index", release: nil, stream: nil, translation: .dub)

    /// Episode 7 playing subtitled from Lantern Index, the first source the start asked, past
    /// its first picture.
    private func pictured() async throws -> Rig {
        let rig = Rig()
        rig.model.apply(.playbackStaged(id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1))
        try await rig.opened()
        step(rig, .askingSource(name: Self.first, place: 1, total: 2, pinned: true), .startAskingSource(name: Self.first))
        step(rig, .pinMissed(name: "Quiet Shelf"), nil)
        step(rig, .found(count: 2, source: Self.first, quality: 1080, translation: .sub), .startOpeningStream)
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }
        rig.step(.seconds(300))
        return rig
    }

    private func step(_ rig: Rig, _ step: StartStep, _ line: Msg?) {
        rig.model.apply(.startingStep(id: 900_001, episode: "7", step: step, line: line))
    }

    /// Another file in mpv, as a switch, a failover or the way back loads one, and its wait
    /// 25 s on, when every line the screen has is up.
    private func anotherFile(_ rig: Rig, _ url: String = "https://cdn-b.example/7.m3u8") async throws -> StartingWalk {
        let before = rig.model.walk?.began
        rig.ops(1, [.load(url: url, options: [])])
        await rig.handedOver()
        rig.mpv.startFile()
        try await until("the file started") { rig.model.walk?.began != before }
        rig.step(.seconds(25))
        return try #require(rig.model.walk)
    }

    /// Every line of the screen, its Skip and its version switch, at `now`.
    private func shown(_ walk: StartingWalk, at now: ContinuousClock.Instant) -> String {
        [walk.stepLine(at: now), walk.detail(at: now), walk.skipLabel(at: now) ?? ""].joined(separator: " | ")
    }

    @Test func aSourcePickedInThePanelIsTheOneItsWaitNames() async throws {
        let rig = try await pictured()
        rig.model.switchTo(PanelSwitchTests.release)
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.harbor, at: 312)))
        let walk = try await anotherFile(rig)
        let now = rig.model.clock
        #expect(walk.detail(at: now).hasPrefix(L.playerStartFrom(name: "Harbor Index")), "\(shown(walk, at: now))")
        #expect(walk.stepLine(at: now) == L.playerStartStillWaiting(name: "Harbor Index"))
        #expect(!shown(walk, at: now).contains(Self.first), "\(shown(walk, at: now))")
        #expect(!shown(walk, at: now).contains("Quiet Shelf"), "the start's pinned source is not this file's")

        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.harbor, at: 312)))
        #expect(rig.model.playingSource == "Harbor Index")
        #expect(rig.model.walk?.detail(at: now).hasPrefix(L.playerStartFrom(name: "Harbor Index")) == true)
    }

    @Test func anotherReleaseOfTheSameSourceNamesThatSource() async throws {
        let rig = try await pictured()
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.harbor, at: 100)))
        _ = try await anotherFile(rig)
        rig.mpv.emit(.playbackRestart)
        try await until("the picture") { rig.model.hasPicture }

        // Back to Lantern Index, on another of its releases.
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.ember, at: 140)))
        let walk = try await anotherFile(rig, "https://cdn-c.example/7.m3u8")
        let now = rig.model.clock
        #expect(walk.detail(at: now).hasPrefix(L.playerStartFrom(name: Self.first)), "\(shown(walk, at: now))")
        #expect(!shown(walk, at: now).contains("Harbor Index"), "\(shown(walk, at: now))")
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.ember, at: 140)))
        #expect(rig.model.playingSource == Self.first)
    }

    @Test func theOtherVersionsFileIsThatVersionFromItsOwnSource() async throws {
        let rig = try await pictured()
        rig.model.panelOpen = true
        rig.model.switchTo(version: .dub)
        rig.model.apply(.playerSwitch(playback: 1, state: .finding(to: PanelSwitchTests.dub)))
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.dub, at: 312)))
        let walk = try await anotherFile(rig)
        let now = rig.model.clock
        #expect(walk.translation == .dub, "the screen's switch shows Dubbed")
        #expect(walk.detail(at: now).hasPrefix(L.playerStartFrom(name: "Harbor Index")), "\(shown(walk, at: now))")
        #expect(!shown(walk, at: now).contains(Self.first), "\(shown(walk, at: now))")
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: Self.dub, at: 312)))
        #expect(rig.model.walk?.translation == .dub)
        #expect(rig.model.playingSource == "Harbor Index")
    }

    @Test func aVersionThatFailsGoesBackToTheOneThatPlayed() async throws {
        let rig = try await pictured()
        rig.model.switchTo(version: .dub)
        rig.model.apply(.playerSwitch(playback: 1, state: .loading(to: Self.dub, at: 312)))
        _ = try await anotherFile(rig)
        // It showed nothing: the way back is the file that played before.
        let walk = try await anotherFile(rig, "https://cdn-a.example/7.m3u8")
        let now = rig.model.clock
        #expect(!shown(walk, at: now).contains("Harbor Index"), "\(shown(walk, at: now))")
        let text = Msg.playerPanelDubFailed(episode: "7")
        rig.model.apply(.playerSwitch(playback: 1, state: .failed(to: Self.dub, at: 312, text: text, reason: nil)))
        #expect(rig.model.walk?.translation == .sub)
        #expect(rig.model.walk?.detail(at: now).hasPrefix(L.playerStartFrom(name: Self.first)) == true)
    }

    @Test func aStreamTheEngineMovedToNamesNoSourceUntilItLands() async throws {
        let rig = try await pictured()
        // The stream broke off, and the engine tries another, which may be another source's.
        step(rig, .tryingAnotherStream(next: 2, total: 2), .startTryingAnotherStream)
        let walk = try await anotherFile(rig)
        let now = rig.model.clock
        #expect(!shown(walk, at: now).contains(Self.first), "\(shown(walk, at: now))")

        let moved = SwitchTo(providerId: "harbor", name: "Harbor Index", release: nil, stream: "1")
        rig.model.apply(.playerSwitch(playback: 1, state: .landed(to: moved, at: 400)))
        #expect(rig.model.playingSource == "Harbor Index")
        #expect(rig.model.walk?.stepLine(at: now) == L.playerStartStillWaiting(name: "Harbor Index"))
    }

    @Test func aStallSwitchNamesNoSourceUntilItLands() async throws {
        let rig = try await pictured()
        rig.model.switchSource()
        let walk = try await anotherFile(rig)
        let now = rig.model.clock
        #expect(!shown(walk, at: now).contains(Self.first), "\(shown(walk, at: now))")
    }
}

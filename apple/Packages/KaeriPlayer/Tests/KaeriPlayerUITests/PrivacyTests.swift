// What the player lets out while private: the label, and nothing handed off or named on the
// system's media controls.

import KaeriKit
import KaeriStore
import Testing

@testable import KaeriPlayerUI

@MainActor
struct PlayerPrivacyTests {
    func staged(_ rig: Rig, alwaysPrivate: Bool = false) {
        rig.model.apply(.playbackStaged(
            id: 900_001, title: "Stardust Wayfarer", episode: "7", episodeTitle: nil, speed: 1,
            alwaysPrivate: alwaysPrivate))
    }

    @Test func privateModeMakesWhatPlaysPrivate() {
        let rig = Rig()
        staged(rig)
        #expect(!rig.model.isPrivate)
        #expect(rig.model.handoff(title: { _ in nil }) != nil)
        rig.model.apply(.private(true))
        #expect(rig.model.isPrivate)
        #expect(rig.model.handoff(title: { _ in nil }) == nil, "nothing offered to other devices")
        rig.model.apply(.private(false))
        #expect(!rig.model.isPrivate)
    }

    @Test func anAlwaysPrivateShowIsPrivateWithTheModeOff() {
        let rig = Rig()
        staged(rig, alwaysPrivate: true)
        #expect(rig.model.isPrivate)
        #expect(rig.model.handoff(title: { _ in nil }) == nil)
        #expect(NowPlaying.playing(rig.model) == nil, "no Live Activity")
    }

    @Test func turningAlwaysPrivateOnForWhatPlaysTakesAtOnce() {
        let rig = Rig()
        staged(rig)
        rig.model.apply(.alwaysPrivate(title: .show(id: 900_002), on: true))
        #expect(!rig.model.isPrivate, "another show")
        rig.model.apply(.alwaysPrivate(title: .show(id: 900_001), on: true))
        #expect(rig.model.isPrivate)
        rig.model.apply(.alwaysPrivate(title: .show(id: 900_001), on: false))
        #expect(!rig.model.isPrivate)
    }
}

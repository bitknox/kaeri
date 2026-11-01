// The player drawing subtitles in the engine's options: the style as the viewer set it, the
// system's caption style when the style follows it, and the Raise composed with the controls'
// lift. The options themselves are the engine's (`mpv_subtitle_options`, tested in Rust); made-up
// styles throughout, and nothing here reads or changes the machine's own.

import KaeriKit
import KaeriPlayer
import Foundation
import Testing

@testable import KaeriPlayerUI

@MainActor
struct SubtitleStylePlayerTests {
    /// The system style until the viewer changes it: white text, no plate behind the letters, a
    /// half-black box behind the lines, no edge.
    static let systemDefault = SystemCaptionStyle(
        fontFamily: nil, relativeSize: 1, textRgba: 0xFFFF_FFFF, backgroundRgba: 0x0000_0000,
        windowRgba: 0x0000_0080, edge: .plain)

    /// The values the player set on mpv for `property`, in order.
    private func sets(_ rig: Rig, _ property: String) -> [String] {
        rig.mpv.calls.compactMap { call in
            if case .set(property: property, let value) = call { value.string } else { nil }
        }
    }

    /// The last `sub-pos` the player set as a number, as the lift does.
    private func lifted(_ rig: Rig) -> Double? {
        rig.mpv.calls.compactMap { call in
            if case .set("sub-pos", .double(let value)) = call { value } else { nil }
        }.last
    }

    @Test func theNextMpvStartsInTheStyleTheSystemAndTheSettingGive() {
        let rig = Rig()
        rig.model.systemCaptionStyle = { Self.systemDefault }
        #expect(rig.model.captionOptions()["sub-border-style"] == "background-box")
        var style = rig.model.subtitleStyle
        style.followSystem = false
        rig.model.subtitleStyle = style
        #expect(rig.model.captionOptions()["sub-border-style"] == "outline-and-shadow")
        #expect(rig.model.captionOptions()["sub-color"] == "#FFFFFFFF")
        // The default font is always set, so a font chosen before does not stay.
        #expect(rig.model.captionOptions()["sub-font"] == PlayerModel.defaultSubtitleFont)
    }

    @Test func aChangeOfStyleReachesTheMpvPlayingNow() async throws {
        let rig = Rig()
        try await rig.opened()
        var style = rig.model.subtitleStyle
        // The system's own colours win while it is followed.
        style.followSystem = false
        style.colour = .yellow
        style.styled = .restyle
        rig.model.subtitleStyle = style
        await rig.handedOver()
        #expect(sets(rig, "sub-color") == ["#FFFFFF00"])
        #expect(sets(rig, "sub-ass-override") == ["force"])
    }

    @Test func theSystemsStyleReachesItOnlyWhileTheStyleFollowsIt() async throws {
        let rig = Rig()
        var system = Self.systemDefault
        rig.model.systemCaptionStyle = { system }
        try await rig.opened()
        system.textRgba = 0xFFFF_00FF
        rig.model.captionStyleChanged()
        await rig.handedOver()
        #expect(sets(rig, "sub-color") == ["#FFFFFF00"])
        var style = rig.model.subtitleStyle
        style.followSystem = false
        rig.model.subtitleStyle = style
        await rig.handedOver()
        #expect(sets(rig, "sub-color").last == "#FFFFFFFF")
        rig.model.captionStyleChanged()
        await rig.handedOver()
        #expect(sets(rig, "sub-color").count == 2)
    }

    @Test func raiseAndTheControlsLiftComposeInSubPos() async throws {
        let rig = Rig()
        try await rig.opened()
        var style = rig.model.subtitleStyle
        style.raise = 6
        rig.model.subtitleStyle = style
        rig.model.liftSubtitles(0.1, animated: false)
        await rig.handedOver()
        #expect(lifted(rig) == 84)
        rig.model.liftSubtitles(0, animated: false)
        await rig.handedOver()
        #expect(lifted(rig) == 94)
    }

    @Test func withNothingPlayingAChangeSendsNothing() async throws {
        let rig = Rig()
        rig.model.systemCaptionStyle = { Self.systemDefault }
        rig.model.captionStyleChanged()
        var style = rig.model.subtitleStyle
        style.followSystem = false
        rig.model.subtitleStyle = style
        #expect(rig.made.isEmpty)
    }
}

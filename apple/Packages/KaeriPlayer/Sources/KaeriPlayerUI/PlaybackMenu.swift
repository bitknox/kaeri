// The menu bar's Playback menu and Window › Picture in Picture, as MacMenuBar draws them, carried
// out on the player the app hosts. The app's shell puts them in its menus.

import KaeriKit
import KaeriStore
import KaeriStrings
import SwiftUI

extension FocusedValues {
    /// Whether the player's window is the one in front, set by the app on the player's scene.
    /// The Playback menu's keys without a modifier (Space, the arrows, S, M, [ and ]) only act
    /// then, so typing in the main window never pauses the player.
    @Entry public var playerInFront: Bool?
}

/// The items of the Playback menu. Dimmed in place when nothing plays, never hidden.
public struct PlaybackMenu: View {
    let model: PlayerModel
    @FocusedValue(\.playerInFront) private var inFront

    public init(model: PlayerModel) {
        self.model = model
    }

    /// The speeds the Speed menu offers, slowest first.
    static let speeds: [Double] = [0.5, 0.75, 1, 1.25, 1.5, 2]

    public var body: some View {
        let playing = model.isPresented
        // Keys with no modifier only while the player is in front. Out of it they are not bound
        // at all, not only dimmed: AppKit stops at the first menu item with a key, enabled or
        // not, so a dimmed Space or arrow here would swallow the reader's and the main window's.
        let keys = playing && inFront == true
        Button(model.paused ? L.actionPlay : L.actionPause) { model.togglePause() }
            .plainKey(.space, when: keys)
            .disabled(!keys)
        Divider()
        Button(L.menuSkipForward) { model.seek(by: 10) }
            .plainKey(.rightArrow, when: keys)
            .disabled(!keys)
        Button(L.menuSkipBack) { model.seek(by: -10) }
            .plainKey(.leftArrow, when: keys)
            .disabled(!keys)
        Divider()
        Button(L.menuNextEpisode) { model.step(1) }
            .keyboardShortcut(.rightArrow, modifiers: .command)
            .disabled(!playing)
        Button(L.menuPreviousEpisode) { model.step(-1) }
            .keyboardShortcut(.leftArrow, modifiers: .command)
            .disabled(!playing)
        Button(model.skip.map { $0.kind == .ending ? L.menuSkipEnding : $0.kind == .recap ? L.menuSkipRecap : L.menuSkipOpening } ?? L.menuSkipOpening) { model.takeSkip() }
            .plainKey("s", when: keys)
            .disabled(!keys || model.skip == nil)
        Button(L.menuSaySomething) { model.requestChat() }
            .disabled(!playing || !model.inParty)
        Button(L.menuMarkSkipTimes) { model.beginSkipMark() }
            .keyboardShortcut("k", modifiers: .option)
            .disabled(!model.canMarkSkips)
        Divider()
        let style = model.subtitleStyle
        Button(L.playerSubsLarger) { model.setSubtitleSetting?(.subtitleSize, .int(Int64(min(200, Int(style.size) + 10)))) }
            .keyboardShortcut("=", modifiers: [.option, .command])
            .disabled(!playing || style.size >= 200)
        Button(L.playerSubsSmaller) { model.setSubtitleSetting?(.subtitleSize, .int(Int64(max(50, Int(style.size) - 10)))) }
            .keyboardShortcut("-", modifiers: [.option, .command])
            .disabled(!playing || style.size <= 50)
        Button(L.playerSubsRaise) { model.setSubtitleSetting?(.subtitleRaise, .int(Int64(min(20, Int(style.raise) + 1)))) }
            .keyboardShortcut(.upArrow, modifiers: [.option, .command])
            .disabled(!playing || style.raise >= 20)
        Button(L.playerSubsLower) { model.setSubtitleSetting?(.subtitleRaise, .int(Int64(max(0, Int(style.raise) - 1)))) }
            .keyboardShortcut(.downArrow, modifiers: [.option, .command])
            .disabled(!playing || style.raise <= 0)
        Menu(L.settingsSubsStyled) {
            Button(L.settingsSubsStyledKeep) { model.setSubtitleSetting?(.styledSubtitles, .str(StyledSubtitles.keep.fileName)) }
            Button(L.settingsSubsStyledRestyle) { model.setSubtitleSetting?(.styledSubtitles, .str(StyledSubtitles.restyle.fileName)) }
            Button(L.settingsSubsStyledStrip) { model.setSubtitleSetting?(.styledSubtitles, .str(StyledSubtitles.strip.fileName)) }
        }
        .disabled(!playing)
        Divider()
        Menu(L.playerSpeed) {
            Button(L.playerSlower) { model.setSpeed(Self.step(model.speed, by: -1)) }
                .plainKey("[", when: keys)
                .disabled(!keys || model.speed <= Self.speeds.first!)
            Button(L.playerFaster) { model.setSpeed(Self.step(model.speed, by: 1)) }
                .plainKey("]", when: keys)
                .disabled(!keys || model.speed >= Self.speeds.last!)
            Divider()
            ForEach(Self.speeds, id: \.self) { speed in
                Toggle(Self.name(speed), isOn: Binding(
                    get: { abs(model.speed - speed) < 0.01 },
                    set: { _ in model.setSpeed(speed) }
                ))
                .keyboardShortcut(speed == 1 && keys ? KeyboardShortcut(.delete, modifiers: []) : nil)
            }
        }
        .disabled(!playing)
        Menu(L.menuSubtitlesAndAudio) {
            Section(L.playerAudio) {
                ForEach(model.audioTracks) { track in
                    Toggle(track.name, isOn: Binding(
                        get: { track.id == model.audioTrack },
                        set: { _ in model.selectAudio(track.id) }
                    ))
                }
            }
            Section(L.playerSubtitles) {
                Toggle(L.valueOff, isOn: Binding(
                    get: { model.subtitleTrack == nil },
                    set: { _ in model.selectSubtitle(nil) }
                ))
                ForEach(model.subtitleTracks) { track in
                    Toggle(track.name, isOn: Binding(
                        get: { track.id == model.subtitleTrack },
                        set: { _ in model.selectSubtitle(track.id) }
                    ))
                }
            }
            #if os(macOS)
            // Last, apart from the picks the engine remembers: a file is this episode's alone.
            Divider()
            Button(TrackFiles.title(.sub)) { TrackFiles.load(.sub, into: model) }
            Button(TrackFiles.title(.audio)) { TrackFiles.load(.audio, into: model) }
            #endif
        }
        .disabled(!playing)
        Divider()
        Button(L.menuVolumeUp) { model.stepVolume(by: 5) }
            .plainKey(.upArrow, when: keys)
            .disabled(!keys)
        Button(L.menuVolumeDown) { model.stepVolume(by: -5) }
            .plainKey(.downArrow, when: keys)
            .disabled(!keys)
        Button(model.muted ? L.menuUnmute : L.menuMute) { model.toggleMute() }
            .plainKey("m", when: keys)
            .disabled(!keys)
        Divider()
        Button(model.fullScreen ? L.menuExitFullScreen : L.menuEnterFullScreen) { model.fullScreen.toggle() }
            .keyboardShortcut("f", modifiers: [.command, .control])
            .disabled(!playing)
        Toggle(L.menuStatsForNerds, isOn: Binding(get: { model.statsShown }, set: { model.showStats($0) }))
            .plainKey("i", when: keys)
            .disabled(!keys)
        Divider()
        // Dimmed in place when discussions are off, the episode is not far enough on, or nothing plays.
        Button(L.discussionOpen) { model.openDiscussion() }
            .keyboardShortcut("d", modifiers: [.command, .option])
            .disabled(model.discussionOffer == nil)
        // Picture in Picture is the Window menu's, PictureInPictureMenuItem below.
    }

    /// The next speed on the ladder toward `by`'s sign, from wherever `speed` is.
    static func step(_ speed: Double, by: Int) -> Double {
        by < 0
            ? speeds.last { $0 < speed - 0.01 } ?? speeds.first!
            : speeds.first { $0 > speed + 0.01 } ?? speeds.last!
    }

    /// "Normal", "1.25×".
    static func name(_ speed: Double) -> String {
        speed == 1 ? L.menuSpeedNormal : speedLabel(speed, whole: false)
    }
}

/// Window › Picture in Picture: ⌥⌘P.
public struct PictureInPictureMenuItem: View {
    let model: PlayerModel

    public init(model: PlayerModel) {
        self.model = model
    }

    public var body: some View {
        Toggle(
L.menuPictureInPicture,
            isOn: Binding(
                get: { model.pictureInPicturePhase != .inline },
                set: { _ in model.togglePictureInPicture() })
        )
        .keyboardShortcut("p", modifiers: [.command, .option])
        .disabled(!model.pictureInPictureAvailable && model.pictureInPicturePhase == .inline)
    }
}

extension View {
    /// A key with no modifier, bound only `when` it should act; see `PlaybackMenu.body`.
    func plainKey(_ key: KeyEquivalent, when on: Bool) -> some View {
        keyboardShortcut(on ? KeyboardShortcut(key, modifiers: []) : nil)
    }
}

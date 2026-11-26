// The room's states for screenshots (apple/scripts/shots.sh, room=<scene>): one scene per frame
// of the room board (v5). The engine's fake room (`startFakeRoom`) stages each one with scripted
// members on fake relays, and the app plays the room's episode as it would in any room; the
// dialogs, popovers and Move are the app's own, opened over Watching. Testing only: the release
// app has none of this.

#if FIXTURES
import Foundation
import KaeriDesign
import KaeriKit
import KaeriStore
#if os(iOS)
import UIKit
#endif

/// A room's state a screenshot shows: `-KaeriRoom wait`.
public enum RoomScene: String, CaseIterable, Sendable {
    // Before play
    /// Mika and you ready, Ren's player still opening; Play for everyone.
    case before
    /// Play pressed, waiting for Ren (6 s).
    case beforewait
    /// 20 s before play: Start without Ren.
    case beforecap
    /// Everyone ready: "Starting in 3".
    case count
    /// Opened from an invite link: Join the room / Not now.
    case invited
    /// Past 10 people: Join anyway.
    case invitedover
    // Watching
    case watching
    /// Mika paused the room.
    case pausedby
    /// Paused for Ren, loading 6 s.
    case wait
    /// 20 s: Carry on without Ren.
    case cap
    /// Mika carried on without Ren.
    case carried
    /// Ren's own window: the starting screen says the room waits; Don't wait for me.
    case ownwait
    /// Ren buffering in a room of three (the room waits).
    case buffering
    /// Sora buffering in a room of six (no wait).
    case bufferingsix
    // Ren's states
    case renchoosing
    case rennosource
    case renclosed
    case renreconnecting
    case renbehind
    case renepisode
    // Your own states
    case youchoosing
    case younosource
    case youbehind
    case youreconnecting
    // Invite and Details
    case invite
    case details
    /// A relay down: the line says so.
    case relaytrouble
    case detailstrouble
    case editrelays
    // Leaving
    case leave
    case cmdw
    // Up next and Skip
    case upnext
    case skip
    // Move to a new room
    case movestart
    case movepick
    case moveoffer
    case moved
    // Full screen and the strip
    case full
    case fullcap
    /// Six people: three rows that scroll on a short window.
    case six
    /// The column hidden: the strip over the windowed video.
    case hidden
    // iPhone only
    /// The room folded to the bar over the tabs.
    case folded
    /// The More menu open.
    case menu
    /// The Invite sheet.
    case invitesheet
    /// Sideways, writing a line.
    case sidewayswrite
    /// For a recording: the room starts from the player (the morph), 1.5 s after the scene is up.
    case morph
    /// For a recording: into full screen and out again.
    case fullloop
    /// Watch together with no room: Join a room and recent rooms.
    case place
    /// Watch together in a room whose player is closed (Keep the room): the room's card on top.
    case placeroom
    /// The pasted link's relays never answered.
    case placejoinfail
    /// Rejoin found nobody answering in the first recent room.
    case placerecentfail

    public static let argument = "-KaeriRoom"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> RoomScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1) else { return nil }
        return RoomScene(rawValue: arguments[index + 1])
    }

    /// The scene goes full screen, which only an app in the Dock can.
    public var fullScreen: Bool { self == .full || self == .fullcap || self == .fullloop }

    /// The scene is Watch together's page, not the player.
    var place: Bool { rawValue.hasPrefix("place") }

    /// The scene covers the video with the room's own panel: the episode needn't play.
    var panelOnly: Bool {
        switch self {
        case .before, .beforewait, .beforecap, .count, .invited, .invitedover, .youchoosing, .younosource: true
        default: false
        }
    }
}

/// The engine's fake room, kept for the run.
@MainActor
enum RoomStage {
    static var fake: FfiFakeRoom?
    static var ticker: Task<Void, Never>?

    /// The fake members' rooms follow the clock only when ticked (the count before play, Up
    /// next): tick them through the run, off the main thread.
    static func tick(_ fake: FfiFakeRoom) {
        ticker?.cancel()
        ticker = Task.detached {
            while !Task.isCancelled {
                fake.tick()
                try? await Task.sleep(for: .milliseconds(250))
            }
        }
    }

    /// Before play: the room hasn't started, and a tick would start the count.
    static func beforePlay(_ scene: RoomScene) -> Bool {
        switch scene {
        case .before, .beforewait, .beforecap, .count, .invited, .invitedover: true
        default: false
        }
    }

    /// The engine's scene for a board frame; the app's own panels open over Watching.
    static func scene(_ scene: RoomScene) -> FfiRoomScene {
        switch scene {
        case .before: .beforePlay
        case .beforewait: .beforePlayWait
        case .beforecap: .beforePlayCap
        case .count: .starting
        case .invited: .invited
        case .invitedover: .invitedFull
        case .pausedby: .paused
        case .wait: .waitLoading
        case .cap, .fullcap: .waitCap
        case .carried: .carriedOn
        case .ownwait: .youLoading
        case .buffering: .buffering
        case .bufferingsix: .bufferingSix
        case .renchoosing: .choosing
        case .rennosource: .noSource
        case .renclosed: .playerClosed
        case .renreconnecting: .reconnecting
        case .renbehind: .behind
        case .renepisode: .otherEpisode
        case .youchoosing: .youChoosing
        case .younosource: .youNoSource
        case .youbehind: .youBehind
        case .youreconnecting: .youReconnecting
        case .six: .six
        case .relaytrouble, .detailstrouble: .relayTrouble
        case .upnext: .upNext
        case .moveoffer: .moveOffer
        case .moved: .movedAway
        case .placejoinfail: .unreachable
        case .watching, .invite, .details, .editrelays, .leave, .cmdw, .skip, .movestart, .movepick, .full, .hidden,
             .folded, .menu, .invitesheet, .sidewayswrite, .morph, .fullloop, .place, .placeroom, .placerecentfail:
            .watching
        }
    }
}

extension PlayerModel {
    /// Stages a room scene: the engine's fake room in the scene's state, the room's episode
    /// playing in the main window, and the app's own panels the scene opens. True once the room
    /// is on screen as the scene has it.
    public func stage(_ scene: RoomScene, store: AppStore) async -> Bool {
        let room = store.room
        if scene.place { return await stagePlace(scene, store: store) }
        #if os(iOS)
        // A room opens upright; a sideways shot (shots.sh --landscape) stays as the phone is held.
        if ProcessInfo.processInfo.arguments.contains("-KaeriLandscape") { PhoneOrientation.opensUpright = false }
        #endif
        guard let fake = store.startFakeRoom() else { return false }
        RoomStage.fake = fake
        if scene == .morph {
            // For the recording: the plain player first, then Watch together from it.
            guard let item = store.continuing.queue(store.titles).all.first else { return false }
            store.send(.play(id: item.id, episode: String(item.episode)))
            guard await roomWaited(30, { self.isPresented && self.hasPicture }) else { return false }
            hideControls()
            try? await Task.sleep(for: .milliseconds(1500))
            room.start(id: item.id, episode: String(item.episode))
            guard await roomWaited(15, { room.inRoom }) else { return false }
            try? await Task.sleep(for: .milliseconds(900))
            return true
        }
        if scene == .invited || scene == .invitedover {
            // An invite looks into the room without joining it: its banner covers the room's
            // episode, opened here and held, which nobody in the room sees.
            store.send(.play(id: 900_001, episode: "9"))
            guard await roomWaited(30, { self.isPresented && self.hasPicture }) else { return false }
            if !paused { togglePause() }
        }
        let engineScene = RoomStage.scene(scene)
        // The fake blocks while its members act: off the main thread.
        let staged = await Task.detached { fake.stage(scene: engineScene) }.value
        if !staged { print("KaeriDev: the fake room did not stage \(scene.rawValue)") }
        guard staged, await roomWaited(15, { room.active }) else { return false }
        if !RoomStage.beforePlay(scene) {
            RoomStage.tick(fake)
            guard await roomWaited(15, { room.view?.started == true }) else { return false }
        }
        if !scene.panelOnly {
            guard await roomWaited(30, { self.isPresented && self.hasPicture }) else { return false }
        }
        if scene == .ownwait { holdsCurtain = true }
        hideControls()
        // Let the members' states settle, then hold the room's clocks there.
        try? await Task.sleep(for: .milliseconds(600))
        RoomModel.stillAt = Int64((Date.now.timeIntervalSince1970 * 1000).rounded())
        room.shown = scene != .hidden && scene != .folded

        switch scene {
        case .invite, .movestart, .invitesheet: room.popover = .invite
        case .details, .detailstrouble: room.popover = .details
        case .editrelays: room.popover = .editRelays
        case .leave: room.leaving = .leave
        case .cmdw: room.leaving = .close
        case .movepick:
            // Ren muted: Move leaves a muted member unticked.
            if let ren = room.members.first(where: { $0.name == "Ren" }) { room.mute(ren, true) } // i18n-skip
            _ = await roomWaited(5) { room.members.contains { $0.muted } }
            room.startMove()
        case .skip:
            room.stage(skipTo: 20)
        case .sidewayswrite:
            room.draft = "that bridge again" // i18n-skip
            room.writing = true
        case .full, .fullcap:
            fullScreen = true
            try? await Task.sleep(for: .milliseconds(1800))
        case .fullloop:
            Task { @MainActor in
                try? await Task.sleep(for: .milliseconds(1500))
                self.fullScreen = true
                try? await Task.sleep(for: .milliseconds(4000))
                self.fullScreen = false
            }
        default: break
        }
        // The column's contents fade in after the window grows.
        try? await Task.sleep(for: .milliseconds(900))
        return room.active
    }

    /// Watch together's page: three recent rooms on Fixtures titles, and the room or failure
    /// named. The engine keeps no recent rooms yet (LoadRecent answers none), so the page's list
    /// is staged.
    private func stagePlace(_ scene: RoomScene, store: AppStore) async -> Bool {
        let room = store.room
        guard await roomWaited(30, { store.titles.entries.count >= 3 }) else { return false }
        let kept = store.titles.entries.values.sorted { $0.title < $1.title }.prefix(3).map { $0 }
        let recent = [
            RecentRoom(roomId: "r1", title: kept[0].title, id: kept[0].id, episode: "8",
                       others: ["Mika", "Ren"], lastAt: "19:25", quiet: scene == .placerecentfail), // i18n-skip
            RecentRoom(roomId: "r2", title: kept[1].title, id: kept[1].id, episode: "3",
                       others: ["Mika"], lastAt: "Tuesday"), // i18n-skip
            RecentRoom(roomId: "r3", title: kept[2].title, id: kept[2].id, episode: "1",
                       others: [], lastAt: "18 November"), // i18n-skip
        ]
        room.stage(recent: recent, joinFailed: nil)
        switch scene {
        case .placejoinfail:
            // The engine's link whose relays never answer.
            guard let fake = store.startFakeRoom() else { return false }
            RoomStage.fake = fake
            room.joinText = fake.invite(by: "Mika")
            _ = await Task.detached { fake.stage(scene: .unreachable) }.value
            guard await roomWaited(40, { room.joinFailure != nil }) else { return false }
            room.stage(recent: recent, joinFailed: room.joinFailed)
        case .placeroom:
            // In a room whose player was closed with Keep the room.
            guard let fake = store.startFakeRoom() else { return false }
            RoomStage.fake = fake
            _ = await Task.detached { fake.stage(scene: .watching) }.value
            guard await roomWaited(30, { room.inRoom && self.isPresented }) else { return false }
            room.keepRoom()
            guard await roomWaited(10, { room.you?.state == .playerClosed }) else { return false }
            room.shown = false
        default:
            break
        }
        RoomModel.stillAt = Int64((Date.now.timeIntervalSince1970 * 1000).rounded())
        try? await Task.sleep(for: .milliseconds(900))
        return scene != .placeroom || room.active
    }

    private func roomWaited(_ seconds: Int, line: Int = #line, _ condition: @MainActor () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !condition() {
            guard ContinuousClock.now < deadline else {
                print("KaeriDev: the room's wait at RoomShots.swift:\(line) ran out")
                return false
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif

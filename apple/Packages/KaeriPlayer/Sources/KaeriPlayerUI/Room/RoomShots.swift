// The room's states for screenshots (apple/scripts/shots.sh, room=<scene>): one scene per frame
// of the room board (v5), staged on `RoomModel` around a Fixtures episode playing in the main
// window. Testing only: the release app has none of this. Until the engine's fake room lands,
// the scenes set the room's state directly.

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

/// The board's room: Ada (this device), Mika and Ren, at 12:04 in Episode 9.
@MainActor
enum RoomStage {
    static let link = "https://kaeri.app/r#k=7Hq2Lw9xVf3pT0aN"
    static let relays = [
        RoomRelayHealth(url: "wss://relay-one.example", healthy: true, rttMs: 38),
        RoomRelayHealth(url: "wss://relay-two.example", healthy: true, rttMs: 61),
        RoomRelayHealth(url: "wss://relay-three.example", healthy: true, rttMs: 112),
    ]
    static let position: Double = 724

    static func line(_ id: UInt64, _ text: String) -> RoomLine { RoomLine(id: id, kind: .system(text)) }
    static func chat(_ id: UInt64, _ from: String, _ text: String) -> RoomLine {
        RoomLine(id: id, kind: .chat(from: from, text: text, you: from == "Ada"))
    }

    // The board's own lines: made-up talk, in the room's words.
    static func conversation(_ episode: String) -> [RoomLine] {
        [
            line(1, "Mika started the room · Episode \(episode)"), // i18n-skip
            chat(2, "Mika", "everyone here?"), // i18n-skip
            chat(3, "Ren", "yes, go"), // i18n-skip
            line(4, "Ren paused at 3:10"), // i18n-skip
            chat(5, "Ren", "back in two, kettle"), // i18n-skip
            line(6, "Ren resumed"), // i18n-skip
            RoomLine(id: 7, kind: .reaction(from: "Ren", reaction: .laugh, at: "11:52")),
            chat(8, "Ada", "the relay handoff at the bridge"), // i18n-skip
            chat(9, "Mika", "every week this song"), // i18n-skip
        ]
    }

    static func lobby(_ episode: String) -> [RoomLine] {
        [
            line(1, "Mika started the room · Episode \(episode)"), // i18n-skip
            chat(2, "Mika", "everyone here?"), // i18n-skip
            chat(3, "Ren", "two minutes, making tea"), // i18n-skip
            chat(4, "Ada", "ready when you are"), // i18n-skip
        ]
    }

    static func member(_ name: String, _ state: RoomMemberState, since: Int64 = 0, you: Bool = false) -> RoomMember {
        RoomMember(id: name.lowercased(), name: name, you: you, state: state, sinceMs: since)
    }
}

extension PlayerModel {
    /// Stages a room scene: a Fixtures episode in the main window (held where the board holds
    /// it), and the room around it. True once the room is on screen as the scene has it.
    public func stage(_ scene: RoomScene, store: AppStore) async -> Bool {
        let room = store.room
        if scene.place { return await stagePlace(scene, store: store) }
        guard let item = store.continuing.queue(store.titles).all.first,
              let entry = store.titles[item.id]
        else { return false }
        let now = Int64((Date.now.timeIntervalSince1970 * 1000).rounded())
        RoomModel.stillAt = now
        let episode = String(item.episode)
        let roomEpisode = RoomEpisode(id: item.id, episode: episode, name: "The Long Leg", duration: 1420) // i18n-skip
        store.send(.play(id: item.id, episode: episode))
        guard await roomWaited(30, { self.isPresented && self.hasPicture && self.duration != nil }) else { return false }
        if scene == .ownwait { holdsCurtain = true }
        let held: Bool = switch scene {
        case .watching, .upnext, .skip, .hidden, .six, .bufferingsix, .renbehind, .renepisode, .renchoosing, .rennosource,
             .renclosed, .renreconnecting, .youbehind, .full, .morph, .fullloop, .invite, .details, .editrelays,
             .detailstrouble, .relaytrouble, .leave, .cmdw, .movestart, .movepick, .moved, .folded, .menu, .invitesheet,
             .sidewayswrite, .youreconnecting:
            false
        default:
            true
        }
        if held != paused { togglePause() }
        hideControls()

        func ago(_ seconds: Int64) -> Int64 { now - seconds * 1000 }
        let s = RoomStage.self
        var members = [s.member("Ada", .watching, you: true), s.member("Mika", .watching), s.member("Ren", .watching)]
        var view = RoomView(
            roomId: "r1", title: entry.title, episode: roomEpisode, position: RoomStage.position, positionAtMs: now,
            playing: !held, relays: s.relays, link: s.link)
        var lines = s.conversation(episode)
        var invitation: RoomInvitation?
        var offer: RoomMoveOffer?
        var muted: Set<String> = []

        func set(_ name: String, _ state: RoomMemberState, since: Int64 = 0) {
            if let index = members.firstIndex(where: { $0.name == name }) {
                members[index].state = state
                members[index].sinceMs = since
            }
        }
        func six() {
            members += [s.member("Sora", .watching), s.member("Kenji", .watching), s.member("Noa", .watching)]
        }

        switch scene {
        case .before, .beforewait, .beforecap, .count:
            view.started = false
            view.playing = false
            view.position = 0
            lines = s.lobby(episode)
            members = members.map { var m = $0; m.ready = true; return m }
            switch scene {
            case .before:
                set("Ren", .loading, since: ago(4))
            case .beforewait:
                set("Ren", .loading, since: ago(6))
                view.waiting = [RoomPausedFor(name: "Ren", reason: .loading, sinceMs: ago(6))]
            case .beforecap:
                set("Ren", .loading, since: ago(20))
                view.waiting = [RoomPausedFor(name: "Ren", reason: .loading, sinceMs: ago(20))]
                view.canCarryOn = true
            default:
                view.startingIn = 3
            }
        case .invited, .invitedover:
            let names = scene == .invitedover
                ? ["Mika", "Ren", "Sora", "Kenji", "Noa", "Yui", "Taro", "Hana", "Kai", "Rio"]
                : ["Mika", "Ren"]
            invitation = RoomInvitation(by: "Mika", title: entry.title, episode: roomEpisode, names: names)
            members = names.map { s.member($0, .watching) }
            lines = [s.line(1, RoomCopy.chatOpensWhenJoin)]
        case .watching, .invite, .details, .editrelays, .leave, .cmdw, .full, .hidden, .morph, .fullloop, .folded, .menu,
             .invitesheet, .sidewayswrite, .place, .placeroom, .placejoinfail, .placerecentfail:
            break
        case .pausedby:
            view.pausedBy = "Mika"
            members = members.map { var m = $0; m.state = .paused; m.pausedRoom = m.name == "Mika"; return m }
            lines.append(s.line(10, "Mika paused at 12:04")) // i18n-skip
        case .wait, .cap, .fullcap, .ownwait:
            let secs: Int64 = scene == .cap || scene == .fullcap ? 20 : 6
            set("Ren", .loading, since: ago(secs))
            view.waiting = [RoomPausedFor(name: "Ren", reason: .loading, sinceMs: ago(secs))]
            view.canCarryOn = secs >= 20
            if scene == .ownwait {
                members = members.map { var m = $0; m.you = m.name == "Ren"; return m }
                view.canDontWait = true
            }
        case .carried:
            set("Ren", .loading, since: ago(24))
            view.carriedOn = (by: "Mika", past: ["Ren"])
            lines.append(s.line(10, "Mika carried on without Ren")) // i18n-skip
        case .buffering:
            set("Ren", .buffering, since: ago(3))
            view.waiting = [RoomPausedFor(name: "Ren", reason: .buffering, sinceMs: ago(3))]
        case .bufferingsix:
            six()
            set("Sora", .buffering, since: ago(3))
        case .six:
            six()
            set("Sora", .noSource)
        case .renchoosing: set("Ren", .choosing)
        case .rennosource: set("Ren", .noSource)
        case .renclosed: set("Ren", .playerClosed)
        case .renreconnecting: set("Ren", .reconnecting)
        case .renbehind:
            set("Ren", .behind)
            members[2].behindS = 42
        case .renepisode:
            set("Ren", .otherEpisode)
            members[2].episode = String(item.episode + 1)
        case .youchoosing:
            set("Ada", .choosing)
            room.stage(choices: [
                RoomSourceChoice(id: "a", name: "Riverside", detail: "1080p · 1.2 GB"), // i18n-skip
                RoomSourceChoice(id: "b", name: "Lantern", detail: "720p · 640 MB"), // i18n-skip
                RoomSourceChoice(id: "c", name: "Harbour", detail: "1080p · 1.4 GB"), // i18n-skip
            ])
        case .younosource:
            set("Ada", .noSource)
        case .youbehind:
            set("Ada", .behind)
            members[0].behindS = 9
        case .youreconnecting:
            view.connection = .reconnecting(relaysUp: 0)
            set("Ada", .reconnecting)
        case .relaytrouble, .detailstrouble:
            view.relays[1].healthy = false
            view.relays[1].rttMs = nil
            view.relays[2].healthy = false
            view.relays[2].rttMs = nil
            view.connection = .reconnecting(relaysUp: 1)
        case .upnext:
            view.upNext = RoomUpNext(episode: String(item.episode + 1), name: "Crosswind", endsAtMs: now + 8000) // i18n-skip
        case .skip:
            room.stage(skip: true)
        case .movestart, .movepick:
            members = [s.member("Mika", .watching, you: true), s.member("Ada", .watching), s.member("Ren", .watching),
                       s.member("Taro", .watching)]
            muted = ["Taro"]
        case .moveoffer:
            members = [s.member("Ada", .watching, you: true), s.member("Ren", .watching), s.member("Taro", .watching),
                       s.member("Mika", .left)]
            offer = RoomMoveOffer(id: "m1", by: "Mika", names: ["Ren"], title: entry.title, episode: episode, expiresAtMs: now + 60000)
            lines = Array(s.conversation(episode).prefix(5)) + [s.line(10, "Mika moved to a new room")] // i18n-skip
        case .moved:
            members = [s.member("Taro", .watching, you: true), s.member("Ada", .left), s.member("Ren", .left)]
            view.moved = RoomMoved(by: "Mika", went: ["Ada", "Ren"])
            lines = [
                s.line(1, "Mika started the room · Episode \(episode)"), // i18n-skip
                s.line(2, "Mika moved to a new room"), // i18n-skip
                s.line(3, "Ada went to the new room"), // i18n-skip
                s.line(4, "Ren went to the new room"), // i18n-skip
            ]
        }

        if scene == .morph {
            // For the recording: the plain player first, then Watch together.
            try? await Task.sleep(for: .milliseconds(1500))
        }
        room.stage(view: view, members: members, lines: lines, invitation: invitation, offer: offer, muted: muted)
        room.shown = scene != .hidden && scene != .folded

        switch scene {
        case .invite, .movestart, .invitesheet: room.popover = .invite
        case .details, .detailstrouble: room.popover = .details
        case .editrelays: room.popover = .editRelays
        case .leave: room.leaving = .leave
        case .cmdw: room.leaving = .close
        case .movepick: room.startMove()
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
        #if os(iOS)
        // A room opens upright; a sideways shot (shots.sh --landscape) turns it back as the
        // person turning the phone would.
        if UIDevice.current.userInterfaceIdiom == .phone, ProcessInfo.processInfo.arguments.contains("-KaeriLandscape"),
           let scene = UIApplication.shared.connectedScenes.compactMap({ $0 as? UIWindowScene }).first {
            // After the room's own upright turn, which comes when the player sees the room.
            _ = await roomWaited(5, { PhoneOrientation.now.room })
            try? await Task.sleep(for: .milliseconds(400))
            scene.requestGeometryUpdate(.iOS(interfaceOrientations: .landscapeRight)) { _ in }
            try? await Task.sleep(for: .milliseconds(600))
        }
        #endif
        // The column's contents fade in after the window grows.
        try? await Task.sleep(for: .milliseconds(900))
        return room.active
    }

    /// Watch together's page: the board's three recent rooms, and the room or failure named.
    private func stagePlace(_ scene: RoomScene, store: AppStore) async -> Bool {
        let room = store.room
        // The Fixtures recent rooms give the titles and art.
        store.party.loadTogether()
        guard await roomWaited(30, { store.party.recent.count >= 3 }) else { return false }
        let now = Int64((Date.now.timeIntervalSince1970 * 1000).rounded())
        RoomModel.stillAt = now
        let kept = Array(store.party.recent.prefix(3))
        let recent = [
            RoomRecent(roomId: "r1", title: kept[0].title, id: kept[0].id, episode: "8",
                       others: ["Mika", "Ren"], lastAt: "19:25", quiet: scene == .placerecentfail), // i18n-skip
            RoomRecent(roomId: "r2", title: kept[1].title, id: kept[1].id, episode: "3",
                       others: ["Mika"], lastAt: "Tuesday"), // i18n-skip
            RoomRecent(roomId: "r3", title: kept[2].title, id: kept[2].id, episode: "1",
                       others: [], lastAt: "18 November"), // i18n-skip
        ]
        room.stage(recent: recent, joinFailed: scene == .placejoinfail ? "kaeri.app/r#v1.Q2m4xR9dK7tWbH3nPzL8sYe" : nil)
        if scene == .placeroom {
            let s = RoomStage.self
            let episode = "8"
            let view = RoomView(
                roomId: "r1", title: kept[0].title,
                episode: RoomEpisode(id: kept[0].id, episode: episode, name: "The Long Leg", duration: 1420), // i18n-skip
                position: RoomStage.position, positionAtMs: now, playing: true, relays: s.relays, link: s.link)
            room.stage(view: view,
                       members: [s.member("Ada", .playerClosed, you: true), s.member("Mika", .watching), s.member("Ren", .watching)],
                       lines: s.conversation(episode), invitation: nil, offer: nil, muted: [])
            room.shown = false
        }
        try? await Task.sleep(for: .milliseconds(900))
        return scene != .placeroom || room.active
    }

    private func roomWaited(_ seconds: Int, _ condition: @MainActor () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !condition() {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif

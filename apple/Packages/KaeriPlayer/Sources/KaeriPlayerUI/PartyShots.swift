// A watch party's states for screenshots (apple/scripts/shots.sh, party=<scene>): a party on the
// testing engine's fake Syncplay server, with members the run plays, as the Windows app rehearses
// them. Testing only: the release app has none of this.

#if FIXTURES
import KaeriKit
import KaeriStore
import Foundation

/// A party's state a screenshot shows: `-KaeriParty panel`.
public enum PartyScene: String, CaseIterable, Sendable {
    /// The form from a title's More, for its episode 7, the server empty and the room named.
    case form
    /// Start pressed on an address nothing answers on: "Joining … on …".
    case joining
    /// Start pressed on a port nobody listens on: the form says the server can't be reached.
    case refused
    /// An invite link opened: "Join the party?", with its room and server.
    case invite
    /// The player in a room of three, waiting: Mika ready, Ren not and on another episode, the
    /// party's panel open.
    case panel
    /// Everyone ready, the panel open.
    case ready
    /// Mika jumping ahead: the notice over the video, everyone's player moved with it.
    case sync
    /// Mika saying something: the chat's notice over the video.
    case chat
    /// Mika's line arriving over the playing video: the column at the top leading corner.
    case chatline
    /// Six lines at once: three rows and "n more in chat", the controls hidden.
    case chatbusy
    /// Controls up, a reaction's moment and the ticks of the reactions so far.
    case chatreact
    /// C pressed and a line typed, over the control row.
    case chatfield
    /// Four sends inside the window: the fourth waits ("Wait 3 s").
    case chatwait
    /// A server with chat turned off: C says so.
    case chatoff
    /// A send the server never echoes: Couldn't send and Try again.
    case chatfail
    /// The people popover or sheet with the conversation and its field.
    case chatpanel
    /// A member muted: the row says Muted and the last line offers Unmute.
    case chatmute
    /// Hide chat on: the popover says Chat is hidden.
    case chathidden
    /// A server that takes 40 characters a message: the count in the field.
    case chatshort
    /// The server gone mid-episode: the panel trying again.
    case lost
    /// Leave party pressed in the panel: the episode goes on, just for this device.
    case left
    /// In the room with nothing playing: none of the sources has the episode.
    case nosource
    /// The same, after Choose source: what each source lists.
    case choose
    /// An invite to an episode not out yet: joined, with nothing to choose.
    case unaired
    /// Quitting while in a party (Mac): Leave the party?
    case quit
    /// Closing the window while in a party (Mac): the same question, Leave and Close.
    case close

    public static let argument = "-KaeriParty"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> PartyScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1) else { return nil }
        return PartyScene(rawValue: arguments[index + 1])
    }

    /// The room every scene's party is in.
    static let room = "quiet-harbor-k7m2xq9ad4tnwe3p"

    /// The fake server, held for the run: it stops when let go.
    @MainActor private static var held: FfiFakeParty?

    @MainActor static func server(_ store: AppStore) -> FfiFakeParty? {
        held = store.startFakeParty()
        return held
    }
}

extension PlayerModel {
    /// Bring the party to `scene`, for a shot. False when it did not get there in time.
    public func stage(_ scene: PartyScene, store: AppStore) async -> Bool {
        let party = store.party
        let room = PartyScene.room
        switch scene {
        case .form, .joining, .refused:
            party.open(.episode(id: 900_001, episode: "7"))
            guard await partyWaited(10, { party.filledIn }) else { return false }
            if scene == .form {
                try? await Task.sleep(for: .milliseconds(700))
                return true
            }
            // An address nothing answers on, or a port nobody listens on.
            party.rehearse(server: scene == .joining ? "10.255.255.1:8999" : "127.0.0.1:9")
            party.join()
            let there = scene == .joining ? party.joining : await partyWaited(20) { party.failure != nil }
            try? await Task.sleep(for: .milliseconds(900))
            return there
        case .invite, .unaired:
            guard let fake = PartyScene.server(store) else { return false }
            let (id, episode): (AnilistId, String) = scene == .invite ? (900_001, "7") : (900_002, "6")
            let link = partyInviteLink(invite: PartyInvite(server: fake.server(), room: room, id: id, episode: episode))
            // The title's page is the shot's own (title=<id>).
            party.openInvite(link) { _ in }
            guard await partyWaited(10, { party.filledIn }) else { return false }
            if scene == .invite {
                try? await Task.sleep(for: .milliseconds(700))
                return true
            }
            party.join()
            // The player, up while the sources were asked, closes for the party's sheet.
            let there = await partyWaited(20) { party.noSource != nil && !self.isPresented }
            quiet(store)
            try? await Task.sleep(for: .milliseconds(1500))
            return there
        case .nosource, .choose:
            // In the room, and an episode the sources were asked for and didn't have.
            guard let fake = PartyScene.server(store) else { return false }
            store.send(.partyJoin(server: fake.server(), room: room, name: "Ada"))
            guard await partyWaited(10, { party.inParty }) else { return false }
            quiet(store)
            store.apply(.partyNoSource(
                id: 900_001, episode: "7", why: .notFound,
                reason: .verbatim(text: "could not match this title: sample-source has no episode 7")))
            if scene == .choose {
                try? await Task.sleep(for: .milliseconds(800))
                party.chooseSource()
                // What two sources list, in before the fixture's own answer, which has none.
                store.apply(.sources(id: 900_001, episode: "7", candidates: [
                    SourceCandidate(
                        id: "a", providerId: "sample-source", title: "Stardust Wayfarer 07 1080p", quality: 1080, // i18n-skip
                        label: "lantern-archive", size: "1.2 GiB", dualAudio: false, dubbed: false, autoPick: true), // i18n-skip
                    SourceCandidate(
                        id: "b", providerId: "night-index", title: "Stardust Wayfarer 07 720p", quality: 720, // i18n-skip
                        label: "night-index", size: "640 MiB", dualAudio: false, dubbed: false, autoPick: false), // i18n-skip
                ]))
                guard await partyWaited(20, { party.choice != .looking }) else { return false }
            }
            try? await Task.sleep(for: .milliseconds(900))
            return true
        case .quit, .close:
            guard let fake = PartyScene.server(store) else { return false }
            store.send(.partyJoin(server: fake.server(), room: room, name: "Ada"))
            guard await partyWaited(10, { party.inParty }) else { return false }
            quiet(store)
            party.askingToLeave = scene == .quit ? .quit : .close
            try? await Task.sleep(for: .milliseconds(900))
            return true
        case .panel, .ready, .sync, .chat, .lost, .left, .chatline, .chatbusy, .chatreact, .chatfield, .chatwait,
             .chatoff, .chatfail, .chatpanel, .chatmute, .chathidden, .chatshort:
            return await stagePlaying(scene, store: store)
        }
    }

    /// A room of three around the episode playing: this device first, then Mika and Ren, who
    /// open the same file (Ren the next episode on the panel's shot).
    private func stagePlaying(_ scene: PartyScene, store: AppStore) async -> Bool {
        let party = store.party
        let room = PartyScene.room
        guard let fake = PartyScene.server(store),
              let item = store.continuing.queue(store.titles).all.first
        else { return false }
        // What the server says of chat is in its welcome, so it is set before joining.
        party.chat.hidden = scene == .chathidden
        if scene == .chatoff { fake.setChat(on: false, maxLength: nil) }
        if scene == .chatshort { fake.setChat(on: true, maxLength: 40) }
        store.send(.partyJoin(server: fake.server(), room: room, name: "Ada"))
        guard await partyWaited(10, { party.inParty }) else { return false }
        store.send(.play(id: item.id, episode: String(item.episode)))
        guard await partyWaited(30, { self.isPresented && self.hasPicture && self.duration != nil }) else { return false }
        // Until this device's member has the episode open, for the others to open the same.
        _ = await partyWaited(10) { party.members?.first(where: \.you)?.duration != nil }
        let you = party.members?.first(where: \.you)
        let file = you?.file ?? "Stardust Wayfarer · Episode \(item.episode)"
        let length = you?.duration ?? 60
        // Held at the start, as a room waiting to be ready is, and clear of Up Next.
        if !paused { togglePause() }
        pinnedControls = true
        fake.addMember(room: room, name: "Mika")
        fake.memberOpens(name: "Mika", file: file, duration: length)
        fake.addMember(room: room, name: "Ren")
        fake.memberOpens(name: "Ren", file: scene == .panel ? "Stardust Wayfarer · Episode \(item.episode + 1)" : file, duration: length)
        fake.memberReady(name: "Mika", ready: true)
        fake.memberReady(name: "Ren", ready: false)
        guard await partyWaited(10, { party.members?.count == 3 }) else { return false }
        try? await Task.sleep(for: .milliseconds(800))
        var there = true
        if scene.rawValue.hasPrefix("chat") {
            // The joins' notices have gone, so the column shows chat alone.
            try? await Task.sleep(for: .milliseconds(1200))
            _ = await partyWaited(8) { self.alert == nil }
        }
        switch scene {
        case .panel:
            party.showPanel()
        case .ready:
            fake.memberReady(name: "Ren", ready: true)
            try? await Task.sleep(for: .milliseconds(400))
            party.setReady(true)
            there = await partyWaited(10) { party.members.map(PartyWords.allReady) == true }
            try? await Task.sleep(for: .milliseconds(600))
            party.showPanel()
        case .sync:
            fake.memberSeeks(name: "Mika", to: 20)
            there = await partyWaited(10) { self.alert != nil && self.position >= 19 }
        case .chat:
            fake.memberSays(name: "Mika", message: "Starting in a minute, grabbing tea")
            there = await partyWaited(10) { self.alert != nil }
        case .chatline, .chatbusy:
            // Playing, the controls gone: what arrives shows alone.
            pinnedControls = false
            if paused { togglePause() }
            hideControls()
            if scene == .chatline {
                fake.memberSays(name: "Mika", message: "this part again")
            } else {
                fake.chatFixture(room: room, state: .burst)
            }
            there = await partyWaited(10) { !party.chat.column.isEmpty }
            if scene == .chatbusy {
                let all = await partyWaited(10) { party.chat.column.count >= 6 }
                there = there && all
            }
        case .chatreact:
            // Reactions inside the short fixture's length, in the board's proportions.
            fake.memberSays(name: "Mika", message: "ok starting")
            fake.memberReacts(name: "Sol", reaction: .clap, at: length * 0.14)
            fake.memberSays(name: "Ren", message: "the handoff in the rain, I knew it")
            fake.memberReacts(name: "Ren", reaction: .gasp, at: length * 0.33)
            fake.memberSays(name: "Sol", message: "wait, who is the new runner")
            fake.memberReacts(name: "Mika", reaction: .laugh, at: length * 0.51)
            there = await partyWaited(10) { party.chat.conversation.marks.count >= 2 }
            // The server takes one reaction in two seconds from each member.
            try? await Task.sleep(for: .milliseconds(2500))
            PartyChat.momentStays = .seconds(30)
            fake.memberReacts(name: "Ren", reaction: .laugh, at: length * 0.51)
            let more = await partyWaited(10) { party.chat.moment != nil }
            there = there && more
        case .chatfield:
            chatFixtureLines(fake)
            there = await partyWaited(10) { party.chat.conversation.lines.count >= 4 }
            requestChat()
            try? await Task.sleep(for: .milliseconds(400))
            party.chat.draft = "back in 2, pause if it gets good"
        case .chatwait:
            requestChat()
            try? await Task.sleep(for: .milliseconds(400))
            for text in ["now", "now now", "it's happening"] {
                party.chat.draft = text
                party.chat.submit()
                try? await Task.sleep(for: .milliseconds(300))
            }
            party.chat.draft = "it's really happening"
            there = await partyWaited(10) { party.chat.wait > 0 }
        case .chatoff:
            there = await partyWaited(10) { !party.chat.allowed }
            requestChat()
        case .chatfail:
            fake.swallowChat(swallow: true)
            requestChat()
            try? await Task.sleep(for: .milliseconds(400))
            party.chat.draft = "the music in this part"
            party.chat.submit()
            there = await partyWaited(12) { party.chat.failed != nil }
            party.showPanel()
        case .chatpanel, .chatmute, .chathidden:
            chatFixtureLines(fake)
            there = await partyWaited(10) { party.chat.conversation.lines.count >= 4 }
            if scene == .chatmute {
                party.chat.mute("Ren", true)
                let more = await partyWaited(10) { party.chat.isMuted("Ren") }
            there = there && more
            }
            party.showPanel()
        case .chatshort:
            requestChat()
            try? await Task.sleep(for: .milliseconds(400))
            party.chat.draft = "ng the whole lap and I still can't believe the coach let him run it"
            there = await partyWaited(10) { party.chat.state?.maxLength == 40 }
        case .lost:
            fake.goDown(down: true)
            there = await partyWaited(10) {
                if case .reconnecting = party.party?.connection { true } else { false }
            }
            try? await Task.sleep(for: .milliseconds(1500))
            party.showPanel()
        case .left:
            party.showPanel()
            try? await Task.sleep(for: .milliseconds(1500))
            party.leave()
            there = await partyWaited(10) { !party.inParty }
        default:
            break
        }
        // The popover's or the sheet's own animation in, and the notices' fades.
        try? await Task.sleep(for: .milliseconds(900))
        return there
    }

    /// The conversation of the board's popover frames, with its own line from this device.
    private func chatFixtureLines(_ fake: FfiFakeParty) {
        fake.chatFixture(room: PartyScene.room, state: .conversation)
    }

    /// The join's "You're in the party" toast gone: the shot is of the sheet, and the Mac
    /// harness's own drawing, with the screen locked, cannot draw the window under a toast.
    private func quiet(_ store: AppStore) {
        if let notice = store.notices.current { store.notices.dismiss(notice) }
    }

    private func partyWaited(_ seconds: Int, _ condition: @MainActor () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !condition() {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif

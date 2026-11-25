// The room's words, as the approved board has them (v5). A stand-in: the engine branch owns the
// catalogue (i18n/), and each of these becomes its `L.room…` string when that lands. Nothing else
// in the room's screens holds English.

import Foundation
import KaeriKit

public enum RoomCopy {
    // MARK: Words on a row

    public static let watching = "Watching"
    public static let ready = "Ready"
    public static let pausedIt = "Paused it"
    public static let paused = "Paused"
    public static let behind = "Behind"
    public static let loading = "Loading"
    public static let buffering = "Buffering"
    public static func onEpisode(_ episode: String) -> String { "On episode \(episode)" }
    public static let choosing = "Choosing a source"
    public static func noSourceFor(_ episode: String) -> String { "No source for episode \(episode)" }
    public static let playerClosed = "Player closed"
    public static let reconnecting = "Reconnecting"
    public static let left = "Left"
    public static let muted = "Muted"
    public static let you = "You"
    public static let youMark = "(you)"
    public static func seconds(_ word: String, _ s: Int) -> String { "\(word) · \(s) s" }
    public static let more = "More"
    public static func notWatching(_ n: Int) -> String { "\(n) not watching" }
    public static let allWatching = "All watching"

    // MARK: The room's line

    public static let watchingTogether = "Watching together"
    public static func episodeAt(_ episode: String, _ at: String) -> String { "Episode \(episode) · \(at)" }
    public static func invitedBy(_ name: String) -> String { "\(name) invited you" }
    public static let overCap = "10 people are here already · rooms work best with 10 or fewer"
    public static func relaysDown(_ down: Int, _ of: Int) -> String {
        down == 1 ? "1 of \(of) relays isn't answering · the room carries on" : "\(down) of \(of) relays aren't answering · the room carries on"
    }
    public static let reconnectingToRoom = "Reconnecting to the room"
    public static let noRelay = "No relay is answering · your episode keeps playing"
    public static func youreBehind(_ s: Int) -> String { "You're \(s) s behind · catching up" }
    public static let pausedForYou = "Paused for you"
    public static func areWaiting(_ names: String, _ many: Bool, _ s: Int) -> String {
        "\(names) \(many ? "are" : "is") waiting · \(s) s"
    }
    public static let notStarted = "Not started yet"
    public static let playStartsIt = "Play starts it for everyone"
    public static func startingIn(_ n: Int) -> String { "Starting in \(n)" }
    public static let everyoneReady = "Everyone is ready"
    public static func stillLoading(_ names: String, _ many: Bool) -> String { "\(names) \(many ? "are" : "is") still loading" }
    public static let waited20 = "The room has waited 20 s"
    public static func startingWhen(_ names: String, _ many: Bool) -> String { "Starting when \(names) \(many ? "are" : "is") ready" }
    public static func upNextIn(_ s: Int) -> String { "Up next in \(s) s" }
    public static func playsForEveryone(_ episode: String) -> String { "Episode \(episode) plays for everyone" }
    public static func carriedOn(_ by: String, _ names: String, _ many: Bool) -> String {
        "\(by) carried on · \(many ? "they join" : "\(names) joins") when ready"
    }
    public static func pausedFor(_ names: String) -> String { "Paused for \(names)" }
    public static func paused(_ by: String) -> String { "\(by) paused" }
    public static func at(_ time: String) -> String { "at \(time)" }
    public static func movedTheRoom(_ by: String) -> String { "\(by) moved the room" }
    public static func goAlongAsk(_ others: [String], _ at: String) -> String {
        let asked = others.isEmpty ? "" : " \(list(others)) \(others.count > 1 ? "are" : "is") asked too."
        return "Go along?\(asked) The episode carries on from \(at)."
    }
    public static func movedToNew(_ by: String) -> String { "\(by) moved to a new room" }
    public static func wentAlong(_ names: [String]) -> String { "\(list(names)) went along · this room carries on" }

    // MARK: Buttons

    public static func carryOnWithout(_ names: String) -> String { "Carry on without \(names)" }
    public static func startWithout(_ names: String) -> String { "Start without \(names)" }
    public static let them = "them"
    public static let joinTheRoom = "Join the room"
    public static let joinAnyway = "Join anyway"
    public static let notNow = "Not now"
    public static let goAlong = "Go along"
    public static let stayHere = "Stay here"
    public static let dontWait = "Don't wait for me"
    public static let playForEveryone = "Play for everyone"
    public static let anyoneCanStart = "Anyone in the room can start it"
    public static let waitingFirstFrames = "Waiting for first frames"
    public static let invite = "Invite"
    public static let details = "Details"
    public static let leave = "Leave"
    public static let leaveDots = "Leave…"
    public static let leaveTheRoomDots = "Leave the room…"
    public static let columnTitle = "Watch together"
    public static let saySomething = "Say something"
    public static let send = "Send"
    public static let react = "React"
    public static let hideRoom = "Hide room"
    public static let showRoom = "Show room"
    public static let closeToBar = "Close the room to the bar"

    // MARK: Before play and your own reasons

    public static let upInTheRoom = "Up in the room"
    public static let roomIsWatching = "The room is watching"
    public static func minutes(_ m: Int) -> String { "\(m) min" }
    public static let playsFromYourSources = "Plays from your sources"
    public static func chooseSourceFor(_ episode: String) -> String { "Choose a source for episode \(episode)" }
    public static let roomKeepsWatching = "The room keeps watching; you join at its place."
    public static let playInTheRoom = "Play in the room"
    public static func noneHas(_ episode: String) -> String { "None of your sources has episode \(episode)" }
    public static func lookedIn(_ n: Int) -> String {
        "Kaeri looked in your \(n) sources. You stay in the room and the chat while the others watch."
    }
    public static let youStay = "You stay in the room and the chat."
    public static let openSources = "Open sources"
    public static let searchTitles = "Search titles"
    public static let roomPausedUntil = "The room is paused until this starts"
    public static func startingEpisode(_ episode: String) -> String { "Starting episode \(episode)" }

    // MARK: Chat lines the room says

    public static func startedTheRoom(_ by: String, _ episode: String) -> String { "\(by) started the room · Episode \(episode)" }
    public static func pausedAt(_ by: String, _ at: String) -> String { "\(by) paused at \(at)" }
    public static func resumed(_ by: String) -> String { "\(by) resumed" }
    public static func pausedForLoading(_ names: String) -> String { "Paused for \(names) · loading" }
    public static let pausedForYouLoading = "Paused for you · loading"
    public static func carriedOnWithout(_ by: String, _ names: String) -> String { "\(by) carried on without \(names)" }
    public static func joinedAt(_ name: String, _ at: String) -> String { "\(name) joined at \(at)" }
    public static func skippedOpening(_ by: String) -> String { "\(by) skipped the opening" }
    public static func upNextLine(_ episode: String) -> String { "Up next: episode \(episode) for everyone" }
    public static func wentToNew(_ name: String) -> String { "\(name) went to the new room" }
    public static let chatOpensWhenJoin = "The chat opens when you join"
    public static func reactionVerb(_ reaction: Reaction) -> String {
        switch reaction {
        case .laugh: "laughs"
        case .gasp: "gasps"
        case .clap: "claps"
        case .cry: "cries"
        }
    }
    public static func reacted(_ name: String, _ word: String, _ at: String) -> String { "\(name) \(word) at \(at)" }

    // MARK: Up next and Skip

    public static let upNextForRoom = "Up next for the room"
    public static func playsForEveryoneIn(_ s: Int) -> String { "Plays for everyone in \(s) s" }
    public static func waitingToFinish(_ names: String) -> String { "Waiting for \(names) to finish" }
    public static let skipOpening = "Skip opening"
    public static let cancel = "Cancel"
    public static let playNextNow = "Play next now"

    // MARK: Invite

    public static let inviteBody = "Anyone with this link can join. The link carries the room's key, so only people who have it can read the room."
    public static let inviteBodyPhone = "Anyone with this link can join. Only people with the link can read the room."
    public static let copyLink = "Copy link"
    public static let copyInviteLink = "Copy invite link"
    public static let share = "Share…"
    public static let newInvite = "New invite"
    public static let newInviteNote = "New invite stops the old link working. Everyone in the room moves to the new one."
    public static let stopsOldLink = "Stops the old link"
    public static let moveToNewDots = "Move to a new room…"
    public static let moveNote = "For someone already here: start a new room and pick who comes along."
    public static let scanNote = "Someone beside you can scan this with their camera."
    public static let qrLabel = "Invite as a QR code"
    public static let linkCopied = "Invite link copied"

    // MARK: Details and relays

    public static let encryption = "Encryption"
    public static let endToEnd = "End to end"
    public static let relays = "Relays"
    public static func answering(_ n: Int, _ of: Int) -> String { n == of ? "\(n) answering" : "\(n) of \(of) answering" }
    public static let notAnswering = "Not answering"
    public static func ms(_ n: Int) -> String { "\(n) ms" }
    public static let relaysNote = "Relays pass the room's messages along and can't read them. These are public relays run by others; Kaeri doesn't run any of them. If one stops answering, the room swaps in a spare."
    public static let editRelaysDots = "Edit relays…"
    public static let editRelays = "Edit relays"
    public static let editRelaysNote = "Changes this room's relays for everyone in it. New invites carry the list."
    public static func remove(_ what: String) -> String { "Remove \(what)" }
    public static let add = "Add"
    public static let settingsRelaysNote = "Settings ▸ Playback ▸ Relays sets the list for new rooms."
    public static let done = "Done"

    // MARK: Leave

    public static let leaveTheRoom = "Leave the room?"
    public static let othersKeepWatching = "The others keep watching."
    public static let othersKeepWatchingKeep = "The others keep watching. Keep the room closes the player and keeps your place."
    public static let keepTheRoom = "Keep the room"

    // MARK: Move

    public static let moveToNew = "Move to a new room"
    public static let moveBody = "Everyone you tick is asked to go along. Nobody moves without saying yes, and anyone not ticked stays in this room."
    public static func moveFoot(_ at: String) -> String { "The new room picks up at \(at) with a new key. Its link reaches only the people ticked." }
    public static let alwaysGoes = "Always goes"
    public static let mutedNotTicked = "Muted, so not ticked"
    public static func moveWith(_ n: Int) -> String { n == 1 ? "Move with 1 other" : "Move with \(n) others" }

    // MARK: The place

    public static let placeIntro = "Each of you plays the episode from your own sources; the room keeps you in step. Start a room from any episode with Watch together, in the player or on its page."
    public static let yourRoom = "Your room"
    public static let openThePlayer = "Open the player"
    public static let joinARoom = "Join a room"
    public static let inviteLink = "Invite link"
    public static let paste = "Paste"
    public static let join = "Join"
    public static let tryAgain = "Try again"
    public static let recentRooms = "Recent rooms"
    public static let lastTenMac = "The last ten, kept on this Mac"
    public static let lastTenPhone = "The last ten, kept on this device"
    public static let rejoin = "Rejoin"
    public static let relaysUnreachable = "Couldn't reach this room's relays."
    public static let relaysUnreachableHint = "Ask for a new invite. Kaeri tried the link's 3 relays and 3 spares for 8 s each."
    public static let nobodyAnswered = "Nobody answered in this room."
    public static let nobodyAnsweredHint = "If someone made a new invite, the old one stopped working; ask them for the new link."
    public static func withNames(_ episode: String, _ names: [String], _ when: String) -> String {
        names.isEmpty ? "Episode \(episode) · \(when)" : "Episode \(episode) · with \(list(names)) · \(when)"
    }
    public static let oldInvite = "This invite is from an older version of Kaeri. Ask for a new one."

    // MARK: Phone

    public static func inTheRoom(_ episode: String, _ n: Int) -> String { "Episode \(episode) · \(n) in the room" }

    // MARK: Joining names

    /// "Ren", "Ren and Mika", "3 people".
    public static func names(_ names: [String]) -> String {
        switch names.count {
        case 0: ""
        case 1: names[0]
        case 2: "\(names[0]) and \(names[1])"
        default: "\(names.count) people"
        }
    }

    /// "Ada and Ren", "Ada, Ren and Sora".
    public static func list(_ names: [String]) -> String {
        guard names.count > 1 else { return names.first ?? "" }
        return names.dropLast().joined(separator: ", ") + " and " + names[names.count - 1]
    }
}

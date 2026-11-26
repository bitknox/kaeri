// The room's words, from the catalogue (i18n/en/ui-room.ftl, `L.room…`): the names the room's
// screens use, each one a string of the catalogue. Nothing in the room's screens holds English.

import Foundation
import KaeriKit
import KaeriStrings

public enum RoomCopy {

    // MARK: Words on a row

    public static var watching: String { L.roomStateWatching }
    public static var ready: String { L.roomStateReady }
    public static var pausedIt: String { L.roomStatePausedIt }
    public static var paused: String { L.roomStatePaused }
    public static func behind(_ s: Int) -> String { L.roomStateBehind(seconds: s) }
    public static func loading(_ s: Int) -> String { L.roomStateLoading(seconds: s) }
    public static func buffering(_ s: Int) -> String { L.roomStateBuffering(seconds: s) }
    public static func onEpisode(_ episode: String) -> String { L.roomStateOtherEpisode(episode: episode) }
    public static var choosing: String { L.roomStateChoosing }
    public static func noSourceFor(_ episode: String) -> String { L.roomStateNoSource(episode: episode) }
    public static var playerClosed: String { L.roomStatePlayerClosed }
    public static var reconnecting: String { L.roomStateReconnecting }
    public static var left: String { L.roomStateLeft }
    public static func andMuted(_ word: String) -> String { L.roomStateAndMuted(state: word) }
    public static var you: String { L.roomYou }
    public static var youMark: String { L.roomYouMarker }
    public static func faceLabel(_ name: String, _ state: String) -> String { L.roomFaceLabel(name: name, state: state) }
    public static func moreFaces(_ n: Int) -> String { L.roomMoreFaces(count: n) }
    public static func moreFacesLabel(_ n: Int) -> String { L.roomMoreFacesLabel(count: n) }
    public static var more: String { L.roomMore }
    public static func notWatching(_ n: Int) -> String { L.roomMoreNotWatching(count: n) }
    public static var allWatching: String { L.roomMoreAllWatching }

    // MARK: The room's line

    public static var watchingTogether: String { L.roomLineWatching }
    public static func episodeAt(_ episode: String, _ at: String) -> String { L.roomLineWatchingDetail(episode: episode, at: at) }
    public static func invitedBy(_ name: String) -> String { L.roomInvitedBy(name: name) }
    public static func invitedDetail(_ title: String, _ episode: String) -> String {
        L.roomInvitedDetail(title: title, episode: episode)
    }
    public static func overCap(_ n: Int) -> String { L.roomInvitedOver(count: n) }
    public static func relaysDown(_ down: Int, _ of: Int) -> String { L.roomLineRelaysDown(count: down, total: of) }
    public static var reconnectingToRoom: String { L.roomLineReconnecting }
    public static var noRelay: String { L.roomLineReconnectingDetail }
    public static func youreBehind(_ s: Int) -> String { L.roomLineYouBehind(seconds: s) }
    public static var pausedForYou: String { L.roomLinePausedForYou }
    public static func areWaiting(_ names: [String], _ s: Int) -> String {
        L.roomLineWaitingForYou(names: Self.names(names), count: names.count, seconds: s)
    }
    public static var notStarted: String { L.roomLineNotStarted }
    public static var playStartsIt: String { L.roomLinePlayStarts }
    public static func startingIn(_ n: Int) -> String { L.roomLineStartingIn(seconds: n) }
    public static var everyoneReady: String { L.roomLineEveryoneReady }
    public static func stillLoading(_ names: [String]) -> String { L.roomLineStillLoading(names: Self.names(names), count: names.count) }
    public static func waited(_ s: Int) -> String { L.roomLineWaited(seconds: s) }
    public static func startingWhen(_ names: [String]) -> String { L.roomLineStartingWhen(names: Self.names(names), count: names.count) }
    public static func upNextIn(_ s: Int) -> String { L.roomLineUpNext(seconds: s) }
    public static func playsForEveryone(_ episode: String) -> String { L.roomLineUpNextDetail(episode: episode) }
    public static func carriedOn(_ by: String, _ past: [String]) -> String {
        past.count == 1 ? L.roomLineCarriedOnOne(carrier: by, name: past[0]) : L.roomLineCarriedOnMany(carrier: by)
    }
    public static func pausedFor(_ names: [String]) -> String { L.roomLinePausedFor(names: Self.names(names)) }
    public static func paused(by name: String?) -> String { name.map { L.roomLinePausedBy(name: $0) } ?? L.roomLineYouPaused }
    public static func at(_ time: String) -> String { L.roomLinePausedAt(at: time) }
    public static func movedTheRoom(_ by: String) -> String { L.roomLineMoved(name: by) }
    public static func goAlongAsk(_ others: [String], _ at: String) -> String {
        others.isEmpty
            ? L.roomLineMovedDetailAlone(at: at)
            : L.roomLineMovedDetail(names: list(others), count: others.count, at: at)
    }
    public static func movedToNew(_ by: String) -> String { L.roomLineMovedAway(name: by) }
    public static func wentAlong(_ names: [String]) -> String { L.roomLineWentAlong(names: list(names)) }

    // MARK: Buttons

    /// "Carry on without Ren", or "… without them" for more than one.
    public static func carryOnWithout(_ names: [String]) -> String {
        names.count == 1 ? L.roomCarryOnWithout(name: names[0]) : L.roomCarryOnWithoutThem
    }
    public static func startWithout(_ names: [String]) -> String {
        names.count == 1 ? L.roomStartWithout(name: names[0]) : L.roomStartWithoutThem
    }
    public static var joinTheRoom: String { L.roomJoin }
    public static var joinAnyway: String { L.roomJoinAnyway }
    public static var notNow: String { L.actionNotNow }
    public static var goAlong: String { L.roomGoAlong }
    public static var stayHere: String { L.roomStay }
    public static var dontWait: String { L.roomDontWait }
    public static var playForEveryone: String { L.roomPlayForEveryone }
    public static var anyoneCanStart: String { L.roomAnyoneCanStart }
    public static var waitingFirstFrames: String { L.roomWaitingFirstFramesShort }
    public static var invite: String { L.roomInvite }
    public static var details: String { L.roomDetails }
    public static var leave: String { L.actionLeave }
    public static var leaveDots: String { L.roomLeaveEllipsis }
    public static var leaveTheRoomDots: String { L.roomLeaveMenu }
    public static var columnTitle: String { L.navWatchTogether }
    public static var saySomething: String { L.roomChatField }
    public static var send: String { L.roomChatSend }
    public static var react: String { L.roomReact }
    public static var hideRoom: String { L.roomHide }
    public static var showRoom: String { L.roomShow }
    public static var closeToBar: String { L.roomFold }

    // MARK: Before play and your own reasons

    public static var upInTheRoom: String { L.roomUpInRoom }
    public static var roomIsWatching: String { L.roomRoomIsWatching }
    public static func minutes(_ m: Int) -> String { L.roomMinutes(minutes: m) }
    public static var playsFromYourSources: String { L.roomPlaysFromSources }
    public static func chooseSourceFor(_ episode: String) -> String { L.roomChooseSource(episode: episode) }
    public static var roomKeepsWatching: String { L.roomChooseSourceLine }
    public static var playInTheRoom: String { L.roomPlayInRoom }
    public static func noneHas(_ episode: String) -> String { L.roomNoSourceTitle(episode: episode) }
    public static func lookedIn(_ n: Int) -> String { L.roomNoSourceLine(count: n) }
    public static var youStay: String { L.roomNoSourceLineShort }
    public static var openSources: String { L.roomOpenSources }
    public static var searchTitles: String { L.roomSearchTitles }
    public static func roomPausedUntil(_ s: Int) -> String { L.roomPausedUntilStart(seconds: s) }
    public static func startingEpisode(_ episode: String) -> String { L.roomStartingEpisode(episode: episode) }

    // MARK: Chat

    public static var chatOpensWhenJoin: String { L.roomChatOpens }
    public static var notSent: String { L.roomChatNotSent }
    /// "Ren laughed at 11:52"
    public static func reacted(_ name: String, _ reaction: Reaction, _ at: String) -> String {
        switch reaction {
        case .laugh: L.roomReactedLaugh(name: name, at: at)
        case .gasp: L.roomReactedGasp(name: name, at: at)
        case .clap: L.roomReactedClap(name: name, at: at)
        case .cry: L.roomReactedCry(name: name, at: at)
        }
    }
    public static func reaction(_ reaction: Reaction) -> String {
        switch reaction {
        case .laugh: L.roomReactLaugh
        case .gasp: L.roomReactGasp
        case .clap: L.roomReactClap
        case .cry: L.roomReactCry
        }
    }

    // MARK: Up next and Skip

    public static var upNextForRoom: String { L.roomUpNext }
    public static func upNextEpisode(_ episode: String, _ name: String?) -> String {
        name.map { L.roomUpNextEpisode(episode: episode, title: $0) } ?? L.roomUpNextEpisodePlain(episode: episode)
    }
    public static func playsForEveryoneIn(_ s: Int) -> String { L.roomUpNextCount(seconds: s) }
    public static func waitingToFinish(_ names: [String]) -> String { L.roomUpNextWaiting(name: Self.names(names)) }
    public static var skipOpening: String { L.roomSkipOpening }
    public static var cancel: String { L.actionCancel }
    public static var playNextNow: String { L.roomPlayNextNow }

    // MARK: Invite

    public static var inviteBody: String { "\(L.roomInviteAnyone) \(L.roomInviteKey)" }
    public static var inviteBodyPhone: String { "\(L.roomInviteAnyone) \(L.roomInviteKeyShort)" }
    public static var copyLink: String { L.roomCopyLink }
    public static var copyInviteLink: String { L.roomCopyInviteLink }
    public static var share: String { L.roomShare }
    public static var newInvite: String { L.roomNewInvite }
    public static var newInviteNote: String { L.roomNewInviteLine }
    public static var stopsOldLink: String { L.roomNewInviteShort }
    public static var moveToNewDots: String { L.roomMoveOpen }
    public static var moveNote: String { L.roomMoveOpenLine }
    public static var scanNote: String { L.roomInviteQrLine }
    public static var qrLabel: String { L.roomInviteQr }
    public static var linkCopied: String { L.roomLinkCopied }

    // MARK: Details and relays

    public static var encryption: String { L.roomDetailsEncryption }
    public static var endToEnd: String { L.roomDetailsEndToEnd }
    public static var relays: String { L.roomRelays }
    public static func answering(_ n: Int, _ of: Int) -> String {
        n == of ? L.roomRelaysAnswering(count: n) : L.roomRelaysAnsweringOf(count: n, total: of)
    }
    public static var notAnswering: String { L.roomRelayNotAnswering }
    public static func ms(_ n: Int) -> String { L.roomRelayLatency(ms: n) }
    public static var relaysNote: String { L.roomDetailsRelaysLine }
    public static var editRelaysDots: String { L.roomEditRelays }
    public static var editRelays: String { L.roomEditRelaysTitle }
    public static var editRelaysNote: String { L.roomEditRelaysLine }
    public static func remove(_ what: String) -> String { L.roomRelayRemove(relay: what) }
    public static var add: String { L.roomRelayAdd }
    public static var settingsRelaysNote: String { L.roomEditRelaysSettings }
    public static var done: String { L.actionDone }

    // MARK: Leave

    public static var leaveTheRoom: String { L.roomLeaveTitle }
    public static var othersKeepWatching: String { L.roomLeaveLine }
    public static var othersKeepWatchingKeep: String { L.roomLeaveLineKeep }
    public static var keepTheRoom: String { L.roomKeep }

    // MARK: Move

    public static var moveToNew: String { L.roomMoveTitle }
    public static var moveBody: String { L.roomMoveBody }
    public static func moveFoot(_ at: String) -> String { L.roomMoveFoot(at: at) }
    public static var alwaysGoes: String { L.roomMoveAlwaysGoes }
    public static var mutedNotTicked: String { L.roomMoveMuted }
    public static func moveWith(_ n: Int) -> String { L.roomMoveWith(count: n) }

    // MARK: The place

    public static var placeIntro: String { L.roomPlaceLine }
    public static var yourRoom: String { L.roomPlaceYours }
    public static var openThePlayer: String { L.roomOpenPlayer }
    public static var joinARoom: String { L.roomPlaceJoin }
    public static var inviteLink: String { L.roomPlaceLink }
    public static var paste: String { L.roomPlacePaste }
    public static var join: String { L.actionJoin }
    public static var tryAgain: String { L.actionTryAgain }
    public static var recentRooms: String { L.roomRecent }
    public static var lastTen: String { L.roomRecentNote }
    public static var rejoin: String { L.roomRejoin }
    public static var relaysUnreachable: String { L.roomErrorUnreachableTitle }
    /// What Join tried, in seconds per relay.
    public static func relaysUnreachableHint(_ seconds: Int) -> String { "\(L.roomErrorAskNewInvite) \(L.roomErrorTried(seconds: seconds))" }
    public static var nobodyAnswered: String { L.roomErrorNobodyAnswered }
    public static var nobodyAnsweredHint: String { L.roomErrorNobodyAnsweredLine }
    public static var full: String { L.roomErrorFull }
    public static var oldInvite: String { L.roomErrorOldInvite }
    public static func withNames(_ episode: String, _ names: [String], _ when: String) -> String {
        names.isEmpty
            ? L.roomRecentRowAlone(episode: episode, when: when)
            : L.roomRecentRow(episode: episode, names: list(names), when: when)
    }

    // MARK: Phone

    public static func inTheRoom(_ episode: String, _ n: Int) -> String { L.roomHeaderLine(episode: episode, count: n) }

    // MARK: Joining names

    /// "Ren", "Ren and Mika", "3 people".
    public static func names(_ names: [String]) -> String {
        switch names.count {
        case 0: ""
        case 1: names[0]
        case 2: L.roomNamesAnd(names: names[0], last: names[1])
        default: L.roomNamesPeople(count: names.count)
        }
    }

    /// "Ada and Ren", "Ada, Ren and Sora".
    public static func list(_ names: [String]) -> String {
        guard names.count > 1 else { return names.first ?? "" }
        let head = names.dropLast().dropFirst().reduce(names[0]) { L.roomNamesComma(first: $0, rest: $1) }
        return L.roomNamesAnd(names: head, last: names[names.count - 1])
    }
}

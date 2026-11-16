// The cast session: connect once to a TV with Chromecast built in, and every play goes there
// until Stop casting. The engine holds the session (`FfiTvSender.castConnect`, `castWatch`):
// which TV, what plays, pause and seek shown at once until the TV agrees, the button's state,
// and a session left running joined again when its TV is found. This keeps its latest view on
// the main actor for the button, the mini and the controller, and follows the engine's cast card
// for what only the controller shows: the tracks, Up next, Undo and why a play failed.

import KaeriKit
import KaeriStrings
import Foundation
#if canImport(UIKit)
import UIKit
#endif

/// The engine's side of the session. The app's is the engine's sender; the Fixtures app scripts
/// one.
public protocol CastSessionLink: AnyObject, Sendable {
    func state() -> FfiCastSessionView
    /// The session once it draws differently; nil once the engine has stopped.
    func next() async -> FfiCastSessionView?
    /// Hold the TV, moving `from` there at its second when something plays on this device.
    func connect(key: String, from: FfiCastFrom?) throws
    /// Stop what plays on the TV, close the receiver, let the TV go.
    func disconnect()
    /// Let the TV go, and say what played there and where, to play on here.
    func playHere() -> FfiCastFrom?
    func command(_ command: FfiTvCommand)
    /// The card of what plays now, for its tracks, Up next and the reason it failed.
    func card() -> (any FfiCastSessionProtocol)?
    /// iPhone and iPad: the app leaves the screen, or is back.
    func away(_ away: Bool)
    /// A play sent while the TV is held. The engine takes it there itself; a script starts it.
    func played(_ request: TVPlayRequest)
}

/// The session through the engine's sender.
final class EngineCastLink: CastSessionLink, @unchecked Sendable {
    private let sender: FfiTvSender
    private let watch: FfiCastSessionWatch

    init(sender: FfiTvSender, watch: FfiCastSessionWatch) {
        self.sender = sender
        self.watch = watch
    }

    func state() -> FfiCastSessionView { watch.state() }
    func next() async -> FfiCastSessionView? { await watch.next() }
    func connect(key: String, from: FfiCastFrom?) throws { try sender.castConnect(key: key, from: from) }
    func disconnect() { sender.castDisconnect() }
    func playHere() -> FfiCastFrom? { sender.castPlayHere() }
    func command(_ command: FfiTvCommand) { sender.castCommand(command: command) }
    func card() -> (any FfiCastSessionProtocol)? { sender.castSession() }
    func away(_ away: Bool) { sender.castAway(away: away) }
    func played(_ request: TVPlayRequest) {}
}

/// What the TV last played, kept across launches: the mini shows it while the app joins the
/// session again.
public struct CastLastItem: Codable, Equatable, Sendable {
    public var id: AnilistId?
    public var title: String
    public var episode: String?
    public var episodeTitle: String?
    public var deviceName: String

    static let key = "castLastItem" // i18n-skip

    static func load() -> CastLastItem? {
        guard let data = UserDefaults.standard.data(forKey: key) else { return nil }
        return try? JSONDecoder().decode(CastLastItem.self, from: data)
    }

    static func save(_ item: CastLastItem?) {
        if let item, let data = try? JSONEncoder().encode(item) {
            UserDefaults.standard.set(data, forKey: key)
        } else {
            UserDefaults.standard.removeObject(forKey: key)
        }
    }
}

extension CastModel {
    /// Follow `link`'s session from now on.
    func hold(_ link: any CastSessionLink) {
        self.link = link
        watching?.cancel()
        lastItem = CastLastItem.load()
        take(view: link.state())
        watching = Task { [weak self, link] in
            while let view = await link.next() {
                guard let self, self.link === link else { return }
                self.take(view: view)
            }
        }
    }

    /// The session as the engine draws it now.
    func take(view: FfiCastSessionView) {
        let before = held?.state ?? .idle
        let noteBefore = held?.note
        remember(view)
        if button != view.button { button = view.button }
        guard view.state != .idle else {
            if held != nil {
                held = nil
                heldAt = .now
            }
            rejoining = false
            expanded = false
            dialogShown = false
            asked = false
            expandOnPlay = false
            if active { end() }
            return
        }
        if held != view {
            held = view
            heldAt = .now
        }
        switch view.state {
        case .connecting:
            // Connecting without being asked: a session left running is joined again.
            if before == .idle, !asked { rejoining = true }
        default:
            rejoining = false
            asked = false
        }
        if view.state == .playing, before != .playing || !active, let card = link?.card() {
            // The card of what plays, for its tracks, Up next and the reason a play failed.
            follow(card, request: nil)
        }
        if view.state == .connected, before == .playing, active, stage == .playing || stage == .starting {
            // The episode ended on the TV, which stays held with nothing loaded.
            end()
        }
        if view.state == .connected, before == .playing, view.item == nil, !active {
            // Nothing plays any more: the episode ended with none after it, or a play could not
            // start. The TV stays connected, as the cast button says, and the controller closes.
            expanded = false
        }
        if let item = view.item {
            let last = CastLastItem(
                id: item.id, title: item.title, episode: item.episode, episodeTitle: item.episodeTitle,
                deviceName: view.deviceName ?? tvName)
            if last != lastItem {
                lastItem = last
                CastLastItem.save(last)
            }
        }
        if let note = view.note, note != noteBefore {
            // A note while it plays, once as it first reads: the TV shows subtitles as plain text.
            say?(Notice(kind: .info, text: note.text, hint: nil, msg: note))
        }
        if let name = view.deviceName, !name.isEmpty, name != tvName { tvName = name }
        if before != .playing, view.state == .playing, expandOnPlay {
            expandOnPlay = false
            chooserShown = false
            dialogShown = false
            expanded = true
        }
    }

    /// What the TV played and where, kept past its end: Play here once the TV dropped off, when
    /// neither the engine nor the card still names it. Where it was is moved on to the moment the
    /// episode left the view.
    private func remember(_ view: FfiCastSessionView) {
        if let item = view.item, view.state == .playing, !item.loading, let id = item.id, let episode = item.episode {
            lastPlayed = TVPlayRequest(id: id, title: item.title, episode: episode, from: item.position)
        } else if itemLive, var last = lastPlayed, item?.id == last.id, item?.episode == last.episode {
            last.from = itemPosition()
            lastPlayed = last
        }
    }

    // MARK: What the screens read

    /// A TV is held, connecting or joined again: what is played goes there.
    public var connected: Bool {
        guard let state = held?.state else { return false }
        return state == .connected || state == .playing || state == .connecting
    }

    /// The held TV's name.
    public var heldName: String {
        if let name = held?.deviceName, !name.isEmpty { return name }
        if rejoining, let name = lastItem?.deviceName, !name.isEmpty { return name }
        return tvName
    }

    /// What plays on the TV.
    public var item: FfiCastItem? { held?.item }

    /// The mini shows: something is loaded on the TV, a play on it failed, or the app joins a
    /// session that played.
    public var miniShown: Bool {
        guard let held else { return false }
        switch held.state {
        case .playing: return held.item != nil || active
        case .connecting: return rejoining && lastItem != nil
        case .connected, .error: return active && (stage == .failed || stage == .stopped)
        case .idle: return false
        }
    }

    /// The TV plays it, paused or not: not starting, failed or joining.
    public var itemLive: Bool {
        guard let item, held?.state == .playing else { return false }
        return !item.loading
    }

    public var itemPaused: Bool { item?.paused ?? false }

    /// Where the TV's playhead is: moved on from its last report on this device's clock while it
    /// plays.
    public func itemPosition(at date: Date = .now) -> Double {
        guard let item, !item.loading else { return 0 }
        if item.paused || item.waiting != nil { return item.position }
        let moved = item.position + max(0, date.timeIntervalSince(heldAt))
        return min(moved, item.duration ?? moved)
    }

    /// What the TV last played stands in for what it plays: while a session left running is
    /// joined again, and on the card of a cast that stopped, which keeps its episode.
    private var showsLast: Bool { rejoining || (active && stage == .stopped) }

    /// The show's title, as the mini and the controller show it.
    public var showTitle: String { item?.title ?? now?.title ?? (showsLast ? lastItem?.title : nil) ?? "" }

    /// The show to open from the controller.
    public var showID: AnilistId? { item?.id ?? now?.id ?? (showsLast ? lastItem?.id : nil) }

    /// The episode line under the show's name.
    public var episodeLine: String {
        if let line = now?.episodeLine, active { return line }
        let episode = item?.episode ?? (showsLast ? lastItem?.episode : nil)
        let title = item?.episodeTitle ?? (showsLast ? lastItem?.episodeTitle : nil)
        if let episode {
            if let title, !title.isEmpty { return L.castEpisodeTitled(episode: episode, title: title) }
            return L.castEpisode(episode: episode)
        }
        return title ?? ""
    }

    /// The TV is held with nothing loaded and no card follows it: connected, not starting.
    public var heldIdle: Bool {
        guard let held, held.state == .connected, held.item == nil else { return false }
        return !rejoining && !active
    }

    /// The mini's and the controller's line: the TV, and how it goes.
    public var sessionLine: String {
        let name = heldName
        if rejoining { return L.tvStatusReconnecting(name: name) }
        if heldIdle { return L.castButtonConnected(name: name) }
        if active {
            switch stage {
            case .starting: return card?.phaseLine?.text ?? L.tvStartingOn(name: name)
            case .failed: return L.castCouldntStart(name: name)
            case .stopped: return L.castStopped
            case .playing: break
            }
        } else if item?.loading ?? true {
            return L.tvStartingOn(name: name)
        }
        return itemPaused ? L.castStatusPaused(name: name) : L.tvStatusPlaying(name: name)
    }

    /// The TV waits for the stream in the middle of the episode: from four seconds on, the line
    /// under the seek line, counting.
    public var waitingSeconds: UInt32? {
        guard let waiting = item?.waiting, waiting >= 4, !(item?.loading ?? true) else { return nil }
        return waiting
    }

    // MARK: Connecting

    /// Connect to the Chromecast row. A `request` from the player moves there at its second, and
    /// the player here closes into the mini; one from elsewhere plays there once it is up.
    public func connect(_ row: FfiCastRow, play request: TVPlayRequest? = nil) {
        guard let link else { return }
        problem = nil
        // The card of a cast that ended is put away: it would read Starting on the new
        // connection, with nothing asked of it.
        if active, stage == .stopped || stage == .failed { end() }
        let from = request.map { FfiCastFrom(id: $0.id, episode: $0.episode, position: $0.from ?? 0) }
        asked = true
        do {
            try link.connect(key: row.key, from: from)
        } catch {
            asked = false
            problem = TVRemoteModel.words(error)
            return
        }
        tvName = row.name
        chooserShown = false
        // From the player, the player closes into the mini; from elsewhere the controller opens.
        if let request, !request.fromPlayer { expandOnPlay = true }
    }

    /// Stop casting: what plays on the TV stops, the receiver closes, and plays open here again.
    public func disconnect() {
        dialogShown = false
        expanded = false
        expandOnPlay = false
        rejoining = false
        if active { end() }
        if let link {
            link.disconnect()
            held = nil
            heldAt = .now
        } else {
            stop()
        }
        CastLastItem.save(nil)
        lastItem = nil
        lastPlayed = nil
    }

    /// A play the engine takes to the held TV: the controller opens as it starts.
    public func playing(_ request: TVPlayRequest) {
        guard connected else { return }
        expandOnPlay = true
        link?.played(request)
    }

    /// A control of the mini and the controller. The engine shows a pause or a seek at once.
    public func command(_ command: FfiTvCommand) {
        if let link, held?.state == .playing {
            link.command(command)
        } else {
            send(command)
        }
    }

    /// Play here: the TV stops and is let go, and the episode plays on here at its second.
    public func playHereFromSession() {
        guard let link else {
            playHere()
            return
        }
        expanded = false
        dialogShown = false
        // Once the TV dropped off, neither the engine nor the card may still name the episode:
        // then what it last played, where it was.
        let fallback = (active ? carried : nil) ?? lastPlayed
        if active { end() }
        let from = link.playHere()
        // The TV is let go now: the play below opens here, ahead of the session's next view.
        held = nil
        heldAt = .now
        CastLastItem.save(nil)
        lastItem = nil
        lastPlayed = nil
        if let from {
            playLocally?(from.id, from.episode, from.position > 0 ? from.position : nil)
        } else if let fallback {
            playLocally?(fallback.id, fallback.episode, fallback.from)
        }
    }

    /// iPhone and iPad: the app leaves the screen, or is back.
    public func scene(active: Bool) {
        inBackground = !active
        settleAway()
    }

    /// iPhone and iPad: the cast's audio session runs, or stopped. While it runs, iOS keeps the
    /// app going in the background, so it goes on serving the TV and the TV plays on.
    public func audioHeld(_ running: Bool) {
        audioRunning = running
        settleAway()
    }

    /// iOS stops an app soon after it leaves the screen unless it plays audio, and with it the
    /// stream this device relays. Away with the cast's audio stopped (it could not start, or a
    /// call took it), the engine pauses the TV, which says it waits for this device, and plays on
    /// once the app is back or the audio runs again, joining again if the session dropped
    /// meanwhile. Away with the audio running, the TV plays on.
    private func settleAway() {
        let away = inBackground && !audioRunning
        guard away != sentAway else { return }
        sentAway = away
        link?.away(away)
    }
}

/// What the cast button draws.
public enum CastButtonLook: Equatable, Sendable {
    case hidden, available, connecting, connected
}

extension TVRemoteModel {
    /// The receiver's word for this device, for its own lines: Waiting for the iPhone.
    static var deviceWord: String {
        #if os(macOS)
        "mac" // i18n-skip
        #else
        UIDevice.current.userInterfaceIdiom == .pad ? "ipad" : "iphone" // i18n-skip
        #endif
    }

    /// The cast button in the top bars: the session's, or a Kaeri TV's link, or available while
    /// either kind of TV is found. Hidden while there is none.
    public var castButton: CastButtonLook {
        switch cast.button {
        case .connecting: return .connecting
        case .connected: return .connected
        case .available, .hidden: break
        }
        if cast.connected { return cast.held?.state == .connecting ? .connecting : .connected }
        if connected != nil { return card?.phase == .connected ? .connected : .connecting }
        if cast.button == .available || !cast.rows.isEmpty { return .available }
        if rows.contains(where: { $0.kind == .paired || $0.kind == .new }) { return .available }
        return .hidden
    }

    /// The app is in the foreground, or left it: TVs are looked for the whole time it is, so the
    /// button knows whether there is one and a session left running is joined again.
    public func foreground(_ on: Bool) {
        inForeground = on
        #if os(iOS)
        cast.scene(active: on)
        #endif
        lookInForeground()
    }

    /// Browse while in front, once the engine's sender is there.
    func lookInForeground() {
        let on = inForeground && sender != nil
        guard on != lookingInForeground else { return }
        lookingInForeground = on
        if on { startBrowsing() } else { stopBrowsing() }
    }

    /// The chooser from the cast button: Chromecast and Kaeri TVs, connecting to one; or with a
    /// TV connected, the dialog that says so.
    public func castButtonTapped() {
        if cast.connected {
            cast.dialogShown = true
        } else if connected != nil {
            openRemote()
        } else {
            cast.chooserShown = true
        }
    }

    /// A Kaeri TV picked in the chooser: a paired one connects, a new one pairs in the sheet.
    public func chooseFromChooser(_ row: FfiTvRow) {
        cast.chooserShown = false
        if cast.connected { cast.disconnect() }
        request = nil
        switch row.kind {
        case .paired, .pairedAway:
            choose(row)
            sheet = .remote
        case .new:
            choose(row)
        case .needsUpdate:
            break
        }
    }
}

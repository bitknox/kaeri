// The Fixtures app's TVs with Chromecast built in: a made-up device list handed to the engine's
// Cast sink, as the network's browse would find it, and a scripted cast behind the card. Nothing
// reaches a real device: the list's addresses are loopback ones nothing listens on, and a pick
// opens the script, not the engine's cast.

#if FIXTURES
import KaeriKit
import KaeriStrings
import Foundation
import Synchronization

/// What the casting shots open on: `-KaeriCast <scene>`.
///
///     found      the picker with Bedroom (Kaeri) and two TVs with Chromecast built in
///     searching  the picker before anything answers, past its quiet 600 ms
///     none       the picker on a network where nothing answers, past its 10 s wait
///     gone       the found scene, then Kitchen display gone from the network
///     blocked    the picker while the system won't let the app look on the local network
///     starting   the card just after a pick, starting on Living room
///     counting   the same card six seconds on: Waiting for Living room · 6 s
///     preparing  the card six seconds on, this device preparing the stream
///     playing    Living room playing episode 3, its volume set on the TV
///     muted      the playing scene, muted
///     level      Kitchen display playing, with a volume level and the skip offer
///     upnext     Living room near the end: Up next, plays in 6 s, with Cancel and Play now
///     skipped    Kitchen display after an automatic skip: Skipped opening, Undo
///     refused    the card after Living room refused every stream
///     never      the card after Living room never read the stream
///     network    the card when this device isn't on the TV's network
///     codec      the card when the TV can't play the video
///     deeplink   the card when the episode opens in another app
///     lost       Living room dropped off mid-episode: Casting stopped
///     takenover  someone else took Living room: the card gone, the notice offering Play here
///     bar        the playing scene with its controller closed: the mini
///     button     Home with the cast button, a TV found and nothing connected
///     chooser    the cast button's chooser: Chromecast and Kaeri TVs
///     connecting the button's waves while Living room connects
///     dialog     Living room connected with nothing playing: the connected dialog
///     paused     the playing scene, paused on the TV
///     waiting    the playing scene with the TV waiting for the stream: 6 s
///     rejoin     the mini joining a session left running: Reconnecting to Living room
///     morph      the mini, opening into the controller and closing again, over and over
///     pause      the controller, paused and played again every 2 s, each with the TV's echoes
///
/// The cast states (16 November): a scene ending in `bar` has the controller closed, the mini
/// showing; the others have it open.
///
///     reconnecting, reconnectingbar   the connection to Living room dropped: Reconnecting
///     gaveup, gaveupbar               its tries ran out: Casting stopped, Lost Living room
///     fallback, fallbackopen          Living room plays this in its own player: the notice
///     messages                        Messages after a fallback, a stop on the TV and a loss
///     waitingtv, waitingbar           the TV waits two seconds, as while a stop is heard
///     outside, outsidebar             Living room stopped the cast on the TV
///     dialogplaying                   the connected dialog while something plays
///     idle                            the controller with Living room held and nothing playing
///
/// And timed for clips, each starting 4 s after the TV plays: `cliprejoin` (Reconnecting for
/// 3 s, then the TV answers further on), `clipgaveup` (Reconnecting for 4 s, then lost),
/// `clipfallback` (the fallback's notice), `clipoutside` (Waiting for Living room for 3 s, then
/// stopped on the TV), each with the controller open, and with `bar` after it, closed.
public enum CastScene: String, Sendable, CaseIterable {
    case found, searching, none, gone, blocked
    case starting, counting, preparing, playing, muted, level, upnext, skipped
    case refused, never, network, codec, deeplink
    case lost, takenover, bar
    case button, chooser, connecting, dialog, paused, waiting, rejoin, morph, pause
    case reconnecting, reconnectingbar, gaveup, gaveupbar, fallback, fallbackopen, messages
    case waitingtv, waitingbar, outside, outsidebar, dialogplaying, idle
    case cliprejoin, cliprejoinbar, clipgaveup, clipgaveupbar, clipfallback, clipfallbackopen
    case clipoutside, clipoutsidebar

    public static let argument = "-KaeriCast"

    public static func named(in arguments: [String] = ProcessInfo.processInfo.arguments) -> CastScene? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1)
        else { return nil }
        return CastScene(rawValue: arguments[index + 1])
    }

    /// The picker's scenes, where the browse finds nothing.
    var silent: Bool { self == .searching || self == .none || self == .blocked }

    var picker: Bool { [.found, .searching, .none, .gone, .blocked].contains(self) }

    /// The controller stays closed: the mini shows.
    var mini: Bool {
        [.bar, .morph, .reconnectingbar, .gaveupbar, .fallback, .messages, .waitingbar, .outsidebar,
         .dialogplaying, .cliprejoinbar, .clipgaveupbar, .clipfallback, .clipoutsidebar].contains(self)
    }
}

/// The made-up device list, found as the network's browse finds TVs.
final class FixtureCastBrowser: FfiCastBrowser, @unchecked Sendable {
    struct Device {
        let instance: String
        let id: String
        let name: String
        let address: String
    }

    static let devices = [
        Device(instance: "Fixture-living", id: "living", name: "Living room", address: "127.0.0.2"), // i18n-skip
        Device(instance: "Fixture-kitchen", id: "kitchen", name: "Kitchen display", address: "127.0.0.3"), // i18n-skip
    ]

    let silent: Bool
    private let sink = Mutex<FfiCastSink?>(nil)

    init(silent: Bool) {
        self.silent = silent
    }

    func start(sink: FfiCastSink) {
        self.sink.withLock { $0 = sink }
        guard !silent else { return }
        for device in Self.devices {
            let txt = [
                FfiTxtPair(key: "id", value: device.id), // i18n-skip
                FfiTxtPair(key: "fn", value: device.name), // i18n-skip
                FfiTxtPair(key: "md", value: "Fixture screen"), // i18n-skip
                FfiTxtPair(key: "ca", value: "465413"), // i18n-skip
            ]
            sink.found(instance: device.instance, addresses: [device.address], port: 8009, txt: txt)
        }
    }

    func stop() {
        sink.withLock { $0 = nil }
    }

    /// A device leaves the network.
    func lose(_ instance: String) {
        sink.withLock { $0 }?.lost(instance: instance)
    }
}

/// A cast that plays by script: its card, moved on as the scene says and as the card's controls
/// ask, the way a TV's reports would.
final class FixtureCastSession: FfiCastSessionProtocol, @unchecked Sendable {
    struct State {
        var name: String
        var phase: FfiLinkPhase = .connecting
        var phaseLine: Msg?
        var upNext: FfiCastUpNext?
        var undo: FfiCastUndo?
        var loading = true
        /// The TV waits for the stream mid-episode, in whole seconds since it began.
        var waiting: UInt32?
        var position: Double = 0
        var duration: Double?
        var paused = false
        var volume: Double?
        var volumeControl: FfiVolumeControl = .fixed
        var muted = false
        var audio: [FfiTvTrack] = []
        var subtitles: [FfiTvTrack] = []
        var subtitlesOff = false
        var skip: FfiSkipCard?
        var notice: Msg?
        /// The card's note: the TV plays this cast in its own player, as with Play on TV.
        var note: Msg?
        var over = false
        /// The connection to the TV closed and is being made again.
        var reconnecting = false
        var rev: UInt64 = 0
        /// The revision the last card handed out was made at.
        var handed: UInt64 = 0
        var stopped = false
        /// The playhead moves on with the clock between reports, as the session's scenes need;
        /// the card's own tests keep it where they put it.
        var moves = false
        /// When the position was last moved on.
        var since = Date.now
    }

    private let state: Mutex<State>
    let request: TVPlayRequest
    private let episodeLine: String

    /// The card's state now, for the session's view.
    var snapshot: State { state.withLock { $0 } }

    init(name: String, request: TVPlayRequest, episodeLine: String) {
        state = Mutex(State(name: name, position: request.from ?? 0))
        self.request = request
        self.episodeLine = episodeLine
    }

    /// Change the card, as a report from the TV would.
    func change(_ edit: (inout State) -> Void) {
        state.withLock { state in
            // The playhead moves on while it plays, as the TV's next report would say.
            if state.moves, !state.paused, !state.loading, state.waiting == nil {
                state.position += Date.now.timeIntervalSince(state.since)
            }
            state.since = .now
            edit(&state)
            state.rev += 1
        }
    }

    func card() -> FfiCastCard {
        let state = state.withLock { state in
            state.handed = state.rev
            return state
        }
        let now = FfiNowCard(
            id: request.id, title: request.title, episode: request.episode, episodeLine: episodeLine,
            loading: state.loading, position: state.position, duration: state.loading ? nil : state.duration,
            paused: state.paused, speed: 1, volume: state.volume, audio: state.audio, subtitles: state.subtitles,
            subtitlesOff: state.subtitlesOff, skip: state.skip)
        let card = FfiRemoteCard(
            tvName: state.name, phase: state.phase, phaseLine: nil, screen: .player, now: now,
            rev: state.rev, over: state.over, reached: true)
        return FfiCastCard(
            card: card, volumeControl: state.volumeControl, muted: state.muted, notice: state.notice,
            phaseLine: state.phaseLine, upNext: state.upNext, undo: state.undo, waiting: state.waiting,
            reconnecting: state.reconnecting && !state.over, note: state.note,
            canMute: state.volumeControl == .level)
    }

    func nextCard() async -> FfiCastCard? {
        while !Task.isCancelled {
            let (rev, handed, stopped) = state.withLock { ($0.rev, $0.handed, $0.stopped) }
            if stopped { return nil }
            if rev != handed { return card() }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return nil
    }

    func command(command: FfiTvCommand) {
        change { state in
            switch command {
            case .togglePause: state.paused.toggle()
            case .seek(let by): state.position = max(0, state.position + by)
            case .seekTo(let position): state.position = position
            case .mute(let muted) where state.volumeControl == .level: state.muted = muted
            case .volume(let step): state.volume = state.volume.map { min(100, max(0, $0 + step)) }
            case .track(let kind, let id):
                if kind == .subtitle {
                    state.subtitlesOff = id == nil
                    state.subtitles = state.subtitles.map { FfiTvTrack(id: $0.id, language: $0.language, title: $0.title, selected: $0.id == id) }
                } else {
                    state.audio = state.audio.map { FfiTvTrack(id: $0.id, language: $0.language, title: $0.title, selected: $0.id == id) }
                }
            case .skip:
                if let skip = state.skip { state.position = skip.to }
                state.skip = nil
            default: break
            }
        }
    }

    func playNext() { change { $0.upNext = nil } }

    func cancelNext() { change { $0.upNext = nil } }

    func undoSkip() {
        change { state in
            if let undo = state.undo { state.position = undo.to }
            state.undo = nil
        }
    }

    func tryAgain() -> Bool { false }

    func deviceName() -> String { state.withLock { $0.name } }

    func playHere() -> Double? {
        let position = state.withLock { $0.position }
        stop()
        return position
    }

    func stop() {
        change { $0.stopped = true }
    }
}

/// A cast session by script: connecting takes a moment, a play starts the scripted card, and
/// the view follows the card as the engine's would. Nothing reaches a real device.
final class FixtureCastLink: CastSessionLink, @unchecked Sendable {
    struct Held {
        var state: FfiCastSessionState = .idle
        var device: FixtureCastBrowser.Device?
        var session: FixtureCastSession?
        var notice: Msg?
        /// A note while the TV is held: the TV plays this cast in its own player.
        var note: Msg?
        var rev: UInt64 = 0
    }

    private let held = Mutex(Held())
    /// The browse finds the made-up TVs: the button shows.
    let found: Bool
    /// The pause scene's echoes: each pause is followed by the statuses a real TV sends, one
    /// without the media's length among them.
    let echoes: Bool

    init(found: Bool, echoes: Bool = false) {
        self.found = found
        self.echoes = echoes
    }

    private func change(_ edit: (inout Held) -> Void) {
        held.withLock { held in
            edit(&held)
            held.rev += 1
        }
    }

    var session: FixtureCastSession? { held.withLock { $0.session } }

    func state() -> FfiCastSessionView {
        let held = held.withLock { $0 }
        let card = held.session?.snapshot
        let request = held.session?.request
        var item: FfiCastItem?
        if held.state == .playing, let card, let request {
            item = FfiCastItem(
                id: request.id, title: request.title, episode: request.episode,
                episodeTitle: request.episode == "7" ? "The Lantern Road" : nil, image: nil, // i18n-skip
                position: card.position, duration: card.duration, paused: card.paused,
                waiting: card.waiting, loading: card.loading)
        }
        let button: FfiCastButton = switch held.state {
        case .connecting: .connecting
        case .connected, .playing: .connected
        case .idle, .error: found ? .available : .hidden
        }
        return FfiCastSessionView(
            state: held.state, button: button, rev: held.rev + (card?.rev ?? 0),
            deviceKey: held.device.map { "cast:\($0.id)" }, deviceName: held.device?.name, // i18n-skip
            item: item, queue: [], notice: held.notice, note: held.note,
            level: card?.volume.map { $0 / 100 }, muted: card?.muted,
            volumeControl: card?.volumeControl ?? .fixed, ownReceiver: held.note == nil,
            reconnecting: held.state == .playing && (card?.reconnecting ?? false),
            canMute: card?.volumeControl == .level)
    }

    /// The TV plays this cast in its own player, as the engine's note says.
    func note(_ note: Msg?) {
        change { $0.note = note }
    }

    /// The cast ends without being asked to: the TV dropped off or was stopped on the TV. The
    /// session reads as the engine's does, failed with its notice, and so does the card.
    func end(_ notice: Msg) {
        held.withLock { $0.session }?.change {
            $0.notice = notice
            $0.over = true
            $0.reconnecting = false
            $0.waiting = nil
        }
        change {
            $0.state = .error
            $0.notice = notice
        }
    }

    private var seen: (UInt64, UInt64)?

    func next() async -> FfiCastSessionView? {
        while !Task.isCancelled {
            let now = held.withLock { ($0.rev, $0.session?.snapshot.rev ?? 0) }
            if seen.map({ $0 != now }) ?? true {
                seen = now
                return state()
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return nil
    }

    /// Connect at once to `device`, as a scene sets up.
    func hold(_ device: FixtureCastBrowser.Device, state: FfiCastSessionState = .connected) {
        change {
            $0.device = device
            $0.state = state
        }
    }

    func connect(key: String, from: FfiCastFrom?) throws {
        let device = FixtureCastBrowser.devices.first { key.contains($0.id) } ?? FixtureCastBrowser.devices[0]
        hold(device, state: .connecting)
        Task {
            try? await Task.sleep(for: .milliseconds(900))
            change { $0.state = .connected }
            if let from {
                played(TVPlayRequest(id: from.id, title: "", episode: from.episode, from: from.position))
            }
        }
    }

    func disconnect() {
        held.withLock { $0.session }?.stop()
        change {
            $0.state = .idle
            $0.session = nil
            $0.device = nil
        }
    }

    func playHere() -> FfiCastFrom? {
        let session = held.withLock { $0.session }
        let position = session?.snapshot.position ?? 0
        disconnect()
        guard let request = session?.request else { return nil }
        return FfiCastFrom(id: request.id, episode: request.episode, position: position)
    }

    func command(_ command: FfiTvCommand) {
        guard let session = held.withLock({ $0.session }) else { return }
        session.command(command: command)
        guard echoes, case .togglePause = command else { return }
        // What a real TV sends after a pause: a buffering status, one without the media object,
        // then the full one. The engine keeps the length through them.
        Task {
            try? await Task.sleep(for: .milliseconds(120))
            session.change { $0.waiting = 0 }
            try? await Task.sleep(for: .milliseconds(180))
            session.change { $0.waiting = nil }
        }
    }

    func card() -> (any FfiCastSessionProtocol)? { held.withLock { $0.session } }

    func away(_ away: Bool) {}

    /// Start `request` on the TV: the scripted card, starting, then playing.
    @discardableResult
    func start(_ request: TVPlayRequest) -> FixtureCastSession? {
        guard let device = held.withLock({ $0.device }) else { return nil }
        held.withLock { $0.session }?.stop()
        let session = FixtureCastSession(
            name: device.name, request: request,
            episodeLine: L.episodeNumberString(number: request.episode))
        change {
            $0.session = session
            $0.state = .playing
        }
        session.change {
            $0.moves = true
            $0.phaseLine = .castPhaseStarting(name: device.name)
        }
        return session
    }

    func played(_ request: TVPlayRequest) {
        let session = start(request)
        Task {
            try? await Task.sleep(for: .milliseconds(1200))
            session?.change(FixtureCastLink.playing)
        }
    }

    /// The card once the TV plays: its length, tracks and volume.
    static func playing(_ state: inout FixtureCastSession.State) {
        state.phase = .connected
        state.phaseLine = nil
        state.loading = false
        state.duration = 1420
        state.position = max(state.position, 754)
        state.subtitles = [
            FfiTvTrack(id: 1, language: "en", title: nil, selected: true), // i18n-skip
            FfiTvTrack(id: 2, language: "en", title: "Signs only", selected: false), // i18n-skip
        ]
        state.audio = [
            FfiTvTrack(id: 1, language: "ja", title: nil, selected: true), // i18n-skip
            FfiTvTrack(id: 2, language: "en", title: nil, selected: false), // i18n-skip
        ]
    }
}

extension TVRemoteModel {
    /// The scene's device list, for the engine's sender to browse.
    static func fixtureCasts() -> FixtureCastBrowser {
        FixtureCastBrowser(silent: CastScene.named()?.silent ?? false)
    }

    /// The scripted session, held in place of the engine's.
    static func fixtureLink() -> FixtureCastLink {
        let scene = CastScene.named()
        return FixtureCastLink(found: !(scene?.silent ?? false), echoes: scene == .pause)
    }

    /// Set up the casting scene the arguments name, for a shot: `play` is what the card casts.
    func stage(_ scene: CastScene, casts: FixtureCastBrowser, link: FixtureCastLink, play: TVPlayRequest) async {
        if scene == .blocked { cast.localNetwork(denied: true) }
        if scene.picker {
            offer(play)
            guard scene == .gone else { return }
            while cast.rows.count < 2 { try? await Task.sleep(for: .milliseconds(50)) }
            try? await Task.sleep(for: .milliseconds(900))
            casts.lose(FixtureCastBrowser.devices[1].instance)
            return
        }
        let kitchen = scene == .level || scene == .skipped
        let device = FixtureCastBrowser.devices[kitchen ? 1 : 0]
        switch scene {
        case .button:
            return
        case .chooser:
            try? await Task.sleep(for: .milliseconds(600))
            cast.chooserShown = true
            return
        case .connecting:
            link.hold(device, state: .connecting)
            cast.asked = true
            return
        case .dialog:
            link.hold(device)
            try? await Task.sleep(for: .milliseconds(300))
            cast.dialogShown = true
            return
        case .rejoin:
            CastLastItem.save(CastLastItem(
                id: play.id, title: play.title, episode: play.episode, episodeTitle: nil, deviceName: device.name))
            cast.lastItem = CastLastItem.load()
            link.hold(device, state: .connecting)
            return
        default:
            break
        }
        if scene == .idle {
            link.hold(device)
            try? await Task.sleep(for: .milliseconds(300))
            cast.expanded = true
            return
        }
        link.hold(device)
        cast.expandOnPlay = !scene.mini
        guard let session = link.start(play) else { return }
        guard scene != .starting else { return }
        try? await Task.sleep(for: .milliseconds(400))
        switch scene {
        case .counting:
            session.change { $0.phaseLine = .castPhaseWaiting(name: device.name, seconds: 6) }
            return
        case .preparing:
            session.change { $0.phaseLine = .castPhasePreparing(seconds: 6) }
            return
        case .refused, .never, .network, .codec, .deeplink:
            let notice: Msg = switch scene {
            case .refused: .noticeCastRefused(name: device.name)
            case .never: .noticeCastNeverFetches(name: device.name)
            case .network: .noticeCastNotSameNetwork(name: device.name)
            case .codec: .noticeCastVideoCodec(name: device.name, codec: "AV1") // i18n-skip
            default: .noticeCastDeepLink
            }
            session.change {
                $0.notice = notice
                $0.over = true
            }
            return
        default:
            break
        }
        session.change { state in
            FixtureCastLink.playing(&state)
            state.position = kitchen ? 58 : 754
            state.subtitlesOff = kitchen
            if kitchen {
                state.volumeControl = .level
                state.volume = 45
                state.skip = FfiSkipCard(to: 88, label: L.playerSkipOpening)
            }
            state.muted = scene == .muted
            state.paused = scene == .paused
            if scene == .waiting { state.waiting = 6 }
            if scene == .waitingtv || scene == .waitingbar { state.waiting = 2 }
            if scene == .upnext {
                state.position = 1382
                state.upNext = FfiCastUpNext(
                    number: "8", title: "Fog Bell", still: nil, auto: true, secondsLeft: 6) // i18n-skip
            }
            if scene == .skipped {
                state.position = 92
                state.undo = FfiCastUndo(kind: .opening, to: 0)
            }
        }
        switch scene {
        case .lost:
            try? await Task.sleep(for: .milliseconds(600))
            session.change {
                $0.phase = .retrying
                $0.notice = .noticeCastLost(name: device.name)
                $0.over = true
            }
        case .takenover:
            try? await Task.sleep(for: .milliseconds(600))
            cast.expanded = false
            session.change {
                $0.notice = .noticeCastTakenOver(name: device.name)
                $0.over = true
            }
        case .reconnecting, .reconnectingbar:
            try? await Task.sleep(for: .milliseconds(400))
            session.change { $0.reconnecting = true }
        case .gaveup, .gaveupbar:
            try? await Task.sleep(for: .milliseconds(400))
            link.end(.noticeCastLost(name: device.name))
        case .outside, .outsidebar:
            try? await Task.sleep(for: .milliseconds(400))
            link.end(.noticeCastStoppedElsewhere(name: device.name))
        case .fallback, .fallbackopen:
            try? await Task.sleep(for: .milliseconds(400))
            link.note(.noticeCastFallback(name: device.name))
        case .messages:
            // What Messages keeps of casts: a loss, a stop on the TV, then a fallback.
            for notice in [Msg.noticeCastLost(name: device.name), .noticeCastStoppedElsewhere(name: device.name)] {
                cast.say?(Notice(kind: .info, text: notice.text, hint: nil, msg: notice))
            }
            try? await Task.sleep(for: .milliseconds(400))
            link.note(.noticeCastFallback(name: device.name))
        case .dialogplaying:
            try? await Task.sleep(for: .milliseconds(600))
            cast.dialogShown = true
        case .cliprejoin, .cliprejoinbar:
            try? await Task.sleep(for: .seconds(4))
            session.change { $0.reconnecting = true }
            try? await Task.sleep(for: .seconds(3))
            // The TV played on meanwhile: it answers further on than it was left.
            session.change {
                $0.reconnecting = false
                $0.position += 4
            }
        case .clipgaveup, .clipgaveupbar:
            try? await Task.sleep(for: .seconds(4))
            session.change { $0.reconnecting = true }
            // The engine's 20 s, shortened for the clip.
            try? await Task.sleep(for: .seconds(4))
            link.end(.noticeCastLost(name: device.name))
        case .clipfallback, .clipfallbackopen:
            try? await Task.sleep(for: .seconds(4))
            link.note(.noticeCastFallback(name: device.name))
        case .clipoutside, .clipoutsidebar:
            try? await Task.sleep(for: .seconds(4))
            // The receiver left the TV: the engine hears why for up to 4 s, the TV's card
            // waiting with its clock still, then says it was stopped there.
            for second in 0..<3 {
                session.change { $0.waiting = UInt32(second) }
                try? await Task.sleep(for: .seconds(1))
            }
            link.end(.noticeCastStoppedElsewhere(name: device.name))
        case .morph:
            // Open and close, with time to look at each end.
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                cast.expanded = true
                try? await Task.sleep(for: .seconds(4))
                cast.expanded = false
            }
        case .pause:
            while !Task.isCancelled {
                try? await Task.sleep(for: .seconds(2))
                cast.command(.togglePause)
            }
        default:
            break
        }
    }
}

extension CastModel {
    /// Wait, up to fifteen seconds, for `scene` to be on screen: true once it is.
    public func reached(_ scene: CastScene, tv: TVRemoteModel) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(15)
        while ContinuousClock.now < deadline {
            let there = switch scene {
            case .found: rows.count == 2 && !tv.rows.isEmpty
            case .searching: tv.sheet == .picker && rows.isEmpty
            case .none: tv.sheet == .picker && rows.isEmpty
            case .gone: rows.contains(where: \.gone)
            case .blocked: tv.sheet == .picker && localNetworkDenied
            case .starting, .counting, .preparing: stage == .starting && expanded
            case .playing, .muted, .level, .upnext, .skipped, .paused, .waiting, .pause,
                 .cliprejoin, .clipgaveup, .clipfallbackopen, .clipoutside:
                stage == .playing && expanded && (scene != .muted || muted)
            case .cliprejoinbar, .clipgaveupbar, .clipfallback, .clipoutsidebar:
                stage == .playing && miniShown
            case .reconnecting: stage == .playing && expanded && reconnecting
            case .reconnectingbar: miniShown && reconnecting
            case .waitingtv: stage == .playing && expanded && waitingShown
            case .waitingbar: miniShown && waitingShown
            case .fallbackopen: expanded && held?.note != nil
            case .fallback, .messages: miniShown && held?.note != nil
            case .gaveup, .outside: stage == .stopped && expanded
            case .gaveupbar, .outsidebar: !miniShown && held?.state == .error
            case .dialogplaying: dialogShown && stage == .playing
            case .idle: heldIdle && expanded
            case .refused, .never, .network, .codec, .deeplink: stage == .failed
            case .lost: stage == .stopped
            case .takenover: !active && offered != nil
            case .bar, .morph: stage == .playing && miniShown
            case .button: button == .available
            case .chooser: chooserShown && rows.count == 2
            case .connecting: button == .connecting
            case .dialog: dialogShown
            case .rejoin: rejoining && miniShown
            }
            if there {
                // The sheet's own animation in; for the searching picker its quiet start, for
                // the empty one its wait, and for the counting card its seconds.
                let settle: Duration = switch scene {
                case .searching: .seconds(1.5)
                case .none: .seconds(11)
                default: .milliseconds(800)
                }
                try? await Task.sleep(for: settle)
                return true
            }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return false
    }
}
#endif

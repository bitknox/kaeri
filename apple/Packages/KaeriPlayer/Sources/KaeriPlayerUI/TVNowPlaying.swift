// Now Playing and the remote commands for the TV this device casts to or drives: while the TV
// plays something and the player here is down, the system's Now Playing shows the TV's episode
// with its art, and play, pause, the playhead and skipping go to the TV, as the controller's
// controls do. It goes as the cast ends or is lost.
//
// Mac: Control Center and the media keys, for a cast and for a Kaeri TV this Mac drives.
// iPhone and iPad: the Lock Screen and Control Center, for a cast. iOS shows Now Playing only for
// an app whose audio session plays, and stops an app soon after it leaves the screen unless it
// plays; this device serves what the TV plays, so while a cast runs the app holds an audio
// session that plays silence (`CastAudioHold`). The TV plays on with the phone locked or the app
// in the background, and nothing is heard here.

import KaeriKit
import KaeriStore
import KaeriStrings
import CoreGraphics
import Foundation
import MediaPlayer
import Observation
#if os(macOS)
import AppKit
#else
import AVFAudio
import UIKit
#endif

/// Where Now Playing's art comes from: the episode's still, else the show's banner, by URL, and
/// the app's artwork store that fetches and decodes it.
public struct NowPlayingArt {
    /// The URL for a show and the episode's still, when there is one.
    let url: @MainActor (AnilistId?, String?) -> String?
    /// The art for a URL once it is here. Read under observation, so its arrival updates Now
    /// Playing.
    let image: @MainActor (String) -> CGImage?
    /// Fetch the art for a URL and hold it until cancelled.
    let fetch: @MainActor (String) async -> Void

    public init(
        url: @escaping @MainActor (AnilistId?, String?) -> String?,
        image: @escaping @MainActor (String) -> CGImage?,
        fetch: @escaping @MainActor (String) async -> Void
    ) {
        self.url = url
        self.image = image
        self.fetch = fetch
    }
}

@MainActor
public final class TVNowPlaying {
    private let tv: TVRemoteModel
    private weak var player: PlayerModel?
    /// Whether nothing of a show may show outside the app: private mode, or always private.
    private let holds: @MainActor (AnilistId) -> Bool
    private let art: NowPlayingArt?
    private var targets: [(MPRemoteCommand, Any)] = []
    /// The art Now Playing shows, and the fetch that holds it.
    private var artURL: String?
    private var artFetch: Task<Void, Never>?
    private var artwork: MPMediaItemArtwork?
    #if os(iOS)
    private let audio = CastAudioHold()
    #endif

    public init(
        tv: TVRemoteModel, player: PlayerModel, art: NowPlayingArt? = nil,
        holds: @escaping @MainActor (AnilistId) -> Bool = { _ in false }
    ) {
        self.tv = tv
        self.player = player
        self.art = art
        self.holds = holds
        #if os(iOS)
        audio.changed = { [weak tv] running in tv?.cast.audioHeld(running) }
        #endif
        watch()
    }

    /// Follows the cast, the card and the player, here rather than in a view: the commands work
    /// with no window up and the screen locked.
    private func watch() {
        withObservationTracking {
            update()
        } onChange: { [weak self] in
            Task { @MainActor in self?.watch() }
        }
    }

    /// What Now Playing shows: its fields, whether the TV's playhead runs, and the art's URL.
    struct Shown {
        var info: [String: Any]
        var playing: Bool
        var art: String?
    }

    private func shown() -> Shown? {
        let cast = tv.cast
        // A cast session: the episode on the TV, "Casting to" it.
        if cast.link != nil, cast.miniShown, let item = cast.item {
            let hidden = item.id.map(holds) ?? false
            return Shown(
                info: Self.info(item, line: cast.episodeLine, tvName: cast.heldName, playhead: cast.itemPosition(), private: hidden),
                playing: cast.itemLive && !item.paused && item.waiting == nil,
                art: hidden ? nil : art?.url(item.id, item.image))
        }
        // A cast followed by its card, as a driven TV is: the card's routing sends the commands
        // to it. On the Mac, a Kaeri TV driven too.
        #if os(macOS)
        let tvName = cast.active ? cast.tvName : tv.connected != nil ? tv.card?.tvName : nil
        #else
        let tvName = cast.active ? cast.tvName : nil
        #endif
        guard let tvName, let now = tv.now else { return nil }
        let hidden = holds(now.id)
        return Shown(
            info: Self.info(now, tvName: tvName, playhead: tv.playhead() ?? now.position, live: tv.live, private: hidden),
            playing: Self.playing(now, live: tv.live),
            art: hidden ? nil : art?.url(now.id, nil))
    }

    private func update() {
        // The player here has Now Playing and the commands while it is up.
        let playerUp = player?.isPresented ?? false
        guard !playerUp, let shown = shown() else {
            release(playerUp: playerUp)
            return
        }
        if targets.isEmpty { bind() }
        #if os(iOS)
        audio.hold(true)
        #endif
        var info = shown.info
        if let artwork = artwork(for: shown.art) { info[MPMediaItemPropertyArtwork] = artwork }
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = info
        #if os(macOS)
        center.playbackState = shown.playing ? .playing : .paused
        #endif
    }

    /// The art for `url`, fetched while it is shown and made once it is here.
    private func artwork(for url: String?) -> MPMediaItemArtwork? {
        guard let url, let art else {
            dropArt()
            return nil
        }
        if url != artURL {
            dropArt()
            artURL = url
            artFetch = Task { await art.fetch(url) }
        }
        if let artwork { return artwork }
        guard let image = art.image(url) else { return nil }
        let made = Self.artwork(SendableImage(image: image))
        artwork = made
        return made
    }

    private func dropArt() {
        artFetch?.cancel()
        artFetch = nil
        artURL = nil
        artwork = nil
    }

    /// The system asks for the art off the main thread, so the handler is made here, outside
    /// the main actor.
    nonisolated static func artwork(_ art: SendableImage) -> MPMediaItemArtwork {
        let size = CGSize(width: art.image.width, height: art.image.height)
        return MPMediaItemArtwork(boundsSize: size) { @Sendable _ in
            #if os(macOS)
            NSImage(cgImage: art.image, size: size)
            #else
            UIImage(cgImage: art.image)
            #endif
        }
    }

    /// Whether the TV's playhead runs: it plays, and the link is up to say so.
    static func playing(_ now: FfiNowCard, live: Bool) -> Bool {
        live && !now.paused && !now.loading
    }

    /// What Now Playing shows of the TV's episode. The system runs the clock from the playhead
    /// at the rate given, so it is written as the card changes, never on a timer. While
    /// `private`, it names nothing: "Playing in Kaeri", with no show, episode or TV.
    static func info(_ now: FfiNowCard, tvName: String, playhead: Double, live: Bool, private: Bool = false) -> [String: Any] {
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: `private` ? L.nowPlayingPrivate : now.episodeLine,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.video.rawValue,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: playhead,
            MPNowPlayingInfoPropertyPlaybackRate: playing(now, live: live) ? now.speed : 0.0,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
        ]
        if !`private` {
            info[MPMediaItemPropertyArtist] = now.title
            info[MPMediaItemPropertyAlbumTitle] = L.tvStatusPlaying(name: tvName)
        }
        if let duration = now.duration { info[MPMediaItemPropertyPlaybackDuration] = duration }
        return info
    }

    /// What Now Playing shows of a cast session's episode: the show, the episode, and the line
    /// that says which TV plays it. While `private`, it names nothing.
    static func info(_ item: FfiCastItem, line: String, tvName: String, playhead: Double, private: Bool = false) -> [String: Any] {
        var info: [String: Any] = [
            MPMediaItemPropertyTitle: `private` ? L.nowPlayingPrivate : line.isEmpty ? item.title : line,
            MPNowPlayingInfoPropertyMediaType: MPNowPlayingInfoMediaType.video.rawValue,
            MPNowPlayingInfoPropertyElapsedPlaybackTime: playhead,
            MPNowPlayingInfoPropertyPlaybackRate: !item.loading && !item.paused && item.waiting == nil ? 1.0 : 0.0,
            MPNowPlayingInfoPropertyDefaultPlaybackRate: 1.0,
        ]
        if !`private` {
            info[MPMediaItemPropertyArtist] = item.title
            info[MPMediaItemPropertyAlbumTitle] = L.castNotificationLine(name: tvName)
        }
        if let duration = item.duration { info[MPMediaItemPropertyPlaybackDuration] = duration }
        return info
    }

    private func bind() {
        let center = MPRemoteCommandCenter.shared()
        func on(_ command: MPRemoteCommand, _ action: @escaping @MainActor (TVRemoteModel, Bool, MPRemoteCommandEvent) -> Void) {
            command.isEnabled = true
            let target = command.addTarget { [weak self] event in
                MainActor.assumeIsolated {
                    guard let tv = self?.tv else { return .noActionableNowPlayingItem }
                    // A cast session's episode, or the card's.
                    if tv.cast.link != nil, let item = tv.cast.item {
                        guard tv.cast.itemLive else { return .commandFailed }
                        action(tv, item.paused, event)
                        return .success
                    }
                    guard let now = tv.now else { return .noActionableNowPlayingItem }
                    guard tv.live else { return .commandFailed }
                    action(tv, now.paused, event)
                    return .success
                }
            }
            targets.append((command, target))
        }
        on(center.playCommand) { tv, paused, _ in if paused { tv.send(.togglePause) } }
        on(center.pauseCommand) { tv, paused, _ in if !paused { tv.send(.togglePause) } }
        on(center.togglePlayPauseCommand) { tv, _, _ in tv.send(.togglePause) }
        on(center.changePlaybackPositionCommand) { tv, _, event in
            guard let event = event as? MPChangePlaybackPositionCommandEvent else { return }
            tv.send(.seekTo(position: event.positionTime))
        }
        #if os(macOS)
        on(center.nextTrackCommand) { tv, _, _ in tv.send(.next) }
        on(center.previousTrackCommand) { tv, _, _ in tv.send(.previous) }
        #else
        // As the player here: back and forward 10 s, which the controller has too.
        center.skipForwardCommand.preferredIntervals = [10]
        center.skipBackwardCommand.preferredIntervals = [10]
        on(center.skipForwardCommand) { tv, _, _ in tv.send(.seek(by: 10)) }
        on(center.skipBackwardCommand) { tv, _, _ in tv.send(.seek(by: -10)) }
        #endif
    }

    /// Let Now Playing go. With the player here up, Now Playing, the commands and on iOS the
    /// audio session are its own: only this one's targets go, and the rest is left to it.
    private func release(playerUp: Bool) {
        dropArt()
        #if os(iOS)
        audio.hold(false, keepSession: playerUp)
        #endif
        guard !targets.isEmpty else { return }
        for (command, target) in targets {
            command.removeTarget(target)
            if !playerUp { command.isEnabled = false }
        }
        targets = []
        guard !playerUp else { return }
        let center = MPNowPlayingInfoCenter.default()
        center.nowPlayingInfo = nil
        #if os(macOS)
        center.playbackState = .stopped
        #endif
    }
}

/// A decoded image handed to the artwork handler, which the system calls off the main thread.
/// The image is immutable.
struct SendableImage: @unchecked Sendable {
    let image: CGImage
}

#if os(iOS)
/// The cast's audio session on iPhone and iPad: `.playback`, playing silence, while a cast runs
/// and the player here is down. iOS keeps an app that plays audio going after it leaves the
/// screen, and shows Now Playing for the app whose session plays; this app serves what the TV
/// plays, so it has to keep going. Like the player's, the session is not mixed with other audio:
/// that is what Now Playing asks for.
///
/// A call or another app's audio stops it. In the background the cast then tells the engine the
/// app is away, which pauses the TV until the app is back; it starts again when the interruption
/// ends or the app comes to the front.
@MainActor
final class CastAudioHold {
    /// Whether the silence plays, each time that changes.
    var changed: (@MainActor (Bool) -> Void)?
    private(set) var running = false
    private var wanted = false
    private var engine: AVAudioEngine?
    private var observers: [any NSObjectProtocol] = []

    func hold(_ on: Bool, keepSession: Bool = false) {
        guard on != wanted else { return }
        wanted = on
        if on {
            observe()
            start()
        } else {
            for observer in observers { NotificationCenter.default.removeObserver(observer) }
            observers = []
            stop(deactivate: !keepSession)
        }
    }

    private func start() {
        guard wanted, !running else { return }
        set(running: begin())
    }

    /// Start a new engine in place of one a route change or a reset stopped. Running stays true
    /// across it when the new one starts, so the TV is not paused for it.
    private func restart() {
        engine?.stop()
        engine = nil
        guard wanted else { return }
        set(running: begin())
    }

    private func begin() -> Bool {
        let session = AVAudioSession.sharedInstance()
        do {
            try session.setCategory(.playback, mode: .default)
            try session.setActive(true)
            let engine = engine ?? Self.silence()
            self.engine = engine
            if !engine.isRunning { try engine.start() }
            return true
        } catch {
            return false
        }
    }

    private func stop(deactivate: Bool) {
        engine?.stop()
        engine = nil
        set(running: false)
        // Other apps' audio may go on.
        if deactivate { try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation) }
    }

    private func set(running: Bool) {
        guard running != self.running else { return }
        self.running = running
        changed?(running)
    }

    private func observe() {
        guard observers.isEmpty else { return }
        let center = NotificationCenter.default
        observers = [
            center.addObserver(forName: AVAudioSession.interruptionNotification, object: nil, queue: .main) { [weak self] note in
                let raw = note.userInfo?[AVAudioSessionInterruptionTypeKey] as? UInt
                let began = raw.flatMap(AVAudioSession.InterruptionType.init(rawValue:)) == .began
                MainActor.assumeIsolated {
                    guard let self else { return }
                    if began {
                        self.engine?.stop()
                        self.set(running: false)
                    } else {
                        self.start()
                    }
                }
            },
            // A route change or a reset stops the engine: start again on a new one.
            center.addObserver(forName: .AVAudioEngineConfigurationChange, object: nil, queue: .main) { [weak self] note in
                let from = (note.object as AnyObject?).map(ObjectIdentifier.init)
                MainActor.assumeIsolated {
                    guard let self, let engine = self.engine, from == ObjectIdentifier(engine) else { return }
                    self.restart()
                }
            },
            center.addObserver(forName: AVAudioSession.mediaServicesWereResetNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.restart() }
            },
            // Back in front after another app took the audio: take it again.
            center.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
                MainActor.assumeIsolated { self?.start() }
            },
        ]
    }

    /// An engine that plays silence: one source node writing zeros into the main mixer. Made
    /// outside the main actor, as the source's block runs on the audio thread.
    private nonisolated static func silence() -> AVAudioEngine {
        let engine = AVAudioEngine()
        let format = AVAudioFormat(standardFormatWithSampleRate: 44_100, channels: 2)!
        let source = AVAudioSourceNode(format: format) { _, _, _, buffers in
            for buffer in UnsafeMutableAudioBufferListPointer(buffers) {
                if let data = buffer.mData { memset(data, 0, Int(buffer.mDataByteSize)) }
            }
            return noErr
        }
        engine.attach(source)
        engine.connect(source, to: engine.mainMixerNode, format: format)
        return engine
    }
}
#endif

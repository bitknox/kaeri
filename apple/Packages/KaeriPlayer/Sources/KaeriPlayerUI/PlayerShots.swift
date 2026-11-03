// The player's states on the Fixtures app's Playing scene, for screenshots set beside the
// boards (apple/scripts/shots.sh). Testing only: the release app has none of this.

#if FIXTURES
import KaeriKit
import KaeriStore
import Foundation

/// A state of the player a screenshot shows.
public enum PlayerShot: String, CaseIterable, Sendable {
    /// The banner under "Starting episode 7" (T2, M4).
    case starting
    /// The starting screen on the engine's made-up start: `KAERI_START_STATE` (or
    /// `-KaeriStartState`) names the state, `KAERI_START_AT` the seconds to hold it at,
    /// `KAERI_START_DETAILS=1` opens Details.
    case startFixture = "start-fixture"
    /// Playing, with the controls up (MacPlayer, PhonePlayer), round and round Part A.
    case playing
    /// Playing, with the controls left to fade 2 s after the last input, as they do: for the
    /// Mac's click test (apple/MacPlayerTests), which brings them back with the pointer. Round
    /// and round Part A, as `playing` is.
    case fading
    /// The opening's skip, with the controls hidden (PhonePlayerSkip).
    case skip
    /// Up Next counting down from the ending (MacPlayer, PhonePlayerUpNext).
    case upNext = "up-next"
    /// "Still watching?" (PhonePlayerStillWatching).
    case stillWatching = "still-watching"
    /// The stall prompt, counting up.
    case stall
    /// Buffering mid-episode: the amber line along the picture's top, the controls hidden.
    case buffering
    /// Scrubbing at half speed, with the frame there from the hidden mpv, the time and the
    /// chapter.
    case scrubbing
    /// Picture in Picture: the app browsing, with the system's floating window over it on
    /// iPhone and iPad; on the Mac the main window, and the frame the floating window shows, as
    /// the feed copied it.
    case pictureInPicture = "picture-in-picture"
    /// "Couldn't play episode 7", with Choose source and Close.
    case couldntPlay = "couldnt-play"
    /// Playing, paused, with the Subtitles and audio menu open over it and the last subtitle
    /// track picked, which a long list scrolls into view.
    case tracks
    /// Playing, paused, with the Speed menu open (on iPhone and iPad, Picture and sound).
    case speed
    /// Playing, paused, with the Picture and sound menu open: upscaling and night mode.
    case picture
    /// Playing, paused, with the Episodes menu open, the playing episode's sources under it.
    case episodes
    /// Stats for nerds over the episode playing, with the controls up: the engine's rows for
    /// the generated file, which is on the device, so no Network row.
    case stats
    /// The same with the controls gone: the panel stays.
    case statsHidden = "stats-hidden"
    /// Opened before the file: the dashes, and "Waiting for the stream".
    case statsWaiting = "stats-waiting"
    /// A stream playing well, from the fixture: the Network and Display rows.
    case statsStream = "stats-stream"
    /// A stream held waiting for data: the buffer in amber, with the buffering line.
    case statsBuffering = "stats-buffering"
    /// An HDR picture tone-mapped for an SDR display, from the fixture.
    case statsHdr = "stats-hdr"
    /// Paused, from the fixture: nothing in amber.
    case statsPaused = "stats-paused"
    /// Just after Copy stats: "Copied" in its place.
    case statsCopied = "stats-copied"

    /// Skip intro (skip-intro-and-invites board): the marking bar at each step and with each
    /// line, drawn from the engine's fixture views over the playing episode.
    case markStart = "mark-start"
    case markEnd = "mark-end"
    case markReview = "mark-review"
    case markShort = "mark-short"
    case markLong = "mark-long"
    case markReplaces = "mark-replaces"
    case markShares = "mark-shares"
    case markNoEntry = "mark-noentry"
    case markEditing = "mark-editing"
    case markPreviewing = "mark-previewing"
    /// "Was that the opening?", "Wrong ending times?" and the sharing question.
    case promptOpening = "prompt-opening"
    case promptWrong = "prompt-wrong"
    case promptShare = "prompt-share"
    /// More > Skip times, with one time from each origin.
    case skipTimes = "skip-times"

    /// Past 85% of the episode, the controls up with the discussion offered.
    case discussion
    /// The same, with the discussion opened beside the video.
    case discussionOpen = "discussion-open"

    /// The launch argument that names one: `-KaeriPlayerShot up-next`.
    public static let argument = "-KaeriPlayerShot"

    public static func named(in arguments: [String]) -> PlayerShot? {
        guard let index = arguments.firstIndex(of: argument), arguments.indices.contains(index + 1) else { return nil }
        return PlayerShot(rawValue: arguments[index + 1])
    }

    /// The launch argument that draws a menu's row as if the pointer were over it, by its title:
    /// `-KaeriPlayerPointer 1.5×`.
    public static let pointerArgument = "-KaeriPlayerPointer"

    static func pointed(in arguments: [String]) -> String? {
        guard let index = arguments.firstIndex(of: pointerArgument), arguments.indices.contains(index + 1) else { return nil }
        return arguments[index + 1]
    }

    /// The menu a shot opens.
    var menu: PlayerMenu? {
        switch self {
        case .tracks: .tracks
        case .speed: .speed
        case .picture: .picture
        case .episodes: .episodes
        default: nil
        }
    }
}

extension PlayerModel {
    /// A launch argument or environment variable of the starting screen's shot.
    private static func startParameter(_ name: String) -> String? {
        let arguments = ProcessInfo.processInfo.arguments
        if let index = arguments.firstIndex(of: "-KaeriStart" + name.capitalized), arguments.indices.contains(index + 1) {
            return arguments[index + 1]
        }
        return ProcessInfo.processInfo.environment["KAERI_START_" + name.uppercased()]
    }

    /// The made-up start named by the launch, its events said as the engine says them and held at
    /// the second asked for.
    func stageStartFixture(store: AppStore) async -> Bool {
        let states: [String: StartingFixture] = [
            "title-lookup": .titleLookup, "sources-loading": .sourcesLoading, "asking-first": .askingFirst,
            "asking-pinned": .askingPinned, "source-missed": .sourceMissed, "source-failed": .sourceFailed,
            "sources-resting": .sourcesResting, "slow": .slow, "finding-peers": .findingPeers,
            "peers-known": .peersKnown, "found": .found, "fetching-subtitles": .fetchingSubtitles,
            "another-stream": .anotherStream, "buffering-torrent": .bufferingTorrent,
            "buffering-direct": .bufferingDirect, "offline": .offline, "offline-timed-out": .offlineTimedOut,
            "failed-no-episode": .failedNoEpisode,
            "failed-no-match": .failedNoMatch, "failed-no-sources": .failedNoSources, "failed-not-aired": .failedNotAired,
        ]
        guard let state = Self.startParameter("state").flatMap({ states[$0] }) else { return false }
        let hold = Self.startParameter("at").flatMap(Double.init) ?? 2
        if Self.startParameter("details") == "1" { startDetailsOpen = true }
        let began = ContinuousClock.now
        for fixture in startingFixture(state: state) {
            let at = began + .milliseconds(Int(fixture.atMs))
            guard at <= began + .seconds(hold) else { break }
            if at > .now { try? await Task.sleep(until: at) }
            store.apply(fixture.event)
        }
        try? await Task.sleep(until: began + .seconds(hold))
        if Self.startParameter("choose") == "1" { chooseSource() }
        try? await Task.sleep(for: .milliseconds(400))
        return isPresented
    }

    /// Play the first Continue row, the Playing scene's episode 7, and bring the player to
    /// `shot`. False when it did not get there in time.
    public func stage(_ shot: PlayerShot, store: AppStore) async -> Bool {
        guard let item = store.continuing.queue(store.titles).all.first else { return false }
        if shot == .couldntPlay {
            // As the engine says it when no source answered: staged, then ended, failed.
            apply(.playbackStaged(id: item.id, title: item.entry.title, episode: String(item.episode), episodeTitle: nil, speed: 1))
            apply(.playbackEnded(watched: false, ending: .failed, rollingOn: false))
            try? await Task.sleep(for: .seconds(1.5))
            return failure != nil
        }
        if shot == .startFixture { return await stageStartFixture(store: store) }
        holdsCurtain = shot == .starting || shot == .statsWaiting
        store.send(.play(id: item.id, episode: String(item.episode)))
        guard await waited(30, { self.isPresented && self.hasPicture && self.duration != nil }) else { return false }
        var there = true
        switch shot {
        case .starting:
            // The banner's art.
            try? await Task.sleep(for: .seconds(1))
        case .couldntPlay, .startFixture:
            break
        case .playing:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            loopPartA()
            pinnedControls = true
        case .tracks, .speed, .picture, .episodes:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 && !self.subtitleTracks.isEmpty && self.menus != nil }
            togglePause()
            if shot == .tracks, let last = subtitleTracks.last { selectSubtitle(last.id) }
            pinnedControls = true
            pointedRow = PlayerShot.pointed(in: ProcessInfo.processInfo.arguments)
            try? await Task.sleep(for: .seconds(0.5))
            if let menu = shot.menu { requestMenu(menu) }
        case .fading:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            loopPartA()
        case .skip:
            seek(to: 7)
            there = await waited(10) { self.skip != nil && self.position >= 7.5 }
            hideControls()
        case .upNext:
            seek(to: 43.5)
            there = await waited(10) { self.upNextShown && self.countdown != nil && self.position >= 44 }
            // The Mac's board has the controls up with the card, the iPhone's has them hidden.
            #if os(macOS)
            pinnedControls = true
            #else
            hideControls()
            #endif
        case .markStart, .markEnd, .markReview, .markShort, .markLong, .markReplaces, .markShares, .markNoEntry,
            .markEditing, .markPreviewing, .promptOpening, .promptWrong, .promptShare, .skipTimes:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            togglePause()
            pinnedControls = true
            guard let playback else { break }
            switch shot {
            case .promptOpening: apply(.playerSkipPrompt(playback: playback, prompt: .wasOpening(start: 30, end: 120)))
            case .promptWrong: apply(.playerSkipPrompt(playback: playback, prompt: .wrongTimes(kind: .ending)))
            case .promptShare: apply(.playerSkipPrompt(playback: playback, prompt: .share))
            case .skipTimes:
                apply(.playerTimeline(playback: playback, marks: [
                    TimelineMark(kind: .recap, start: 0, end: 8, origin: .file),
                    TimelineMark(kind: .opening, start: 10, end: 28, origin: .yours),
                    TimelineMark(kind: .ending, start: 38, end: 46, origin: .aniSkip),
                ]))
                toggleSkipTimes()
            default:
                let state: SkipMarkFixture = switch shot {
                case .markStart: .start
                case .markEnd: .end
                case .markShort: .tooShort
                case .markLong: .tooLong
                case .markReplaces: .replaces
                case .markShares: .shares
                case .markNoEntry: .noEntry
                case .markEditing: .editing
                case .markPreviewing: .previewing
                default: .review
                }
                // The fixture's times (1:31 to 3:01) lie beyond the 46 s test clip: the same view
                // with its edges moved into the clip, so the seek line shows the start tick and
                // the stretch.
                var view = skipMarkFixture(state: state)
                if view.start != nil { view.start = 10 }
                if view.end != nil { view.end = view.check == .tooShort ? 13 : state == .tooLong ? 44 : 28 }
                view.length = (view.end ?? 0) - (view.start ?? 0)
                apply(.playerSkipMark(playback: playback, view: view))
            }
        case .discussion, .discussionOpen:
            seek(to: (duration ?? 46) * 0.86)
            there = await waited(10) { self.discussionOffer != nil && self.paused == false }
            pinnedControls = true
            if shot == .discussionOpen {
                try? await Task.sleep(for: .seconds(1))
                openDiscussion()
                try? await Task.sleep(for: .seconds(1.5))
            }
        case .stillWatching:
            seek(to: 33)
            there = await waited(10) { self.position >= 33.5 }
            askStillWatching()
            togglePause()
            there = await waited(10) { self.paused } && there
        case .stall:
            seek(to: 33)
            there = await waited(10) { self.position >= 33.5 }
            if let playback { apply(.playerUi(playback: playback, ui: .stall(seconds: 9))) }
            hideControls()
        case .buffering:
            seek(to: 33)
            there = await waited(10) { self.position >= 33.5 }
            bufferForShot()
            hideControls()
        case .scrubbing:
            seek(to: 24)
            there = await waited(10) { self.position >= 24.5 }
            // The finger held over the line, slowed to half speed: never let go.
            scrub(to: 9, pace: 0.5)
            there = await waited(10) { self.preview.ready } && there
        case .stats, .statsHidden:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            showStats(true)
            there = await waited(10) { self.statsWaitingSince == nil && self.statsPanel != nil } && there
            if shot == .stats { pinnedControls = true } else { hideControls() }
        case .statsWaiting:
            stageStats(playerStatsFixture(state: .waiting), waited: .seconds(5))
        case .statsStream, .statsBuffering, .statsHdr, .statsPaused, .statsCopied:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            let fixture: StatsFixture = switch shot {
            case .statsBuffering: .buffering
            case .statsHdr: .hdr
            case .statsPaused: .paused
            default: .playing
            }
            if shot == .statsPaused {
                togglePause()
                pinnedControls = true
            } else if shot == .statsBuffering {
                bufferForShot()
                hideControls()
            } else {
                hideControls()
            }
            stageStats(playerStatsFixture(state: fixture), copied: shot == .statsCopied)
        case .pictureInPicture:
            seek(to: 31)
            there = await waited(10) { self.position >= 31.5 }
            togglePictureInPicture()
            there = await waited(10) { self.pictureInPicturePhase == .active } && there
            // The floating window's first frames at its own size.
            try? await Task.sleep(for: .seconds(1.5))
        }
        // The fades.
        try? await Task.sleep(for: .milliseconds(700))
        return there
    }

    /// Out of Picture in Picture by the player's own toggle, 10 s after it is in it (within
    /// `seconds`), as the floating window's return button does: whether the picture came back
    /// into the player on the same playback within 10 s. For apple/scripts/pip-check.sh, which
    /// cannot press the window's buttons in the Simulator, and shoots the window meanwhile.
    public func returnFromPictureInPicture(within seconds: Int) async -> Bool {
        guard await waited(seconds, { self.pictureInPicturePhase == .active }) else { return false }
        try? await Task.sleep(for: .seconds(10))
        guard pictureInPicturePhase == .active else { return false }
        togglePictureInPicture()
        return await waited(10) { self.pictureInPicturePhase == .inline && self.inMainWindow && self.hasPicture }
    }

    /// Round and round Part A (20–42 s), from 22 s to 40 s: the episode is a minute long, so a
    /// click test that started at 31 s would otherwise meet Up Next at 42 s and the next episode
    /// ten seconds later, and a click made while that one starts does nothing.
    private func loopPartA() {
        loop(from: 22, to: 40)
    }

    private func waited(_ seconds: Int, _ condition: @MainActor () -> Bool) async -> Bool {
        let deadline = ContinuousClock.now + .seconds(seconds)
        while !condition() {
            guard ContinuousClock.now < deadline else { return false }
            try? await Task.sleep(for: .milliseconds(50))
        }
        return true
    }
}
#endif

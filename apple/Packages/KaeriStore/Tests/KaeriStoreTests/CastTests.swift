// Casting as the store holds it: the picker's Chromecast rows, including one that just left, and
// the card of a scripted cast through starting, playing, failing and ending. The engine's own
// cast is covered by its tests; here the session is the Fixtures app's script.

@testable import KaeriStore
import KaeriKit
import KaeriStrings
import Foundation
import Testing

@MainActor
@Suite(.timeLimit(.minutes(1)))
struct CastTests {
    let request = TVPlayRequest(id: 900_001, title: "Lanterns", episode: "3", from: 312)

    func row(_ id: String, _ name: String) -> FfiCastRow {
        FfiCastRow(key: "cast:\(id)", name: name, model: "", address: "127.0.0.9", routeHint: .throughThisDevice)
    }

    /// A model casting to a scripted session.
    func casting() -> (CastModel, FixtureCastSession) {
        let model = CastModel()
        let session = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        model.opener = { _, _ in session }
        model.cast("cast:living", name: "Living room", request: request)
        return (model, session)
    }

    func play(_ session: FixtureCastSession) {
        session.change { state in
            state.phase = .connected
            state.loading = false
            state.duration = 1420
            state.position = 492
        }
    }

    @Test func rowsComeByNameAndOneThatLeftSaysSoBeforeItGoes() async throws {
        let model = CastModel()
        model.browsing(true)
        model.refresh([row("b", "Living room"), row("a", "Kitchen display")])
        #expect(model.rows.map(\.row.name) == ["Kitchen display", "Living room"])

        model.refresh([row("b", "Living room")])
        #expect(model.rows.map(\.row.name) == ["Kitchen display", "Living room"])
        #expect(model.rows.first?.gone == true)
        #expect(model.rows.last?.gone == false)

        try await waitFor { model.rows.count == 1 }
        #expect(model.rows.map(\.row.name) == ["Living room"])
    }

    @Test func aTvBackInTimeTakesItsRowBack() async throws {
        let model = CastModel()
        model.browsing(true)
        model.refresh([row("a", "Kitchen display")])
        model.refresh([])
        model.refresh([row("a", "Kitchen display")])
        try await Task.sleep(for: .seconds(4.5))
        #expect(model.rows.map(\.gone) == [false])
    }

    @Test func stoppingTheBrowseOrHidingTheGroupClearsTheRows() {
        let model = CastModel()
        model.browsing(true)
        model.refresh([row("a", "Kitchen display")])
        model.browsing(false)
        #expect(model.rows.isEmpty)
        model.browsing(true)
        model.shown = false
        model.refresh([row("a", "Kitchen display")])
        #expect(model.rows.isEmpty)
    }

    @Test func theCardStartsAtOnceThenCountsThenPlays() async throws {
        let (model, session) = casting()
        #expect(model.active)
        #expect(model.stage == .starting)
        #expect(model.now?.title == "Lanterns")
        #expect(model.line == L.tvStartingOn(name: "Living room"))

        session.change { $0.phaseLine = .castPhasePreparing(seconds: 5) }
        try await waitFor { model.card?.phaseLine != nil }
        #expect(model.line == L.castPhasePreparing(seconds: 5))

        play(session)
        try await waitFor { model.stage == .playing }
        #expect(model.live)
        #expect(model.line == L.tvStatusPlaying(name: "Living room"))
        #expect(model.fixedVolume)
        #expect(!model.canMute, "a TV whose own volume is in charge offers no mute")
        #expect(model.actions.isEmpty)
    }

    @Test func upNextAndUndoComeFromTheEngineAndAnswerBack() async throws {
        let (model, session) = casting()
        play(session)
        session.change {
            $0.upNext = FfiCastUpNext(number: "4", title: "Lanterns", still: nil, auto: true, secondsLeft: 6)
            $0.undo = FfiCastUndo(kind: .opening, to: 12)
        }
        try await waitFor { model.card?.upNext != nil }
        #expect(model.card?.upNext?.secondsLeft == 6)
        model.cancelNext()
        try await waitFor { model.card?.upNext == nil }
        model.undoSkip()
        try await waitFor { model.card?.undo == nil }
        #expect(abs((model.playhead() ?? 0) - 12) < 2)
    }

    @Test func eachFailureOffersItsFix() async throws {
        let cases: [(Msg, [CastAction])] = [
            (.noticeCastRefused(name: "Living room"), [.chooseSource, .playHere]),
            (.noticeCastVideoCodec(name: "Living room", codec: "AV1"), [.chooseSource, .playHere]),
            (.noticeCastNeverFetches(name: "Living room"), [.tryAgain, .playHere]),
            (.noticeCastNotSameNetwork(name: "Living room"), [.playHere, .chooseAnotherTV]),
            (.noticeCastDeepLink, [.chooseSource]),
        ]
        for (notice, actions) in cases {
            let (model, session) = casting()
            session.change {
                $0.notice = notice
                $0.over = true
            }
            try await waitFor { model.stage == .failed }
            #expect(model.actions == actions, "\(notice)")
            #expect(model.reason == notice.text)
            #expect(model.line == L.castCouldntStart(name: "Living room"))
        }
    }

    @Test func aTvThatDropsOffStopsTheCastAndPlayHereCarriesOn() async throws {
        let (model, session) = casting()
        var played: (AnilistId, String, Double?)?
        model.playLocally = { played = ($0, $1, $2) }
        play(session)
        try await waitFor { model.stage == .playing }
        // With the controller open, the controller says it stopped.
        model.expanded = true
        session.change {
            $0.notice = .noticeCastLost(name: "Living room")
            $0.over = true
        }
        try await waitFor { model.stage == .stopped }
        #expect(model.line == L.castStopped)
        #expect(model.actions == [.playHere, .chooseAnotherTV])

        model.playHere()
        #expect(!model.active)
        #expect(played?.0 == 900_001)
        #expect(played?.1 == "3")
        #expect(played?.2 == 492)
    }

    @Test func someoneElseTakingTheTvClosesTheCardWithANotice() async throws {
        let (model, session) = casting()
        var said: [Notice] = []
        var closed = false
        model.say = { said.append($0) }
        model.closed = { closed = true }
        play(session)
        try await waitFor { model.stage == .playing }
        session.change {
            $0.notice = .noticeCastTakenOver(name: "Living room")
            $0.over = true
        }
        try await waitFor { !model.active }
        #expect(closed)
        #expect(said.map(\.msg) == [.noticeCastTakenOver(name: "Living room")])
        #expect(model.offered?.from == 492)

        var played: Double?
        model.playLocally = { _, _, position in played = position }
        model.playHereFromNotice()
        #expect(played == 492)
        #expect(model.offered == nil)
    }

    @Test func aNewCastTakesTheTakeOverNoticeAndItsPlayHereAway() async throws {
        let (model, session) = casting()
        let notices = NoticesModel()
        model.say = { notices.post($0) }
        model.unsay = { notices.withdraw($0) }
        play(session)
        try await waitFor { model.stage == .playing }
        session.change {
            $0.notice = .noticeCastTakenOver(name: "Living room")
            $0.over = true
        }
        try await waitFor { !model.active }
        #expect(notices.current?.notice.msg == .noticeCastTakenOver(name: "Living room"))

        model.opener = { _, _ in FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3") }
        model.cast("cast:living", name: "Living room", request: request)
        #expect(model.stage == .starting)
        #expect(notices.current == nil)
        #expect(model.offered == nil)
        #expect(notices.log.count == 1, "Messages keeps it")
    }

    @Test func tryAgainCastsTheSameEpisodeToTheSameTv() async throws {
        let (model, session) = casting()
        session.change {
            $0.notice = .noticeCastNeverFetches(name: "Living room")
            $0.over = true
        }
        try await waitFor { model.stage == .failed }
        var opened: [String] = []
        model.opener = { key, _ in
            opened.append(key)
            return FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        }
        model.tryAgain()
        #expect(opened == ["cast:living"])
        #expect(model.stage == .starting)
        #expect(model.request == request)
    }

    @Test func theCardsControlsReachTheTv() async throws {
        let (model, session) = casting()
        play(session)
        try await waitFor { model.stage == .playing }
        session.change { $0.volumeControl = .level }
        try await waitFor { model.canMute }
        model.send(.mute(muted: true))
        try await waitFor { model.muted }
        model.send(.togglePause)
        try await waitFor { model.now?.paused == true }
        #expect(model.playhead() == 492)
    }

    @Test func aPlayStartedWhileCastingStaysOffThePlayerHere() {
        #expect(AppStore.playsHere(.playbackEnded(watched: false, ending: .stopped, rollingOn: false)))
        #expect(AppStore.playsHere(.playerSkipPrompt(playback: 1, prompt: .foundEdges(kind: .opening, episode: 7))))
        #expect(!AppStore.playsHere(.toast(Notice(kind: .info, text: "", hint: nil))))
    }

    // MARK: The session through a link

    nonisolated static func view(_ state: FfiCastSessionState, item: FfiCastItem? = nil) -> FfiCastSessionView {
        FfiCastSessionView(
            state: state, button: .connected, rev: 1, deviceKey: "cast:living", deviceName: "Living room",
            item: item, queue: [], notice: nil, note: nil, level: nil, muted: nil,
            volumeControl: .fixed, ownReceiver: true, canMute: false)
    }

    nonisolated static func item(at position: Double) -> FfiCastItem {
        FfiCastItem(
            id: 900_001, title: "Lanterns", episode: "3", episodeTitle: nil, image: nil,
            position: position, duration: 1420, paused: false, waiting: nil, loading: false)
    }

    @Test func leavingTheScreenWithoutTheCastsAudioPausesTheTvUntilTheAppIsBack() {
        let model = CastModel()
        let link = RecordingLink()
        model.link = link
        model.scene(active: false)
        #expect(link.aways == [true])
        model.scene(active: true)
        #expect(link.aways == [true, false])
    }

    @Test func whileTheCastsAudioRunsTheTvPlaysOnWithTheAppAway() {
        let model = CastModel()
        let link = RecordingLink()
        model.link = link
        model.audioHeld(true)
        model.scene(active: false)
        model.scene(active: true)
        model.scene(active: false)
        #expect(link.aways.isEmpty)
    }

    @Test func aCallThatStopsTheCastsAudioAwayPausesTheTvUntilItRunsAgain() {
        let model = CastModel()
        let link = RecordingLink()
        model.link = link
        model.audioHeld(true)
        model.scene(active: false)
        model.audioHeld(false)
        #expect(link.aways == [true])
        model.audioHeld(true)
        #expect(link.aways == [true, false])
        // The audio stopping in front says nothing: the app runs while it is in front.
        model.scene(active: true)
        model.audioHeld(false)
        #expect(link.aways == [true, false])
    }

    @Test func playHereOnceTheTvDroppedOffPlaysWhereItWas() {
        let model = CastModel()
        let link = RecordingLink()
        model.link = link
        var played: (AnilistId, String, Double?)?
        model.playLocally = { played = ($0, $1, $2) }
        model.take(view: Self.view(.playing, item: Self.item(at: 600)))
        // Lost: the engine names no episode any more, and has none to carry here.
        model.take(view: Self.view(.error))
        model.playHereFromSession()
        #expect(played?.0 == 900_001)
        #expect(played?.1 == "3")
        #expect((played?.2 ?? 0) >= 600)
    }

    @Test func stoppingTheCastForgetsWhatItPlayed() {
        let model = CastModel()
        let link = RecordingLink()
        model.link = link
        model.take(view: Self.view(.playing, item: Self.item(at: 600)))
        model.disconnect()
        var played = false
        model.playLocally = { _, _, _ in played = true }
        model.playHereFromSession()
        #expect(!played)
    }

    @Test func aTvLeftWithNothingLoadedReadsConnectedNotStarting() {
        let model = CastModel()
        model.link = RecordingLink()
        // The last episode plays to its end, with none after it.
        model.take(view: Self.view(.playing, item: Self.item(at: 1400)))
        model.expanded = true
        model.take(view: Self.view(.connected))
        #expect(model.heldIdle)
        #expect(model.sessionLine == L.castButtonConnected(name: "Living room"))
        #expect(!model.expanded)
        #expect(!model.miniShown)

        // A play that could not start: starting, then nothing.
        let starting = FfiCastItem(
            id: 900_001, title: "Lanterns", episode: "4", episodeTitle: nil, image: nil,
            position: 0, duration: nil, paused: false, waiting: nil, loading: true)
        model.take(view: Self.view(.playing, item: starting))
        #expect(!model.heldIdle)
        #expect(model.sessionLine == L.tvStartingOn(name: "Living room"))
        model.expanded = true
        model.take(view: Self.view(.connected))
        #expect(model.sessionLine == L.castButtonConnected(name: "Living room"))
        #expect(!model.expanded)
    }
}

extension CastTests {
    @Test func reconnectingHoldsThePlayheadUntilTheTvAnswersFurtherOn() {
        let model = CastModel()
        model.link = RecordingLink()
        model.take(view: Self.view(.playing, item: Self.item(at: 600)))
        var dropped = Self.view(.playing, item: Self.item(at: 600))
        dropped.rev = 2
        dropped.reconnecting = true
        model.take(view: dropped)
        #expect(model.reconnecting)
        #expect(model.sessionLine == L.tvStatusReconnecting(name: "Living room"))
        #expect(model.itemPosition(at: model.heldAt.addingTimeInterval(10)) == 600)

        var back = Self.view(.playing, item: Self.item(at: 615))
        back.rev = 3
        model.take(view: back)
        #expect(!model.reconnecting)
        #expect(model.sessionLine == L.tvStatusPlaying(name: "Living room"))
        #expect(model.itemPosition(at: model.heldAt) == 615)
    }

    @Test func aTvLostWithTheControllerClosedSaysSoAndOffersPlayHere() async throws {
        let (model, session) = casting()
        var said: [Notice] = []
        var kept: [Notice] = []
        model.say = { said.append($0) }
        model.keep = { kept.append($0) }
        play(session)
        try await waitFor { model.stage == .playing }
        session.change { $0.reconnecting = true }
        try await waitFor { model.reconnecting }
        #expect(model.line == L.tvStatusReconnecting(name: "Living room"))
        #expect(model.playhead(at: .now.addingTimeInterval(30)) == 492)

        session.change {
            $0.notice = .noticeCastLost(name: "Living room")
            $0.over = true
        }
        try await waitFor { !model.active }
        #expect(said.map(\.msg) == [.noticeCastLost(name: "Living room")])
        #expect(kept.isEmpty)
        #expect(model.offered?.from == 492)

        var played: Double?
        model.playLocally = { _, _, position in played = position }
        model.playHereFromNotice()
        #expect(played == 492)
    }

    @Test func aStopOnTheTvWithTheControllerOpenShowsThereAndGoesWithIt() async throws {
        let (model, session) = casting()
        var said: [Notice] = []
        var kept: [Notice] = []
        model.say = { said.append($0) }
        model.keep = { kept.append($0) }
        play(session)
        try await waitFor { model.stage == .playing }
        model.expanded = true
        session.change {
            $0.notice = .noticeCastStoppedElsewhere(name: "Living room")
            $0.over = true
        }
        try await waitFor { model.stage == .stopped }
        #expect(said.isEmpty)
        #expect(kept.map(\.msg) == [.noticeCastStoppedElsewhere(name: "Living room")], "Messages keeps it")
        #expect(model.reason == Msg.noticeCastStoppedElsewhere(name: "Living room").text)
        #expect(model.actions == [.playHere, .chooseAnotherTV])
        #expect(model.seekPoint() == 492)

        model.expanded = false
        #expect(!model.active)
        #expect(model.offered == nil)
    }

    @Test func waitingShowsOnlyOnceTheTvHasWaited600Ms() async throws {
        let model = CastModel()
        model.link = RecordingLink()
        model.take(view: Self.view(.playing, item: Self.item(at: 600)))
        var waits = Self.item(at: 600)
        waits.waiting = 0
        model.take(view: Self.view(.playing, item: waits))
        #expect(!model.waitingShown)
        // A short wait: over before 600 ms.
        model.take(view: Self.view(.playing, item: Self.item(at: 600)))
        try await Task.sleep(for: .milliseconds(800))
        #expect(!model.waitingShown)

        model.take(view: Self.view(.playing, item: waits))
        try await waitFor { model.waitingShown }
        #expect(model.sessionLine == L.tvWaitingFor(name: "Living room"))
        model.take(view: Self.view(.playing, item: Self.item(at: 602)))
        #expect(!model.waitingShown)
    }

    @Test func theFallbackNoteIsSaidEachTimeItComes() {
        let model = CastModel()
        model.link = RecordingLink()
        var said: [Msg?] = []
        model.say = { said.append($0.msg) }
        var playing = Self.view(.playing, item: Self.item(at: 10))
        playing.note = .noticeCastFallback(name: "Living room")
        model.take(view: playing)
        model.take(view: playing)
        #expect(said == [.noticeCastFallback(name: "Living room")])
        var connected = Self.view(.connected)
        model.take(view: connected)
        connected.note = .noticeCastFallback(name: "Living room")
        model.take(view: connected)
        #expect(said.count == 2)
    }

    @Test func aPlayOnTvCastsFallbackNoteIsSaidOnceEachTimeItComes() async throws {
        let fallback = Msg.noticeCastFallback(name: "Living room")
        let (model, session) = casting()
        var said: [Msg?] = []
        model.say = { said.append($0.msg) }
        // The fallback comes as the TV connects, and the card keeps it while it plays.
        session.change { $0.note = fallback }
        try await waitFor { said.count == 1 }
        play(session)
        try await waitFor { model.stage == .playing }
        #expect(said == [fallback])

        // It happens again on the next cast: said again.
        let again = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        model.opener = { _, _ in again }
        model.cast("cast:living", name: "Living room", request: request)
        again.change { $0.note = fallback }
        try await waitFor { said.count == 2 }
        #expect(said == [fallback, fallback])
    }

    @Test func aHeldSessionsCardLeavesTheFallbackNoteToTheSession() {
        let fallback = Msg.noticeCastFallback(name: "Living room")
        let model = CastModel()
        model.link = RecordingLink()
        var said: [Msg?] = []
        model.say = { said.append($0.msg) }
        var playing = Self.view(.playing, item: Self.item(at: 10))
        playing.note = fallback
        model.take(view: playing)
        let card = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        card.change { $0.note = fallback }
        model.take(card.card())
        #expect(said == [fallback])
    }
}

extension CastTests {
    /// The hunt's steps: cast, open the controller, the cast is stopped on the TV, then Choose
    /// another TV and the same TV again. The pick plays the episode on from where it was.
    @Test func chooseAnotherTvAfterAStopFromOutsideCarriesTheEpisodeToThePick() async throws {
        let tv = TVRemoteModel()
        let model = tv.cast
        let link = RecordingLink()
        model.link = link
        let session = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        model.opener = { _, _ in session }
        model.cast("cast:living", name: "Living room", request: request)
        play(session)
        try await waitFor { model.stage == .playing }
        model.expanded = true
        session.change {
            $0.notice = .noticeCastStoppedElsewhere(name: "Living room")
            $0.over = true
        }
        try await waitFor { model.stage == .stopped }
        #expect(model.actions == [.playHere, .chooseAnotherTV])

        tv.chooseAnotherTV()
        #expect(!model.expanded)
        #expect(!model.active, "the stopped card goes with the controller")
        #expect(model.chooserShown)

        tv.connectFromChooser(row("living", "Living room"))
        #expect(link.connects.map(\.key) == ["cast:living"])
        let from = try #require(link.connects.first?.from)
        #expect(from.id == 900_001)
        #expect(from.episode == "3")
        #expect(from.position == 492)
        #expect(tv.request == nil)
        #expect(!model.chooserShown)
        #expect(model.expandOnPlay, "the controller opens once the TV plays")
    }

    @Test func chooseAnotherTvWhileItPlaysCarriesTheEpisodeFromTheTvsPlayhead() {
        let tv = TVRemoteModel()
        let link = RecordingLink()
        tv.cast.link = link
        tv.cast.take(view: Self.view(.playing, item: Self.item(at: 600)))
        tv.chooseAnotherTV()
        #expect(tv.request?.episode == "3")
        #expect((tv.request?.from ?? 0) >= 600)

        tv.connectFromChooser(row("kitchen", "Kitchen"))
        #expect(link.connects.map(\.key) == ["cast:kitchen"])
        #expect(link.connects.first?.from?.id == 900_001)
        #expect((link.connects.first?.from?.position ?? 0) >= 600)
    }

    @Test func theCastButtonsChooserCarriesNothing() {
        let tv = TVRemoteModel()
        let link = RecordingLink()
        tv.cast.link = link
        tv.cast.take(view: Self.view(.playing, item: Self.item(at: 600)))
        // Choose another TV, closed with no pick, then the TV let go.
        tv.chooseAnotherTV()
        tv.cast.chooserShown = false
        tv.cast.disconnect()

        tv.castButtonTapped()
        #expect(tv.cast.chooserShown)
        #expect(tv.request == nil)
        tv.connectFromChooser(row("kitchen", "Kitchen"))
        #expect(link.connects.count == 1)
        #expect(link.connects.first?.from == nil)
    }
}

extension CastTests {
    /// A phone that joins a cast another device started: the engine's card says what plays, so
    /// the mini player opens the remote on it, playing.
    @Test func aCastAnotherDeviceStartedOpensTheRemoteOnAPhoneThatJoinsIt() {
        let tv = TVRemoteModel()
        let session = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        play(session)
        tv.cast.link = JoinedLink(session: session)
        tv.cast.take(view: Self.view(.playing, item: Self.item(at: 492)))
        #expect(tv.cast.miniShown)

        tv.openRemote()
        #expect(tv.cast.expanded)
        #expect(tv.cast.stage == .playing)
        #expect(tv.cast.card?.card.now?.id == 900_001)
        #expect(tv.cast.card?.card.now?.episode == "3")
    }

    /// Another app's cast, or an older Kaeri's, says nothing of what it plays: the mini's
    /// controls drive it, its line says it plays, and the remote doesn't open.
    @Test func aJoinedCastThatDoesNotSayWhatItPlaysKeepsToTheMini() {
        let tv = TVRemoteModel()
        let session = FixtureCastSession(name: "Living room", request: request, episodeLine: "Episode 3")
        play(session)
        let link = JoinedLink(session: session)
        tv.cast.link = link
        var theirs = Self.item(at: 120)
        theirs.id = nil
        theirs.episode = nil
        tv.cast.take(view: Self.view(.playing, item: theirs))
        #expect(tv.cast.miniShown)
        #expect(!tv.cast.active, "no card follows it")
        #expect(tv.cast.sessionLine == L.tvStatusPlaying(name: "Living room"))

        tv.openRemote()
        #expect(!tv.cast.expanded)
        tv.cast.expanded = true
        #expect(!tv.cast.expanded, "the mini's tap opens nothing either")
        tv.cast.command(.togglePause)
        #expect(link.commands.count == 1, "the pause went to the TV")
    }
}

/// A held session another device started, whose card the engine reads from the TV.
private final class JoinedLink: CastSessionLink, @unchecked Sendable {
    let session: FixtureCastSession
    var commands: [FfiTvCommand] = []

    init(session: FixtureCastSession) {
        self.session = session
    }

    func state() -> FfiCastSessionView { CastTests.view(.playing, item: CastTests.item(at: 492)) }
    func next() async -> FfiCastSessionView? { nil }
    func connect(key: String, from: FfiCastFrom?) throws {}
    func disconnect() {}
    func playHere() -> FfiCastFrom? { nil }
    func command(_ command: FfiTvCommand) { commands.append(command) }
    func card() -> (any FfiCastSessionProtocol)? { session }
    func away(_ away: Bool) {}
    func played(_ request: TVPlayRequest) {}
}

/// A link that records what the model tells the engine of the app's place.
private final class RecordingLink: CastSessionLink, @unchecked Sendable {
    var aways: [Bool] = []
    var connects: [(key: String, from: FfiCastFrom?)] = []

    func state() -> FfiCastSessionView { CastTests.view(.idle) }
    func next() async -> FfiCastSessionView? { nil }
    func connect(key: String, from: FfiCastFrom?) throws { connects.append((key, from)) }
    func disconnect() {}
    func playHere() -> FfiCastFrom? { nil }
    func command(_ command: FfiTvCommand) {}
    func card() -> (any FfiCastSessionProtocol)? { nil }
    func away(_ away: Bool) { aways.append(away) }
    func played(_ request: TVPlayRequest) {}
}

package app.kaeri.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import app.kaeri.engine.core.AnilistId
import app.kaeri.engine.ffi.FfiCastBrowser
import app.kaeri.engine.ffi.FfiCastCard
import app.kaeri.engine.ffi.FfiCastSink
import app.kaeri.engine.ffi.FfiCastRouteHint
import app.kaeri.engine.ffi.FfiCastRow
import app.kaeri.engine.ffi.FfiLinkPhase
import app.kaeri.engine.ffi.FfiNowCard
import app.kaeri.engine.ffi.FfiRemoteCard
import app.kaeri.engine.ffi.FfiScreen
import app.kaeri.engine.ffi.FfiTvCommand
import app.kaeri.engine.ffi.FfiVolumeControl
import app.kaeri.engine.model.Msg

/** The cast card's stages, its endings and its volume, on a scripted TV with no engine. */
@OptIn(ExperimentalCoroutinesApi::class)
class CastModelTest {
    private var time = 0L

    /** A TV that sends what the test tells it to, and keeps what it was asked. */
    private class ScriptedLink(first: FfiCastCard) : CastLink {
        override val tvName = "Living room"
        var card = first
        val cards = Channel<FfiCastCard?>(Channel.UNLIMITED)
        val sent = mutableListOf<FfiTvCommand>()
        var stopped = false
        var playedHere = false
        val asked = mutableListOf<String>()
        var again = true

        override fun card() = card
        override suspend fun nextCard(): FfiCastCard? = cards.receive()?.also { card = it }
        override fun command(command: FfiTvCommand) {
            sent += command
        }

        override fun playHere(): Double? {
            playedHere = true
            return card.card.now?.position
        }

        override fun playNext() {
            asked += "play-next"
        }

        override fun cancelNext() {
            asked += "cancel-next"
        }

        override fun undoSkip() {
            asked += "undo-skip"
        }

        override fun tryAgain(): Boolean {
            asked += "try-again"
            return again
        }

        override fun stop() {
            stopped = true
        }
    }

    private val request = TvPlayRequest(id = 900001u, title = "The Lantern Road", episode = "7", from = null, fromPlayer = true)

    private fun card(
        loading: Boolean,
        volume: Double? = 40.0,
        over: Boolean = false,
        notice: Msg? = null,
        control: FfiVolumeControl = FfiVolumeControl.LEVEL,
        canMute: Boolean = control == FfiVolumeControl.LEVEL,
        position: Double = 120.0,
        reconnecting: Boolean = false,
        waiting: UInt? = null,
        note: Msg? = null,
        episode: String = "7",
    ) = FfiCastCard(
        card = FfiRemoteCard(
            tvName = "Living room",
            phase = FfiLinkPhase.CONNECTED,
            phaseLine = null,
            screen = FfiScreen.Player,
            now = FfiNowCard(
                id = 900001u, title = "The Lantern Road", episode = episode, episodeLine = "Episode $episode", loading = loading,
                position = position, duration = 1420.0, paused = false, speed = 1.0, volume = volume, audio = emptyList(),
                subtitles = emptyList(), subtitlesOff = false, skip = null,
            ),
            rev = 1u,
            over = over,
            reached = true,
        ),
        volumeControl = control,
        muted = false,
        notice = notice,
        waiting = waiting,
        reconnecting = reconnecting,
        note = note,
        canMute = canMute,
    )

    private fun TestScope.model(link: ScriptedLink): CastModel {
        val cast = CastModel(CoroutineScope(StandardTestDispatcher(testScheduler)), clock = { time })
        cast.starter = { _, _ -> link }
        return cast
    }

    private val row = FfiCastRow(
        key = "cast:fixture-living", name = "Living room", model = "Chromecast", address = "127.0.0.2",
        routeHint = FfiCastRouteHint.THROUGH_THIS_DEVICE,
    )

    @Test
    fun a_cast_starts_plays_and_closes_the_player_once() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        var handed = 0
        cast.handedOff = { handed++ }
        cast.cast(row, request)
        assertEquals(CastStage.Starting, cast.shown.value?.stage)
        assertTrue(cast.open.value)
        time = 5_000
        assertEquals(5L, cast.startingSeconds())
        link.cards.send(card(loading = false))
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        assertEquals(1, handed)
    }

    @Test
    fun taken_over_closes_the_card_with_a_notice_and_lost_keeps_it() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        val ended = backgroundScope.async { cast.ended.first() }
        runCurrent()
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastTakenOver("Living room")))
        advanceUntilIdle()
        assertNull(cast.shown.value)
        assertFalse(cast.open.value)
        assertEquals("7", ended.await().episode)

        val again = ScriptedLink(card(loading = false))
        val lost = model(again)
        lost.cast(row, request)
        again.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Lost, lost.shown.value?.stage)
        lost.dismiss()
        assertNull(lost.shown.value)
    }

    @Test
    fun the_volume_keys_step_5_and_mute_only_on_a_tv_that_takes_a_level() = runTest {
        val level = ScriptedLink(card(loading = false))
        val cast = model(level)
        cast.cast(row, request)
        cast.stepVolume(up = true)
        cast.setVolume(55.0)
        cast.toggleMute()
        assertEquals(listOf(FfiTvCommand.Volume(5.0), FfiTvCommand.Volume(15.0), FfiTvCommand.Mute(true)), level.sent)

        // A TV whose own volume is in charge takes neither: its sound stays where the TV has it.
        val fixed = ScriptedLink(card(loading = false, volume = null, control = FfiVolumeControl.FIXED))
        val other = model(fixed)
        other.cast(row, request)
        other.stepVolume(up = true)
        other.toggleMute()
        assertEquals(emptyList<FfiTvCommand>(), fixed.sent)

        // One that refused a mute is not sent another.
        val refused = ScriptedLink(card(loading = false, canMute = false))
        val third = model(refused)
        third.cast(row, request)
        third.toggleMute()
        third.stepVolume(up = false)
        assertEquals(listOf(FfiTvCommand.Volume(-5.0)), refused.sent)
    }

    @Test
    fun play_here_stops_the_tv_and_plays_on_here() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        var played: Pair<AnilistId, String>? = null
        cast.playHere = { id, episode -> played = id to episode }
        cast.cast(row, request)
        cast.playHere()
        assertTrue(link.playedHere)
        assertEquals(request.id to "7", played)
        assertNull(cast.shown.value)
    }

    @Test
    fun a_cast_from_play_on_tv_fills_the_button_and_marks_its_row() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        assertEquals(CastButtonState.Hidden, cast.button.value)
        cast.cast(row, request)
        assertEquals(CastButtonState.Connecting, cast.button.value)
        assertEquals(row.key, CastModel.playingKey(cast.shown.value, cast.session.value))
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastButtonState.Connected, cast.button.value)
        // The button opens the TV's dialog, not the chooser.
        cast.closeCard()
        cast.pressButton()
        assertEquals(CastDialog.Connected, cast.dialog.value)
        cast.closeDialog()
        cast.openCard()

        // Lost: the card stays to say so, and the button and the row let go.
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Lost, cast.shown.value?.stage)
        assertEquals(CastButtonState.Hidden, cast.button.value)
        assertNull(CastModel.playingKey(cast.shown.value, cast.session.value))
    }

    @Test
    fun the_cast_button_during_a_cast_from_play_on_tv_opens_the_tvs_dialog() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        // From the mini player, with the remote closed.
        cast.closeCard()
        cast.pressButton()
        assertEquals(CastDialog.Connected, cast.dialog.value)
        assertFalse(cast.open.value)
        // From the remote's own header: the dialog comes up over the remote.
        cast.closeDialog()
        cast.openCard()
        cast.pressButton()
        assertEquals(CastDialog.Connected, cast.dialog.value)
        assertTrue(cast.open.value)
        // Stop casting there lets the TV go, and the dialog with it.
        cast.disconnect()
        assertTrue(link.stopped)
        assertNull(cast.dialog.value)
        assertNull(cast.shown.value)
    }

    @Test
    fun the_tvs_dialog_goes_when_a_cast_from_play_on_tv_ends_under_it() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        cast.closeCard()
        cast.pressButton()
        assertEquals(CastDialog.Connected, cast.dialog.value)
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastTakenOver("Living room")))
        advanceUntilIdle()
        assertNull(cast.shown.value)
        assertNull(cast.dialog.value)
    }

    @Test
    fun a_stopped_cast_from_play_on_tv_lets_the_button_go() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastButtonState.Connected, cast.button.value)
        cast.stop()
        assertTrue(link.stopped)
        assertEquals(CastButtonState.Hidden, cast.button.value)
        assertNull(CastModel.playingKey(cast.shown.value, cast.session.value))
    }

    @Test
    fun up_next_and_undo_go_to_the_engine() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        cast.playNext()
        cast.cancelNext()
        cast.undoSkip()
        assertEquals(listOf("play-next", "cancel-next", "undo-skip"), link.asked)
    }

    @Test
    fun a_cast_that_played_and_then_failed_says_it_stopped_not_that_it_could_not_start() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastSegment("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Lost, cast.shown.value?.stage)
    }

    @Test
    fun try_again_starts_the_failed_cast_over_on_the_same_session() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastNeverFetches("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Failed, cast.shown.value?.stage)
        time = 9_000
        // The engine's card starts over.
        link.card = card(loading = true)
        cast.tryAgain()
        assertEquals(listOf("try-again"), link.asked)
        assertEquals(CastStage.Starting, cast.shown.value?.stage)
        assertEquals(9_000L, cast.shown.value?.startedAt)
        link.cards.send(card(loading = false, volume = 41.0))
        advanceUntilIdle()
        assertEquals(41.0, cast.shown.value?.cast?.card?.now?.volume)
    }

    /** A Cast browse that counts how often it was asked to resolve what it found again. */
    private class RelookingBrowser : FfiCastBrowser, NsdRelook {
        var relooked = 0
        override fun start(sink: FfiCastSink) {}
        override fun stop() {}
        override fun relook() {
            relooked++
        }
    }

    @Test
    fun a_cast_that_was_lost_resolves_the_tvs_again() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        val browser = RelookingBrowser()
        cast.attach(TvRemoteModel(CoroutineScope(StandardTestDispatcher(testScheduler))), browser)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(0, browser.relooked)
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        assertEquals(1, browser.relooked)
    }

    @Test
    fun a_cast_stopped_here_resolves_nothing_again() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        val browser = RelookingBrowser()
        cast.attach(TvRemoteModel(CoroutineScope(StandardTestDispatcher(testScheduler))), browser)
        cast.cast(row, request)
        link.cards.send(card(loading = false, over = true))
        advanceUntilIdle()
        assertEquals(0, browser.relooked)
    }

    /** A held TV's session that says what the test tells it to. */
    private class ScriptedSessions(private val link: ScriptedLink) : CastSessionSource {
        var view = CastSession.IDLE
        val views = Channel<CastSession?>(Channel.UNLIMITED)
        override fun state() = view
        override suspend fun next(): CastSession? = views.receive()?.also { view = it }
        override fun connect(key: String, from: CastFrom?) {}
        override fun disconnect() {}
        override fun command(command: FfiTvCommand) {}
        override fun play(request: TvPlayRequest) {}
        override val queues = false
        override fun playNext(id: AnilistId, episode: String) = false
        override fun playHere(): CastFrom? = null
        override fun leftOn(): String? = "cast:fixture-living"
        override fun card(): CastLink = link
    }

    private val item = CastItem(
        id = 900001u, title = "The Lantern Road", episode = "7", episodeTitle = null, image = null, position = 21.4,
        duration = 1420.0, paused = false, loading = false, waiting = null,
    )

    private fun view(state: CastSessionState, notice: Msg? = null) = CastSession(
        state = state, button = null, rev = 1u, deviceKey = row.key, deviceName = "Living room",
        item = item.takeIf { state == CastSessionState.Playing }, queue = emptyList(), notice = notice, note = null,
        level = 0.4, muted = false, levelControl = true,
    )

    /** A held TV playing the card's episode, its controller down, as with the phone locked. */
    private fun TestScope.playingOnAHeldTv(link: ScriptedLink): Pair<CastModel, ScriptedSessions> {
        val cast = model(link)
        val sessions = ScriptedSessions(link)
        cast.sessions = sessions
        sessions.views.trySend(view(CastSessionState.Connected))
        runCurrent()
        assertTrue(cast.playOnTv(request))
        sessions.views.trySend(view(CastSessionState.Playing))
        runCurrent()
        cast.closeCard()
        runCurrent()
        return cast to sessions
    }

    @Test
    fun a_held_tv_lost_with_the_controller_down_ends_with_a_notice_and_plays_on_nowhere() = runTest {
        val link = ScriptedLink(card(loading = false))
        val (cast, sessions) = playingOnAHeldTv(link)
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        assertTrue("the session is kept up while it plays", cast.casting.value)
        val ended = backgroundScope.async { cast.ended.first() }
        runCurrent()

        sessions.views.trySend(view(CastSessionState.Error, Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        assertNull(cast.shown.value)
        assertFalse("nothing claims to play on", cast.casting.value)
        assertNull("never a Reconnecting that waits on nothing", cast.joining.value)
        val said = ended.await()
        assertEquals(Msg.NoticeCastLost("Living room"), said.notice)
        assertEquals("7", said.episode)
    }

    @Test
    fun a_held_tv_lost_with_the_controller_up_keeps_a_card_that_says_so() = runTest {
        val link = ScriptedLink(card(loading = false))
        val (cast, sessions) = playingOnAHeldTv(link)
        cast.openCard()
        sessions.views.trySend(view(CastSessionState.Error, Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        val shown = cast.shown.value
        assertEquals(CastStage.Lost, shown?.stage)
        assertFalse(shown!!.live)
        assertFalse(cast.casting.value)
        // A card the engine sends late changes nothing: the card no longer follows it.
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastStage.Lost, cast.shown.value?.stage)
        cast.dismiss()
        assertNull(cast.shown.value)
    }

    @Test
    fun a_cast_lost_with_the_controller_down_says_so_once() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        cast.closeCard()
        val said = mutableListOf<CastEnded>()
        // Collected as it is said: advanceUntilIdle stops once only background work is left.
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { cast.ended.collect { said += it } }
        runCurrent()
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastLost("Living room")))
        advanceUntilIdle()
        assertNull(cast.shown.value)
        assertEquals(listOf(Msg.NoticeCastLost("Living room")), said.map { it.notice })
    }

    @Test
    fun only_a_session_with_nothing_held_or_ended_is_joined_again() {
        val rows = listOf(row)
        assertEquals("Living room", CastModel.joining(view(CastSessionState.Idle), row.key, rows))
        assertNull(CastModel.joining(view(CastSessionState.Error, Msg.NoticeCastLost("Living room")), row.key, rows))
        assertNull(CastModel.joining(view(CastSessionState.Playing), row.key, rows))
        assertNull(CastModel.joining(view(CastSessionState.Idle), row.key, emptyList()))
        assertNull(CastModel.joining(view(CastSessionState.Idle), null, rows))
        assertNull(CastModel.joining(view(CastSessionState.Idle), row.key, rows, live = true))
    }

    @Test
    fun a_tv_picked_again_after_a_lost_cast_plays_without_reading_reconnecting() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        val sessions = ScriptedSessions(link)
        cast.sessions = sessions
        runCurrent()
        // The session ended lost and the TV is found again: its left session is joined meanwhile.
        cast.takeRows(listOf(row))
        assertEquals("Living room", cast.joining.value)

        // Choose another TV: the play goes to the TV on its own, the session stays idle.
        cast.cast(row, request)
        sessions.views.trySend(view(CastSessionState.Idle))
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        assertNull("the TV plays the new cast, never Reconnecting", cast.joining.value)
        cast.takeRows(listOf(row))
        assertNull(cast.joining.value)
    }

    @Test
    fun a_reconnecting_tv_holds_the_playhead_until_it_answers_again() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        time = 10_000
        assertEquals(130.0, cast.playhead()!!, 0.001)
        link.cards.send(card(loading = false, reconnecting = true))
        advanceUntilIdle()
        assertTrue(cast.shown.value!!.reconnecting)
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        time = 25_000
        assertEquals("the clock stops where it was as the connection closed", 130.0, cast.playhead()!!, 0.001)
        // The TV played on meanwhile: it answers ahead of where the playhead held.
        link.cards.send(card(loading = false, position = 145.0))
        advanceUntilIdle()
        assertFalse(cast.shown.value!!.reconnecting)
        assertNull(cast.shown.value!!.frozen)
        assertEquals(145.0, cast.playhead()!!, 0.001)
        time = 27_000
        assertEquals(147.0, cast.playhead()!!, 0.001)
    }

    @Test
    fun a_tv_waiting_while_its_stop_is_learned_holds_the_playhead() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        advanceUntilIdle()
        time = 4_000
        link.cards.send(card(loading = false, waiting = 0u))
        advanceUntilIdle()
        time = 7_000
        assertEquals(124.0, cast.playhead()!!, 0.001)
    }

    @Test
    fun a_cast_stopped_on_the_tv_with_the_controller_down_says_so_with_play_here_and_keeps_it() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        val kept = mutableListOf<Msg>()
        cast.kept = { kept += it }
        cast.cast(row, request)
        cast.closeCard()
        val said = mutableListOf<CastEnded>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { cast.ended.collect { said += it } }
        runCurrent()
        link.cards.send(card(loading = false))
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastStoppedElsewhere("Living room")))
        advanceUntilIdle()
        assertNull(cast.shown.value)
        assertEquals(listOf(Msg.NoticeCastStoppedElsewhere("Living room")), said.map { it.notice })
        assertEquals("7", said.single().episode)
        assertEquals(listOf<Msg>(Msg.NoticeCastStoppedElsewhere("Living room")), kept)
    }

    @Test
    fun a_cast_stopped_on_the_tv_with_the_controller_up_reads_casting_stopped() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        cast.cast(row, request)
        link.cards.send(card(loading = false))
        link.cards.send(card(loading = false, over = true, notice = Msg.NoticeCastStoppedElsewhere("Living room")))
        advanceUntilIdle()
        assertEquals(CastStage.Lost, cast.shown.value?.stage)
        assertFalse(cast.shown.value!!.reconnecting)
    }

    @Test
    fun a_fallback_note_is_said_and_kept_once_each_time_it_happens() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        val sessions = ScriptedSessions(link)
        cast.sessions = sessions
        val kept = mutableListOf<Msg>()
        cast.kept = { kept += it }
        val notes = mutableListOf<Msg>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { cast.notes.collect { notes += it } }
        val fallback = Msg.NoticeCastFallback("Living room")
        // The fallback usually comes at connect, while nothing plays yet.
        sessions.views.trySend(view(CastSessionState.Connected).copy(note = fallback))
        sessions.views.trySend(view(CastSessionState.Connected).copy(note = fallback, rev = 2u))
        advanceUntilIdle()
        assertEquals(listOf<Msg>(fallback), notes)
        assertEquals(listOf<Msg>(fallback), kept)
    }

    @Test
    fun a_play_on_tv_casts_fallback_note_is_said_and_kept_once_each_time_it_happens() = runTest {
        val link = ScriptedLink(card(loading = true))
        val cast = model(link)
        val kept = mutableListOf<Msg>()
        cast.kept = { kept += it }
        val notes = mutableListOf<Msg>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { cast.notes.collect { notes += it } }
        val fallback = Msg.NoticeCastFallback("Living room")
        cast.cast(row, request)
        // The fallback comes as the TV connects, and the card keeps it while it plays.
        link.cards.trySend(card(loading = true, note = fallback))
        link.cards.trySend(card(loading = false, note = fallback))
        link.cards.trySend(card(loading = false, position = 130.0, note = fallback))
        advanceUntilIdle()
        assertEquals(listOf<Msg>(fallback), notes)
        assertEquals(listOf<Msg>(fallback), kept)
        // It happens again on the next cast: said again.
        val again = ScriptedLink(card(loading = true))
        cast.starter = { _, _ -> again }
        cast.cast(row, request)
        again.cards.trySend(card(loading = false, note = fallback))
        advanceUntilIdle()
        assertEquals(listOf<Msg>(fallback, fallback), notes)
        assertEquals(listOf<Msg>(fallback, fallback), kept)
    }

    @Test
    fun a_held_sessions_card_leaves_the_fallback_note_to_the_session() = runTest {
        val link = ScriptedLink(card(loading = false))
        val (cast, sessions) = playingOnAHeldTv(link)
        val notes = mutableListOf<Msg>()
        backgroundScope.launch(UnconfinedTestDispatcher(testScheduler)) { cast.notes.collect { notes += it } }
        val fallback = Msg.NoticeCastFallback("Living room")
        sessions.views.trySend(view(CastSessionState.Playing).copy(note = fallback, rev = 3u))
        link.cards.trySend(card(loading = false, note = fallback))
        advanceUntilIdle()
        assertEquals(listOf<Msg>(fallback), notes)
    }

    @Test
    fun a_reconnecting_session_holds_its_item_playhead() = runTest {
        val link = ScriptedLink(card(loading = false))
        val cast = model(link)
        val sessions = ScriptedSessions(link)
        cast.sessions = sessions
        sessions.views.trySend(view(CastSessionState.Playing))
        advanceUntilIdle()
        time = 3_000
        sessions.views.trySend(view(CastSessionState.Playing).copy(reconnecting = true))
        advanceUntilIdle()
        time = 13_000
        assertEquals(24.4, cast.itemPlayhead()!!, 0.001)
    }

    /** A phone that joins a cast another device started: no play of its own, the session's item alone. */
    private fun TestScope.joined(link: ScriptedLink, item: CastItem?): Pair<CastModel, ScriptedSessions> {
        val cast = model(link)
        val sessions = ScriptedSessions(link)
        cast.sessions = sessions
        sessions.views.trySend(view(CastSessionState.Connected))
        runCurrent()
        sessions.views.trySend(view(CastSessionState.Playing).copy(item = item))
        runCurrent()
        return cast to sessions
    }

    @Test
    fun a_cast_another_device_started_opens_the_remote_on_a_phone_that_joins_it() = runTest {
        val link = ScriptedLink(card(loading = false))
        val (cast, _) = joined(link, item)
        assertEquals(CastStage.Playing, cast.shown.value?.stage)
        assertEquals("7", cast.shown.value?.request?.episode)
        cast.openCard()
        assertTrue("the remote opens", cast.open.value)
    }

    @Test
    fun a_joined_cast_that_does_not_say_what_it_plays_opens_no_remote() = runTest {
        val link = ScriptedLink(card(loading = false))
        val (cast, _) = joined(link, item.copy(id = null, episode = null))
        assertNull("no card to open", cast.shown.value)
        cast.openCard()
        assertFalse(cast.open.value)
    }

    @Test
    fun a_joined_cast_follows_the_next_episode_the_other_device_casts() = runTest {
        val link = ScriptedLink(card(loading = false))
        val (cast, sessions) = joined(link, item)
        cast.openCard()
        // The engine's card and the session's item both move to the episode the TV plays.
        link.cards.send(card(loading = false, episode = "8"))
        sessions.views.trySend(view(CastSessionState.Playing).copy(item = item.copy(episode = "8")))
        advanceUntilIdle()
        val shown = cast.shown.value
        assertEquals(CastStage.Playing, shown?.stage)
        assertEquals("8", shown?.cast?.card?.now?.episode)
        assertEquals("8", shown?.request?.episode)
        assertTrue(cast.open.value)
        // Ended later, it says the episode the TV was on.
        val ended = backgroundScope.async { cast.ended.first() }
        runCurrent()
        cast.closeCard()
        sessions.views.trySend(view(CastSessionState.Error, Msg.NoticeCastLost("Living room")).copy(item = null))
        advanceUntilIdle()
        assertEquals("8", ended.await().episode)
    }
}

package app.kaeri.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
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
    ) = FfiCastCard(
        card = FfiRemoteCard(
            tvName = "Living room",
            phase = FfiLinkPhase.CONNECTED,
            phaseLine = null,
            screen = FfiScreen.Player,
            now = FfiNowCard(
                id = 900001u, title = "The Lantern Road", episode = "7", episodeLine = "Episode 7", loading = loading,
                position = 120.0, duration = 1420.0, paused = false, speed = 1.0, volume = volume, audio = emptyList(),
                subtitles = emptyList(), subtitlesOff = false, skip = null,
            ),
            rev = 1u,
            over = over,
            reached = true,
        ),
        volumeControl = control,
        muted = false,
        notice = notice,
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
    fun the_volume_keys_step_5_only_on_a_tv_that_takes_a_level() = runTest {
        val level = ScriptedLink(card(loading = false))
        val cast = model(level)
        cast.cast(row, request)
        cast.stepVolume(up = true)
        cast.setVolume(55.0)
        assertEquals(listOf(FfiTvCommand.Volume(5.0), FfiTvCommand.Volume(15.0)), level.sent)

        val fixed = ScriptedLink(card(loading = false, volume = null, control = FfiVolumeControl.MUTE_ONLY))
        val other = model(fixed)
        other.cast(row, request)
        other.stepVolume(up = true)
        other.toggleMute()
        assertEquals(listOf(FfiTvCommand.Mute(true)), fixed.sent)
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
        backgroundScope.launch { cast.ended.collect { said += it } }
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
    }
}

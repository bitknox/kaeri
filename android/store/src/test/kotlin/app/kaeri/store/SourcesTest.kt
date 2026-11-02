package app.kaeri.store

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.assertNull
import org.junit.Test
import app.kaeri.engine.core.SourceCandidate
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Notice
import app.kaeri.engine.model.NoticeKind
import app.kaeri.engine.model.ProxyStatus
import app.kaeri.engine.model.TorrentProxy

/** An episode's releases, listed for the title page's Choose source and the player's. */
class SourcesTest {
    private val release = SourceCandidate("r1", "fixture", "Release one", 1080u, null, "1.2 GiB", false, false, true)

    @Test
    fun the_releases_listed_reach_the_choice_for_their_episode_and_the_player() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val heard = mutableListOf<Event>()
        store.onPlayback = { heard += it }
        store.loadSources(900001u, "7")
        assertEquals(listOf<Command>(Command.LoadSources(900001u, "7")), store.waiting)
        assertEquals(SourceChoices(900001u, "7"), store.sources.value)

        // Another episode's answer is not this choice's.
        store.apply(Event.Sources(900001u, "8", listOf(release)))
        assertNull(store.sources.value?.candidates)
        store.apply(Event.Sources(900001u, "7", listOf(release)))
        assertEquals(listOf(release), store.sources.value?.candidates)
        assertEquals(2, heard.size)

        store.playSource(900001u, "7", release)
        assertNull(store.sources.value)
        assertEquals(Command.PlaySource(900001u, "7", "fixture", "r1"), store.waiting.last())
    }

    @Test
    fun an_alert_reaches_the_player_as_well_as_the_notices_and_an_info_does_not() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val heard = mutableListOf<Event>()
        store.onPlayback = { heard += it }
        val alert = Event.Toast(Notice(NoticeKind.ALERT, "no playable stream", null))
        store.apply(Event.Toast(Notice(NoticeKind.INFO, "looking", null)))
        store.apply(alert)
        assertEquals(listOf<Event>(alert), heard)
    }

    @Test
    fun a_proxy_check_runs_one_at_a_time_and_its_verdict_lands_on_what_it_checked() {
        val store = AppStore(CoroutineScope(Dispatchers.Unconfined))
        val first = TorrentProxy("proxy.example", 1080u, "mika", "one")
        val second = TorrentProxy("proxy.example", 1081u, "mika", null)
        val third = TorrentProxy("other.example", 1080u, null, null)
        store.checkTorrentProxy(first)
        store.checkTorrentProxy(second)
        store.checkTorrentProxy(third)
        // Only the first goes to the engine; the newest of the rest waits for its verdict.
        assertEquals(listOf<Command>(Command.CheckTorrentProxy(first)), store.waiting)

        // The first's verdict is no form's any more: the third is sent, and is still checking.
        store.apply(Event.ProxyChecked(ProxyStatus.FAILED))
        assertEquals(Command.CheckTorrentProxy(third), store.waiting.last())
        val checking = store.providers.trial.value
        assertTrue(checking is ProvidersModel.Trial.Checking && checking.proxy == third)

        store.apply(Event.ProxyChecked(ProxyStatus.PASSED))
        val said = store.providers.trial.value as ProvidersModel.Trial.Said
        assertEquals(third, said.proxy)
        assertTrue(said.passed)

        // A verdict after its form let go is not kept, and the next check goes straight out.
        store.checkTorrentProxy(first)
        store.providers.clearTrial()
        store.apply(Event.ProxyChecked(ProxyStatus.PASSED))
        assertNull(store.providers.trial.value)
        store.checkTorrentProxy(second)
        assertEquals(Command.CheckTorrentProxy(second), store.waiting.last())
    }
}

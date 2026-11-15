// Private mode and always private as the store keeps them: what the engine says, what the switch
// sends, and the line a title or series page shows from them.

package app.kaeri.store

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.UnconfinedTestDispatcher
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.DownloadRow
import app.kaeri.engine.model.DownloadState
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.PrivateMode
import app.kaeri.engine.model.PrivateTitle
import app.kaeri.engine.model.ShowSettingsView
import app.kaeri.engine.model.TitleProgress

@OptIn(ExperimentalCoroutinesApi::class)
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class PrivacyTest {
    private val context = RuntimeEnvironment.getApplication()
    private val main = UnconfinedTestDispatcher()
    private var now = 0L
    private lateinit var store: AppStore
    private val show = PrivateTitle.Show(900001u)

    @Before
    fun setUp() {
        Dispatchers.setMain(main)
        store = AppStore(scope = TestScope(main), jobs = JobsModel(now = { now }), decoder = { _, _ -> null }, clock = { now })
    }

    @After
    fun tearDown() = Dispatchers.resetMain()

    private fun progress(id: UInt, mode: PrivateMode?, kept: Boolean = false) =
        Event.Progress(id, TitleProgress(done = 3u, next = 4u), null, emptyList(), private = mode, kept = kept)

    @Test
    fun the_switch_turns_it_on_at_once_and_tells_the_engine() {
        assertFalse(store.privacy.on.value)
        store.setPrivate(true)
        assertTrue(store.privacy.on.value)
        store.setPrivate(false)
        assertFalse(store.privacy.on.value)
        assertEquals(listOf(Command.SetPrivate(true), Command.SetPrivate(false)), store.waiting)
    }

    @Test
    fun the_engine_saying_it_is_what_stands() {
        store.apply(Event.Private(true))
        assertTrue(store.privacy.on.value)
        assertEquals(PrivateLine.Session, store.privacy.line(show))
        store.apply(Event.Private(false))
        assertNull(store.privacy.line(show))
    }

    @Test
    fun a_shows_settings_say_whether_it_is_always_private() {
        store.apply(Event.ShowSettings(ShowSettingsView(id = 900001u, rows = emptyList(), remembered = emptyList(), source = null, alwaysPrivate = true)))
        assertTrue(store.privacy.isAlways(show))
        assertEquals(PrivateLine.Always, store.privacy.line(show))
        // The show's own flag comes first, whether the mode is on or not.
        store.apply(Event.Private(true))
        assertEquals(PrivateLine.Always, store.privacy.line(show))
        store.apply(Event.ShowSettings(ShowSettingsView(id = 900001u, rows = emptyList(), remembered = emptyList(), source = null, alwaysPrivate = false)))
        assertEquals(PrivateLine.Session, store.privacy.line(show))
    }

    @Test
    fun always_private_goes_to_the_engine_and_shows_at_once() {
        store.setAlwaysPrivate(show, true)
        assertTrue(store.privacy.isAlways(show))
        store.apply(Event.AlwaysPrivate(show, false))
        assertFalse(store.privacy.isAlways(show))
        assertEquals(listOf(Command.SetAlwaysPrivate(show, true)), store.waiting)
    }

    @Test
    fun a_place_kept_in_memory_says_so_while_the_mode_is_on() {
        store.apply(Event.Private(true))
        store.apply(progress(900001u, PrivateMode.SESSION, kept = true))
        assertEquals(PrivateLine.Kept, store.privacy.line(show))
        assertTrue(store.privacy.holds(show))
        // Off, the line goes, but the kept place still keeps the show off the system's surfaces.
        store.apply(Event.Private(false))
        assertNull(store.privacy.line(show))
        assertTrue(store.privacy.holds(show))
        // Another show is not held.
        assertFalse(store.privacy.holds(PrivateTitle.Show(900002u)))
    }

    @Test
    fun progress_carries_the_shows_own_flag() {
        store.apply(progress(900001u, PrivateMode.ALWAYS))
        assertTrue(store.privacy.isAlways(show))
        store.apply(progress(900001u, null))
        assertFalse(store.privacy.isAlways(show))
        assertNull(store.privacy.line(show))
    }

    @Test
    fun the_line_follows_the_board() {
        assertNull(PrivacyModel.line(on = false, always = false, kept = false))
        assertNull(PrivacyModel.line(on = false, always = false, kept = true))
        assertEquals(PrivateLine.Session, PrivacyModel.line(on = true, always = false, kept = false))
        assertEquals(PrivateLine.Kept, PrivacyModel.line(on = true, always = false, kept = true))
        assertEquals(PrivateLine.Always, PrivacyModel.line(on = false, always = true, kept = false))
        assertEquals(PrivateLine.Always, PrivacyModel.line(on = true, always = true, kept = true))
    }

    private fun row(id: Long, title: String, episode: String, state: DownloadState, hidden: Boolean) = DownloadRow(
        id = id, anilistId = 900000u + id.toUInt(), title = title, episode = episode, state = state,
        fraction = 0.3, downloaded = 300_000_000u, total = 1_000_000_000u, error = null, path = null, private = hidden,
    )

    @Test
    fun a_private_download_names_nothing_in_its_notification() {
        val one = LiveDownloads.of(context, listOf(row(1, "Stardust Wayfarer", "8", DownloadState.ACTIVE, hidden = true)), mapOf(1L to 2_400_000.0))!!
        assertEquals("Downloading in Kaeri", one.title)
        assertEquals("5 min left", one.text)
        assertFalse("Stardust Wayfarer" in one.title + one.text)
        // With others in the queue, none is named either, and the bar stays.
        val several = LiveDownloads.of(
            context,
            listOf(
                row(1, "Summer Wind Relay", "3", DownloadState.ACTIVE, hidden = false),
                row(2, "Stardust Wayfarer", "8", DownloadState.QUEUED, hidden = true),
            ),
            emptyMap(),
        )!!
        assertEquals("Downloading in Kaeri", several.title)
        assertEquals("1.4 GB left", several.text)
        assertTrue(several.several)
        assertEquals(2, several.segments.size)
    }
}

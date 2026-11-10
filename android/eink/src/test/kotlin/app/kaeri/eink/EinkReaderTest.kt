// The e-ink reader on made-up grey pages: the zone guide over the first chapter, turns at once by
// the 40% forward zone and the upper page button, the folio in the margin, the bars on a middle
// tap, the page between chapters in the flow, a full refresh asked for each image page, and the
// reader's own settings and Contents, which Back closes before it closes the reader.

package app.kaeri.eink

import android.content.ComponentName
import androidx.activity.ComponentActivity
import androidx.compose.runtime.collectAsState
import androidx.compose.ui.test.click
import androidx.compose.ui.test.hasText
import androidx.compose.ui.input.key.Key
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.v2.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performKeyInput
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.pressKey
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.reader.phone.ReaderLook
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.Refresh
import app.kaeri.engine.model.TurnHow
import app.kaeri.reader.PageImages
import app.kaeri.reader.ReaderSession
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.ExternalResource
import org.junit.rules.RuleChain
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.annotation.GraphicsMode

@RunWith(RobolectricTestRunner::class)
@GraphicsMode(GraphicsMode.Mode.NATIVE)
@Config(sdk = [36], qualifiers = "w572dp-h772dp-port-xhdpi")
class EinkReaderTest {
    private val compose = createAndroidComposeRule<ComponentActivity>()
    private val dir = TemporaryFolder()

    @get:Rule
    val rules: RuleChain = RuleChain.outerRule(dir).around(
        object : ExternalResource() {
            override fun before() {
                val app = RuntimeEnvironment.getApplication()
                shadowOf(app.packageManager).addActivityIfNotPresent(ComponentName(app, ComponentActivity::class.java))
            }
        },
    ).around(compose)

    private val sent = mutableListOf<ReaderCommand>()
    private val session = ReaderSession { (it as? Command.Reader)?.let { reader -> sent += reader.v1 } }
    private val fake = FakeRefresh()

    private fun open(onBack: () -> Unit = {}) {
        session.open(EinkFixtures.WORK, EinkFixtures.surface.copy(eink = true))
        session.apply(ReaderEvent.Opened(EinkFixtures.view(start = 0u)))
        session.apply(ReaderEvent.Leaves(EinkFixtures.SESSION, EinkFixtures.CHAPTER, EinkFixtures.leaves(3) { EinkFixtures.grey(dir.root, "$it.gray", 80, 80) }))
        val painter = EinkPainter.defaults(app.kaeri.engine.model.EinkPanel.GREY, fake)
        val actions = EinkReaderActions(onBack, {}, {})
        val images = PageImages()
        compose.setContent {
            EinkTheme {
                val state = session.state.collectAsState().value
                EinkReader(session, state, images, painter, actions, showZones = true)
            }
        }
        compose.waitForIdle()
    }

    private fun shown(): List<ReaderCommand.Shown> = sent.filterIsInstance<ReaderCommand.Shown>()

    @Test
    fun the_guide_shows_until_the_first_touch_which_still_turns() {
        open()
        compose.onNodeWithText("Tap, or swipe right").assertIsDisplayed()
        assertEquals(ReaderCommand.Shown(EinkFixtures.SESSION, EinkFixtures.CHAPTER, 0u, TurnHow.PLACED), shown().first())
        // Right to left: forward is the left 40%.
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.2f, height * 0.5f)) }
        compose.waitForIdle()
        assertTrue(compose.onAllNodes(hasText("Tap, or swipe right")).fetchSemanticsNodes().isEmpty())
        assertEquals(1u, shown().last().leaf)
        compose.onNodeWithText("2 of 3").assertIsDisplayed()
    }

    @Test
    fun the_lower_volume_key_turns_forward_and_each_image_page_is_a_full_refresh() {
        open()
        // Down goes forward, as on the phone, until the phone's swap says otherwise.
        compose.onRoot().performKeyInput { pressKey(Key.VolumeDown) }
        compose.waitForIdle()
        assertEquals(TurnHow.KEY, shown().last().how)
        assertEquals(1u, shown().last().leaf)
        // Pages decode off the main thread: the refresh follows once the page is drawn.
        compose.waitUntil(5_000) { fake.requests.count { it.refresh == Refresh.FULL } >= 2 }
    }

    @Test
    fun a_middle_tap_shows_the_bars() {
        open()
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
        compose.onNodeWithText("Page 1 of 3").assertIsDisplayed()
        compose.onNodeWithText("2 pages left in chapter").assertIsDisplayed()
        compose.onNodeWithText("Chapter 12 · The Long Stair").assertIsDisplayed()
        assertEquals(Refresh.FAST, fake.requests.last().refresh)
    }

    @Test
    fun the_page_between_chapters_is_in_the_flow() {
        open()
        repeat(3) {
            compose.onRoot().performKeyInput { pressKey(Key.VolumeDown) }
            compose.waitForIdle()
        }
        assertEquals(3u, shown().last().leaf)
        compose.onNodeWithText("Continue to chapter 13").assertIsDisplayed()
        compose.onNodeWithText("Turn the page to continue").assertIsDisplayed()
        assertEquals(Refresh.FULL, fake.requests.last().refresh)

        // As it says: the next turn opens the next chapter.
        compose.onRoot().performKeyInput { pressKey(Key.PageDown) }
        compose.waitForIdle()
        assertEquals(listOf(ReaderCommand.OpenChapter(EinkFixtures.SESSION, "local:lantern-stair/13")), sent.filterIsInstance<ReaderCommand.OpenChapter>())
    }

    @Test
    fun the_phones_volume_swap_turns_the_e_ink_pages_too() {
        val context = RuntimeEnvironment.getApplication()
        ReaderLook.swapVolume(context, true)
        try {
            open()
            compose.onRoot().performKeyInput { pressKey(Key.VolumeUp) }
            compose.waitForIdle()
            assertEquals(1u, shown().last().leaf)
        } finally {
            ReaderLook.swapVolume(context, false)
        }
    }

    @Test
    fun the_keys_turn_the_pages_of_contents_and_not_the_page_under_it() {
        open()
        compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
        compose.waitForIdle()
        compose.onNodeWithText("Contents").performClick()
        val rows = List(60) { EinkFixtures.chapter(id = "local:lantern-stair/${it + 1}", number = "${it + 1}", title = null) }
        session.apply(Event.Reading(ReadingEvent.Chapters(EinkFixtures.WORK, null, listOf(VolumeGroup(null, null, rows, false)), null)))
        compose.waitForIdle()
        fun listPage(n: Int) = compose.onAllNodes(hasText("Page $n of", substring = true)).fetchSemanticsNodes().isNotEmpty()
        // Contents opens on the page with the chapter being read, the 12th: the second.
        assertTrue(listPage(2))
        compose.onRoot().performKeyInput { pressKey(Key.VolumeDown) }
        compose.waitForIdle()
        assertTrue(listPage(3))
        assertTrue(!listPage(2))
        compose.onRoot().performKeyInput { pressKey(Key.PageUp) }
        compose.waitForIdle()
        assertTrue(listPage(2))
        // The reader's page stayed where it was.
        assertEquals(0u, shown().last().leaf)
    }

    @Test
    fun settings_and_contents_are_the_readers_own_and_back_closes_them_first() {
        var closed = 0
        open(onBack = { closed++ })
        fun middle() {
            compose.onRoot().performTouchInput { click(androidx.compose.ui.geometry.Offset(width * 0.55f, height * 0.5f)) }
            compose.waitForIdle()
        }
        fun back() {
            compose.runOnUiThread { compose.activity.onBackPressedDispatcher.onBackPressed() }
            compose.waitForIdle()
        }
        middle()
        compose.onNodeWithText("Settings").performClick()
        compose.waitForIdle()
        compose.onNodeWithText("Reader settings").assertIsDisplayed()
        assertEquals(Refresh.REGION, fake.requests.last().refresh)
        back()
        assertTrue(compose.onAllNodes(hasText("Reader settings")).fetchSemanticsNodes().isEmpty())
        assertEquals(0, closed)

        middle()
        compose.onNodeWithText("Contents").performClick()
        compose.waitForIdle()
        assertEquals(Refresh.FAST, fake.requests.last().refresh)
        back()
        assertTrue(compose.onAllNodes(hasText("Contents")).fetchSemanticsNodes().isEmpty())
        assertEquals(Refresh.FULL, fake.requests.last().refresh)
        assertEquals(0, closed)

        // With nothing over the page, Back closes the reader.
        back()
        assertEquals(1, closed)
    }
}

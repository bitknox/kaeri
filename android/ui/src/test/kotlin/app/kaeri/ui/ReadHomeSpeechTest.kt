package app.kaeri.ui

import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasContentDescription
import app.kaeri.store.FixtureScene
import org.junit.Assert.assertTrue
import org.junit.Test
import org.robolectric.annotation.Config

/** TalkBack on Read home: the spotlight's art, which opens the series, says which series it is. */
class ReadHomeSpeechTest : ReadingShotHarness() {
    @Test
    @Config(qualifiers = ReadingShotHarness.PHONE)
    fun the_spotlight_art_is_named_by_its_series() {
        var title = ""
        shoot("phone-read-home-speech", FixtureScene.READING_HOME, Destination.Read) { store ->
            val spotlight = store.reading.home.value?.spotlight
            if (spotlight != null && store.reading.home.value?.spot != null) title = spotlight.title
            title.isNotEmpty()
        }
        val named = compose.onAllNodes(hasClickAction() and hasContentDescription(title)).fetchSemanticsNodes()
        assertTrue("no clickable node is named $title", named.isNotEmpty())
    }
}

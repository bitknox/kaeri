// The paginator's messages, as paginator.js posts them, read into the host's calls.

package app.kaeri.reader

import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.NovelPos
import app.kaeri.engine.model.TurnHow
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class NovelMessageTest {
    @Test
    fun a_turn_carries_its_place_and_how() {
        val text = """{"type":"page","spine":2,"page":4,"pages":11,"pos":{"spine":2,"path":[0,4,1],"offset":120},"at":"2/0.4.1/120","how":"swipe","last":false}"""
        assertEquals(
            NovelMessage.Page(2, 4, 11, NovelPos(2u, listOf(0u, 4u, 1u), 120u), TurnHow.SWIPE, false),
            NovelMessage.parse(text),
        )
    }

    @Test
    fun the_first_layout_has_no_how() {
        val ready = NovelMessage.parse("""{"type":"ready","spine":0,"page":0,"pages":3,"pos":{"spine":0,"path":[],"offset":0},"vertical":false,"failed":null}""")
        assertEquals(NovelMessage.Page(0, 0, 3, NovelPos(0u, emptyList(), 0u), null, false), ready)
    }

    @Test
    fun the_other_messages() {
        assertEquals(NovelMessage.End(3), NovelMessage.parse("""{"type":"end","spine":3}"""))
        assertEquals(NovelMessage.Start(1), NovelMessage.parse("""{"type":"start","spine":1}"""))
        assertEquals(NovelMessage.Tap(0.25f, 0.5f), NovelMessage.parse("""{"type":"tap","x":0.25,"y":0.5}"""))
        assertEquals(NovelMessage.Link("#note-3", true), NovelMessage.parse("""{"type":"link","href":"#note-3","internal":true}"""))
        assertEquals(NovelMessage.Unselected, NovelMessage.parse("""{"type":"unselected"}"""))
        assertNull(NovelMessage.parse("""{"type":"later"}"""))
        assertNull(NovelMessage.parse("not json"))
    }

    @Test
    fun places_write_as_the_store_keeps_them() {
        assertEquals("3/0.4.1/120", NovelPos(3u, listOf(0u, 4u, 1u), 120u).text())
        assertEquals("0//0", NovelPos(0u, emptyList(), 0u).text())
        assertEquals("swipe", NovelMessage.word(TurnHow.SWIPE))
    }

    @Test
    fun the_page_is_told_once_which_side_taps_turn() {
        val host = NovelWebHost(RuntimeEnvironment.getApplication(), NovelAssets("", "", ""), { null }) {}
        val web = shadowOf(host.view)
        host.tapsTurn(1f / 3f, rtl = false)
        val sent = web.lastEvaluatedJavascript
        assertEquals(
            """window.kaeriReader && window.kaeriReader.input({edge: 0.33333334, forward: "right", keys: false, hold: false})""",
            sent,
        )
        host.view.evaluateJavascript("0", null)
        host.tapsTurn(1f / 3f, rtl = false)
        assertEquals("the same again is not sent", "0", web.lastEvaluatedJavascript)
        host.tapsTurn(1f / 3f, rtl = true)
        assertEquals(true, web.lastEvaluatedJavascript.contains("forward: \"left\""))
        host.destroy()
    }
}

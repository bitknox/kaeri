// The detection bridge: the facts go out as DeviceFacts, the engine's profile comes back, and the
// app asks only what the engine could not tell.

package app.kaeri.eink

import app.kaeri.engine.model.Command
import app.kaeri.engine.model.EinkMatch
import app.kaeri.engine.model.EinkProfile
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.NovelSupport
import app.kaeri.engine.model.EinkPanel
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.VendorFamily
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class EinkModelTest {
    private val sent = mutableListOf<ReadingCommand>()
    private val model = EinkModel { sent += (it as Command.Reading).v1 }

    private fun profile(eink: EinkMatch, panel: EinkPanel, colourSure: Boolean = true) = EinkProfile(
        eink = eink, panel = panel, ppi = 300u, family = VendorFamily.OTHER, refreshHook = false,
        pageButtons = false, colourSure = colourSure, novels = NovelSupport.FULL, policy = null,
    )

    private fun hear(profile: EinkProfile) = model.apply(Event.Reading(ReadingEvent.Eink(profile)))

    @Test
    fun the_facts_go_out_before_anything_is_known() {
        model.sendFacts(RuntimeEnvironment.getApplication())
        val facts = (sent.single() as ReadingCommand.DeviceFacts).facts
        assertEquals(36u, facts.sdk)
        assertTrue(facts.widthPx > 0u)
        assertTrue(facts.hooks.isEmpty())
        assertFalse(facts.wideColourGamut)
        assertNull(model.state.value.profile)
        assertNull(model.state.value.question)
    }

    @Test
    fun the_fixtures_can_say_the_panel_shows_colour() {
        model.sendFacts(RuntimeEnvironment.getApplication(), wideColourGamut = true)
        assertTrue((sent.single() as ReadingCommand.DeviceFacts).facts.wideColourGamut)
    }

    @Test
    fun an_unsure_match_asks_if_this_is_eink() {
        hear(profile(EinkMatch.UNSURE, EinkPanel.GREY))
        assertEquals(EinkQuestion.EINK, model.state.value.question)
        model.answerEink(true)
        assertEquals(ReadingCommand.AnswerEink(true, null), sent.single())
    }

    @Test
    fun a_sure_match_with_an_unsure_colour_asks_the_colour_and_keeps_colour() {
        hear(profile(EinkMatch.SURE, EinkPanel.UNKNOWN_COLOUR, colourSure = false))
        val state = model.state.value
        assertEquals(EinkQuestion.COLOUR, state.question)
        assertTrue(state.eink)
        assertTrue(state.colour)
        assertFalse(state.amber)
        model.answerColour(false)
        assertEquals(ReadingCommand.AnswerEink(true, false), sent.single())
        // The answer goes with a later e-ink answer, so the engine keeps it.
        model.answerEink(true)
        assertEquals(ReadingCommand.AnswerEink(true, false), sent.last())
    }

    @Test
    fun skipping_the_colour_question_keeps_pictures_in_colour() {
        hear(profile(EinkMatch.SURE, EinkPanel.UNKNOWN_COLOUR, colourSure = false))
        model.skipColour()
        assertNull(model.state.value.question)
        assertTrue(model.state.value.colour)
        assertTrue(sent.isEmpty())
    }

    @Test
    fun a_sure_kaleido_is_amber_and_a_phone_asks_nothing() {
        hear(profile(EinkMatch.SURE, EinkPanel.KALEIDO3))
        assertTrue(model.state.value.amber)
        assertNull(model.state.value.question)
        hear(profile(EinkMatch.NO, EinkPanel.LCD))
        assertFalse(model.state.value.eink)
        assertFalse(model.state.value.amber)
        assertNull(model.state.value.question)
    }

    @Test
    fun reading_off_leaves_the_profile_unknown() {
        model.apply(Event.Reading(ReadingEvent.Failed(app.kaeri.engine.model.ReadingAbout.EINK, app.kaeri.engine.model.ReadingFailure.ReadingOff)))
        assertNull(model.state.value.profile)
    }
}

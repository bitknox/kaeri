// The e-ink profile as the app holds it: the facts go to the engine before the first screen, the
// engine answers with a profile, and when it is unsure the app asks the person. An unsure e-ink
// match asks "Is this an e-ink screen?" first; a sure one with an unsure colour asks the colour
// question. Until a colour answer, pictures stay in colour.

package app.kaeri.eink

import android.content.Context
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.EinkMatch
import app.kaeri.engine.model.EinkProfile
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Panel
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ScreenFacts
import app.kaeri.store.AppStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** The question the app has to ask about its screen, if any. */
enum class EinkQuestion { EINK, COLOUR }

data class EinkState(
    /** Null until the engine answers, and on a build with reading off. */
    val profile: EinkProfile? = null,
    /** The colour question was skipped on this run. */
    val colourSkipped: Boolean = false,
) {
    /** The e-ink layout is the one to show. */
    val eink: Boolean get() = profile?.eink == EinkMatch.SURE

    /** The panel shows colour, or might: pictures stay in colour until a grey answer. */
    val colour: Boolean get() = profile?.panel?.let { it != Panel.GREY } ?: false

    /** Amber for the one primary action, only once the colour is sure. */
    val amber: Boolean get() = eink && colour && profile?.colourSure == true && profile.panel != Panel.LCD

    val question: EinkQuestion?
        get() {
            val p = profile ?: return null
            return when {
                p.eink == EinkMatch.UNSURE -> EinkQuestion.EINK
                p.eink == EinkMatch.SURE && !p.colourSure && !colourSkipped -> EinkQuestion.COLOUR
                else -> null
            }
        }
}

class EinkModel(private val send: (Command) -> Unit) {
    private val _state = MutableStateFlow(EinkState())
    val state: StateFlow<EinkState> = _state.asStateFlow()

    private var colourAnswer: Boolean? = null

    /** Hear the profile from the store until what is returned is called. */
    fun bind(store: AppStore): () -> Unit = store.listenToReading(::apply)

    /** Tell the engine about this device: before the first screen. */
    fun sendFacts(facts: ScreenFacts) = send(Command.Reading(ReadingCommand.DeviceFacts(facts)))

    fun sendFacts(context: Context, wideColourGamut: Boolean? = null) = sendFacts(ScreenFactsReader.read(context, wideColourGamut))

    /** "Is this an e-ink screen?": Use the e-ink layout, or Keep the standard layout. */
    fun answerEink(eink: Boolean) = send(Command.Reading(ReadingCommand.AnswerEink(eink, colourAnswer)))

    /**
     * "Does this screen show colour?". The question only shows on a sure e-ink match, and the
     * engine writes both answers at once, so the e-ink one goes with it.
     */
    fun answerColour(colour: Boolean) {
        colourAnswer = colour
        send(Command.Reading(ReadingCommand.AnswerEink(true, colour)))
    }

    /** Skip: pictures stay in colour, and the question waits for another run. */
    fun skipColour() {
        _state.value = _state.value.copy(colourSkipped = true)
    }

    fun apply(event: Event) {
        val reading = (event as? Event.Reading)?.v1 ?: return
        if (reading is ReadingEvent.Eink) _state.value = _state.value.copy(profile = reading.v1)
    }
}

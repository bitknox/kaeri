// The e-ink profile as the app holds it: the facts go to the engine before the first screen, the
// engine answers with a profile, and when it is unsure the app asks the person. An unsure e-ink
// match asks "Is this an e-ink screen?" first; a sure one with an unsure colour asks the colour
// question. Until a colour answer, pictures stay in colour. The last answer is kept on the device,
// so a cold start draws paper from its first frame while the engine starts.

package app.kaeri.eink

import android.content.Context
import android.content.SharedPreferences
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.EinkMatch
import app.kaeri.engine.model.EinkProfile
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.EinkPanel
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ScreenFacts
import app.kaeri.store.AppStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** The question the app has to ask about its screen, if any. */
enum class EinkQuestion { EINK, COLOUR }

/** What the engine's last profile said about the screen, kept for the next cold start. */
data class EinkAnswer(val eink: Boolean, val amber: Boolean)

/** Where the last [EinkAnswer] is kept between runs. */
interface EinkAnswerStore {
    fun load(): EinkAnswer?
    fun save(answer: EinkAnswer)
}

/** The last answer in the app's own preferences: one small file, read before the first frame. */
class EinkAnswerPrefs(private val prefs: SharedPreferences) : EinkAnswerStore {
    constructor(context: Context) : this(context.getSharedPreferences(FILE, Context.MODE_PRIVATE))

    override fun load(): EinkAnswer? =
        if (prefs.contains(EINK)) EinkAnswer(prefs.getBoolean(EINK, false), prefs.getBoolean(AMBER, false)) else null

    override fun save(answer: EinkAnswer) {
        prefs.edit().putBoolean(EINK, answer.eink).putBoolean(AMBER, answer.amber).apply()
    }

    private companion object {
        const val FILE = "eink"
        const val EINK = "eink"
        const val AMBER = "amber"
    }
}

data class EinkState(
    /** Null until the engine answers, and on a build with reading off. */
    val profile: EinkProfile? = null,
    /** The colour question was skipped on this run. */
    val colourSkipped: Boolean = false,
    /** The last run's answer, which stands in until the engine answers on this one. */
    val saved: EinkAnswer? = null,
) {
    /** The e-ink layout is the one to show. */
    val eink: Boolean get() = profile?.let { it.eink == EinkMatch.SURE } ?: (saved?.eink == true)

    /** The panel shows colour, or might: pictures stay in colour until a grey answer. */
    val colour: Boolean get() = profile?.panel?.let { it != EinkPanel.GREY } ?: false

    /** Amber for the one primary action, only once the colour is sure. */
    val amber: Boolean
        get() = if (profile == null) saved?.amber == true
        else eink && colour && profile.colourSure && profile.panel != EinkPanel.LCD

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

class EinkModel(
    /** Where the answer is kept between runs; none keeps nothing. */
    private val answers: EinkAnswerStore? = null,
    private val send: (Command) -> Unit,
) {
    private val _state = MutableStateFlow(EinkState(saved = answers?.load()))
    val state: StateFlow<EinkState> = _state.asStateFlow()

    private var colourAnswer: Boolean? = null

    /** Hear the profile from the store until what is returned is called. */
    fun bind(store: AppStore): () -> Unit = store.listenToReading(::apply)

    /** Tell the engine about this device: before the first screen. */
    fun sendFacts(facts: ScreenFacts) = send(Command.Reading(ReadingCommand.DeviceFacts(facts)))

    fun sendFacts(context: Context, wideColourGamut: Boolean? = null, einkSupport: String? = null) =
        sendFacts(ScreenFactsReader.read(context, wideColourGamut, einkSupport))

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
        if (reading !is ReadingEvent.Eink) return
        val next = _state.value.copy(profile = reading.v1)
        _state.value = next
        val answer = EinkAnswer(next.eink, next.amber)
        if (answer != next.saved) {
            answers?.save(answer)
            _state.value = next.copy(saved = answer)
        }
    }
}

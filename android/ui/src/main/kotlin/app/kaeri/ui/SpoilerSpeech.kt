package app.kaeri.ui

import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.semantics

/**
 * What a screen reader reads for text with spoiler bars. A bar's words sit in the text in a
 * transparent colour, and TalkBack reads text whatever its colour, so each unopened bar is read as
 * [word] instead. Fill it alongside the drawn text: [plain] for what shows, [bar] for a closed spoiler.
 */
internal class SpoilerSpeech(private val word: String) {
    private val spoken = StringBuilder()

    /** How many bars are still closed. */
    var closed = 0
        private set

    fun plain(text: String) {
        spoken.append(text)
    }

    fun bar() {
        spoken.append(word)
        closed++
    }

    val text: String get() = spoken.toString()
}

/**
 * Reads [speech] in place of the text while a bar is closed, with an action that opens every bar
 * ([openLabel], [openAll]); with none closed, the text reads as it is.
 */
internal fun Modifier.spoilerSpeech(speech: SpoilerSpeech, openLabel: String, openAll: () -> Unit): Modifier =
    if (speech.closed == 0) {
        this
    } else {
        val spoken = speech.text
        semantics {
            contentDescription = spoken
            customActions = listOf(CustomAccessibilityAction(openLabel) { openAll(); true })
        }
    }

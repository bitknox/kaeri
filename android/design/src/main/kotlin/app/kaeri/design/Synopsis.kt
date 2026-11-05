// What the phone and TV episode rows share: the language a synopsis is written in, so screen
// readers and line breaking follow the text and not the interface, and the episodes a visit has
// revealed under spoiler protection.

package app.kaeri.design

import android.content.Context
import android.view.textclassifier.TextClassificationManager
import android.view.textclassifier.TextLanguage
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.produceState
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.intl.LocaleList
import androidx.compose.ui.text.withStyle
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/** The language a synopsis is written in, found by the system's text classifier. */
object SynopsisLanguage {
    private val tags = object : LinkedHashMap<String, String?>(64, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, String?>?): Boolean = size > 256
    }

    /** [text]'s language tag, or null when the classifier is unsure or unavailable. */
    suspend fun tag(context: Context, text: String): String? {
        synchronized(tags) { if (tags.containsKey(text)) return tags[text] }
        val found = withContext(Dispatchers.Default) { detect(context, text) }
        synchronized(tags) { tags[text] = found }
        return found
    }

    private fun detect(context: Context, text: String): String? = runCatching {
        val manager = context.getSystemService(TextClassificationManager::class.java) ?: return@runCatching null
        val language = manager.textClassifier.detectLanguage(TextLanguage.Request.Builder(text).build())
        if (language.localeHypothesisCount == 0) return@runCatching null
        val locale = language.getLocale(0)
        locale.toLanguageTag().takeIf { language.getConfidenceScore(locale) >= CONFIDENT }
    }.getOrNull()

    private const val CONFIDENT = 0.7f
}

/** [text] tagged with [tag] as a span, or plain without one. */
fun tagged(text: String, tag: String?): AnnotatedString =
    if (tag == null) AnnotatedString(text)
    else buildAnnotatedString { withStyle(SpanStyle(localeList = LocaleList(tag))) { append(text) } }

/** [text] with its language found, as it is, until the classifier has answered. */
@Composable
fun rememberTagged(text: String): AnnotatedString {
    val context = LocalContext.current
    val tag by produceState<String?>(null, text) { value = SynopsisLanguage.tag(context, text) }
    return remember(text, tag) { tagged(text, tag) }
}

/**
 * The episodes a visit has revealed, one at a time or all after the protection line's Show, until
 * its Hide covers them again.
 */
@Stable
class Reveals {
    private val shown = mutableStateListOf<String>()
    private var all by mutableStateOf(false)

    fun isShown(number: String): Boolean = all || number in shown

    val everything: Boolean get() = all

    fun show(number: String) {
        if (number !in shown) shown += number
    }

    fun showAll() {
        all = true
    }

    /** The protection line's Hide: every row after it is covered again, one revealed alone too. */
    fun hideAll() {
        all = false
        shown.clear()
    }
}

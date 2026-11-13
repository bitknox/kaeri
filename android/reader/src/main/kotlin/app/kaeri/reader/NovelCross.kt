// Where a novel's web view goes between the book's documents, and which of its pages keep a
// place. Only the book's own documents keep one: a document of the book outside the reading order
// (its contents page, a note) shows and keeps none, and a page that is not the book's, such as an
// error's empty answer, keeps none and goes back once to the last place the book showed.

package app.kaeri.reader

import app.kaeri.engine.model.NovelPos
import app.kaeri.engine.model.NovelSpineItem
import java.io.ByteArrayOutputStream

/**
 * One open book's crossings. [root] is the start every address of the open session's book has;
 * [opening] is where the chapter opened, which a page with no place shown yet goes back to.
 */
class NovelCross(
    private val spine: List<NovelSpineItem>,
    private val root: String,
    val opening: String,
) {
    /** What a page the paginator laid out is. */
    enum class Shown {
        /** A document of the reading order, at the address shown: its place is kept. */
        BOOK,

        /** A document of the book outside the reading order: it shows and keeps no place. */
        OUTSIDE,

        /** Not the book's document at the address shown: it keeps no place. */
        NOT_SERVED,
    }

    /** The last place the book showed, to go back to. */
    var lastPlace: Pair<Int, NovelPos>? = null
        private set

    /** Going back to a known place after a page that was not the book's, until the book shows. */
    private var recovering = false

    /** The spine document at [address], whatever its fragment and however its escapes are written. */
    fun index(address: String?): Int? {
        if (address == null) return null
        val path = document(address)
        return spine.indexOfFirst { document(it.url) == path }.takeIf { it >= 0 }
    }

    /** The address of spine document [index]. */
    fun url(index: Int): String? = spine.getOrNull(index)?.url

    /** Whether [address] names a file of the open book. */
    fun inBook(address: String?): Boolean = address != null && document(address).startsWith(document(root))

    /** A page laid out at [address] that says it is spine document [spine], at [pos]. */
    fun shown(address: String?, spine: Int, pos: NovelPos?): Shown {
        val index = index(address)
        return when {
            index != null && index == spine -> {
                if (pos != null) lastPlace = index to pos
                recovering = false
                Shown.BOOK
            }
            index == null && inBook(address) -> {
                recovering = false
                Shown.OUTSIDE
            }
            else -> Shown.NOT_SERVED
        }
    }

    /** A turn on a page already shown: whether its place is the book's to keep. */
    fun keeps(address: String?, spine: Int, pos: NovelPos?): Boolean {
        val index = index(address) ?: return false
        if (index != spine) return false
        if (pos != null) lastPlace = index to pos
        return true
    }

    /** Where a page that was not the book's goes, once: the last place shown, or the opening. */
    fun recover(): String? {
        if (recovering) return null
        recovering = true
        return back()
    }

    /** The last place the book showed, or where the chapter opened. */
    fun back(): String {
        val (index, pos) = lastPlace ?: return opening
        return spine.getOrNull(index)?.let { "${it.url}#kaeri:pos=${pos.text()}" } ?: opening
    }

    /** A load that never came. With a page of the book still shown it stays; with none, once back. */
    fun failed(shown: String?): String? =
        if (index(shown) != null || inBook(shown)) null else recover()

    /** What a turn past the end or the start of a document does. */
    sealed interface Step {
        data class Load(val url: String) : Step

        /** No document further that way: the end page past the last, nothing before the first. */
        data object Edge : Step

        /** No document of the book known: back to the last place, or the opening. */
        data class Back(val url: String) : Step
    }

    /** Past the last page of the document at [shown]: the next linear document, from the last place when the page is outside the reading order. */
    fun next(shown: String?): Step = step(shown, +1)

    /** Before the first page: the previous linear document, at its end. */
    fun previous(shown: String?): Step = step(shown, -1)

    private fun step(shown: String?, by: Int): Step {
        val current = index(shown) ?: lastPlace?.first ?: return Step.Back(back())
        var i = current + by
        while (i in spine.indices) {
            if (spine[i].linear) return Step.Load(if (by < 0) "${spine[i].url}#kaeri:end" else spine[i].url)
            i += by
        }
        return Step.Edge
    }

    /** A link the paginator reported: followed when it names a file of the book. */
    fun link(href: String, internal: Boolean): String? = href.takeIf { internal && inBook(it) }

    companion object {
        /** The document an address names: without its fragment and with its escapes read. */
        fun document(address: String): String = decode(address.substringBefore('#'))

        private fun decode(text: String): String {
            if ('%' !in text) return text
            val out = ByteArrayOutputStream()
            var i = 0
            while (i < text.length) {
                val c = text[i]
                val hex = c == '%' && i + 2 < text.length && text[i + 1].isHex() && text[i + 2].isHex()
                if (hex) {
                    out.write(text.substring(i + 1, i + 3).toInt(16))
                    i += 3
                } else {
                    out.write(c.toString().toByteArray(Charsets.UTF_8))
                    i += 1
                }
            }
            return out.toString(Charsets.UTF_8)
        }

        private fun Char.isHex(): Boolean = this in '0'..'9' || this in 'a'..'f' || this in 'A'..'F'
    }
}

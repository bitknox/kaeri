// A light novel's turn from one document of the book into another, as the engine serves it, on
// the host's real web view client: only the open session's addresses answer, each document of the
// reading order carries its spine index in its settings, and a document outside the reading order
// (the contents page, the notes) says the first. Made-up book, made-up names.

package app.kaeri.reader

import android.net.Uri
import android.os.Looper
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import app.kaeri.engine.model.BookBody
import app.kaeri.engine.model.BookResource
import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.NovelPos
import app.kaeri.engine.model.NovelSpineItem
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.Shadows.shadowOf
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowWebView

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class NovelCrossTest {
    /**
     * "Paper Lantern Vol 1" as an EPUB the engine serves for one session at a time: three parts in
     * the reading order with spaces (and a comma) in their file names, a contents page and a notes
     * document in the manifest but outside the spine. Any other session's address, a part listed as
     * missing and a file the book does not have answer 404 with an empty body.
     */
    class Book(var session: Int = 1) {
        val parts = listOf(
            "Text/Part 1 of Paper Lantern.xhtml",
            "Text/Part 2 of Paper Lantern.xhtml",
            "Text/Part 3, Harbour Lights.xhtml",
        )
        val outside = listOf("nav.xhtml", "Text/Notes for Part 1.xhtml")
        val missing = mutableSetOf<String>()

        /** Parts served without the reader's settings, which the paginator then reads as its defaults. */
        val bare = mutableSetOf<String>()

        /** The address the engine writes: each segment escaped, the comma kept as it is. */
        fun url(path: String, session: Int = this.session): String =
            NovelWebHost.PREFIX + session + "/" + path.split('/').joinToString("/") { it.replace(" ", "%20") }

        fun spine(session: Int = this.session) = parts.map { NovelSpineItem(url = url(it, session), linear = true, words = 0u) }

        fun resource(address: String): BookResource {
            val path = Uri.decode(address.substringBefore('#'))
            val prefix = NovelWebHost.PREFIX + session + "/"
            val name = path.removePrefix(prefix).takeIf { path.startsWith(prefix) }
            if (name == null || name in missing || (name !in parts && name !in outside)) {
                return BookResource(status = 404u, mime = "text/plain", headers = emptyList(), body = BookBody.Bytes(ByteArray(0)))
            }
            val spine = parts.indexOf(name).coerceAtLeast(0)
            val setup = if (name in bare) "" else """<meta name="kaeri-setup" content="{&quot;spine&quot;:$spine}"/>"""
            val text = """<html xmlns="http://www.w3.org/1999/xhtml"><head>$setup<title>$name</title></head>""" +
                "<body><p>The ferry keeper counted the lanterns along the harbour wall.</p></body></html>"
            return BookResource(status = 200u, mime = "application/xhtml+xml", headers = emptyList(), body = BookBody.Bytes(text.toByteArray()))
        }
    }

    /** The host, with the web view's part played here: each load is asked of the host's own client, and what came is laid out. */
    private class Reader(val book: Book) {
        val forwarded = mutableListOf<NovelMessage>()
        val host = NovelWebHost(RuntimeEnvironment.getApplication(), NovelAssets("", "", ""), { book.resource(it) }) { forwarded += it }
        val web: ShadowWebView = shadowOf(host.view)
        val loads = mutableListOf<String>()

        /** The places kept: what the screen sends the engine as NovelAt. */
        val places get() = forwarded.filterIsInstance<NovelMessage.Page>()

        fun open(session: Int) {
            book.session = session
            host.open(book.spine(session), book.url(book.parts[0], session), null)
            settle()
        }

        /** The paginator says [json]. */
        fun say(json: String) {
            val bridge = web.getJavascriptInterface(NovelWebHost.HOST) ?: error("the host has no bridge for the paginator")
            bridge.javaClass.getDeclaredMethod("postMessage", String::class.java).apply { isAccessible = true }.invoke(bridge, json)
            idle()
        }

        fun end() {
            say("""{"type":"end","spine":0}""")
            settle()
        }

        fun start() {
            say("""{"type":"start","spine":0}""")
            settle()
        }

        fun link(href: String, internal: Boolean = true) {
            say("""{"type":"link","href":"$href","internal":$internal}""")
            settle()
        }

        /** A turn inside the document shown, to its page [page], at a place of its own. */
        fun turn(spine: Int, page: Int) =
            say("""{"type":"page","spine":$spine,"page":$page,"pages":12,"pos":{"spine":$spine,"path":[$page],"offset":7},"how":"key","last":false}""")

        /** The answers the host's client gave the main frame, by address. */
        val answers = mutableListOf<Pair<String, Int>>()

        /**
         * Every load the host asked for since the last: its main document asked of the host's
         * client, as the web view would. A 204 leaves the page as it was; anything else is laid out,
         * and the paginator on an empty answer reads its defaults, spine 0 on one page.
         */
        fun settle() {
            repeat(4) {
                val url = web.lastLoadedUrl ?: return
                lastUrl.set(web, null)
                loads += url
                val answer = ask(url)
                answers += url to answer.statusCode
                idle()
                if (answer.statusCode == 204) return@repeat
                val body = answer.data.readBytes().decodeToString()
                val spine = Regex("&quot;spine&quot;:(\\d+)").find(body)?.groupValues?.get(1)?.toInt() ?: 0
                val pages = if (body.isEmpty()) 1 else 12
                val page = if (url.endsWith("#kaeri:end")) pages - 1 else 0
                say("""{"type":"ready","spine":$spine,"page":$page,"pages":$pages,"pos":{"spine":$spine,"path":[$page],"offset":0},"vertical":false,"failed":null}""")
            }
        }

        fun ask(url: String): WebResourceResponse {
            val request = object : WebResourceRequest {
                override fun getUrl(): Uri = Uri.parse(url)
                override fun isForMainFrame() = true
                override fun isRedirect() = false
                override fun hasGesture() = false
                override fun getMethod() = "GET"
                override fun getRequestHeaders(): Map<String, String> = emptyMap()
            }
            return web.webViewClient.shouldInterceptRequest(host.view, request)
        }

        companion object {
            val lastUrl = ShadowWebView::class.java.getDeclaredField("lastUrl").apply { isAccessible = true }
        }
    }

    private fun idle() = shadowOf(Looper.getMainLooper()).idle()

    /** Part 2 does not come: part 1 stays with its keys, no place is kept from the empty answer, and the next key asks for part 2 again rather than a part further on. */
    @Test
    fun a_part_that_does_not_come_leaves_the_page_and_keeps_no_place() {
        val book = Book().apply { missing += parts[1] }
        val reader = Reader(book)
        reader.open(1)
        reader.turn(0, 11)
        val before = reader.places.size
        reader.end()
        assertEquals("part 2 is asked for", book.url(book.parts[1]), reader.loads.last())
        assertEquals("the empty answer never replaces part 1", 204, reader.answers.last().second)
        val after = reader.places.drop(before)
        assertTrue("no place from the empty answer: $after", after.all { it.spine == 0 && it.pages > 1 })
        reader.end()
        assertEquals("the next key asks for part 2 again, from part 1", book.url(book.parts[1]), reader.loads.last())
        reader.host.destroy()
    }

    /** A page that is not the book's at its address keeps no place and goes back to the last place shown, once. */
    @Test
    fun a_page_that_is_not_the_books_keeps_no_place_and_goes_back_once() {
        val book = Book().apply { bare += parts[1] }
        val reader = Reader(book)
        reader.open(1)
        reader.turn(0, 11)
        val before = reader.places.size
        book.missing += book.parts[0]
        reader.end()
        val after = reader.places.drop(before)
        assertTrue("no place from part 2's page without the book's settings: $after", after.isEmpty())
        val back = reader.loads.drop(1)
        assertEquals(
            "back to part 1's last place, once, and no further when that does not come either",
            listOf(book.url(book.parts[1]), book.url(book.parts[0]) + "#kaeri:pos=0/11/7"),
            back.drop(back.indexOf(book.url(book.parts[1]))),
        )
        reader.host.destroy()
    }

    /** The same book opened again in a new session crosses into its own next part, at the new session's address. */
    @Test
    fun a_book_opened_again_crosses_into_its_next_part() {
        val book = Book()
        val reader = Reader(book)
        reader.open(1)
        reader.open(2)
        reader.turn(0, 11)
        reader.end()
        assertEquals(book.url(book.parts[1], session = 2), reader.loads.last())
        assertEquals(1, reader.places.last().spine)
        reader.host.destroy()
    }

    /**
     * Links to the book's documents outside the reading order are followed and keep no place; a
     * turn from them goes on from the last place shown. Escapes written another way still name
     * their part, and links out of the book or to a file it does not have leave the page.
     */
    @Test
    fun documents_outside_the_spine_and_escaped_names_cross_and_keep_no_place() {
        val book = Book()
        val reader = Reader(book)
        reader.open(1)
        reader.turn(0, 5)

        // A note outside the spine: shown, no place kept.
        val note = book.url("Text/Notes for Part 1.xhtml") + "#note-2"
        reader.link(note)
        assertEquals("the note is followed", note, reader.loads.last())
        assertEquals("no place from the note", 0 to 5, reader.places.last().let { it.spine to it.page })

        // Past the note's end: part 2, from the last place shown in part 1.
        reader.end()
        assertEquals(book.url(book.parts[1]), reader.loads.last())
        assertEquals(1, reader.places.last().spine)

        // The contents page outside the spine, then part 3 by a link that escapes its comma.
        reader.link(book.url("nav.xhtml"))
        assertEquals(book.url("nav.xhtml"), reader.loads.last())
        assertEquals("no place from the contents page", 1, reader.places.last().spine)
        val third = NovelWebHost.PREFIX + "1/Text/Part%203%2C%20Harbour%20Lights.xhtml"
        reader.link(third)
        assertEquals(third, reader.loads.last())
        assertEquals("part 3 keeps its place", 2, reader.places.last().spine)

        // Back from part 3's first page: part 2's last page.
        reader.start()
        assertEquals(book.url(book.parts[1]) + "#kaeri:end", reader.loads.last())
        assertEquals(1, reader.places.last().spine)

        // Out of the book, and a file the book does not have: the page stays.
        val loads = reader.loads.size
        reader.link("https://elsewhere.example/", internal = false)
        assertEquals("nothing loads for a link out of the book", loads, reader.loads.size)
        reader.link(book.url("Text/Part 9.xhtml"))
        assertNotEquals("a file the book does not have is refused", 404, reader.answers.last().second)
        assertEquals("and keeps no place", 1, reader.places.last().spine)
        reader.end()
        assertEquals("the keys go on from part 2", book.url(book.parts[2]), reader.loads.last())
        reader.host.destroy()
    }
}

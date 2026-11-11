// The web view novels are read in, shared by the phone reader and the e-reader build. The engine
// serves the book at https://appassets.androidplatform.net/book/<session>/<path>; this host puts
// the paginator in at document start, answers those requests from the engine on the web view's
// own thread, refuses every other load and every navigation, and turns the paginator's messages
// into calls a screen forwards to its ReaderSession.

package app.kaeri.reader

import android.annotation.SuppressLint
import android.content.Context
import android.net.Uri
import android.webkit.JavascriptInterface
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import app.kaeri.engine.model.BookBody
import app.kaeri.engine.model.BookResource
import app.kaeri.engine.model.NovelAssets
import app.kaeri.engine.model.NovelPos
import app.kaeri.engine.model.NovelSpineItem
import app.kaeri.engine.model.TurnHow
import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.File
import java.io.FileInputStream
import java.io.InputStream

/** What the paginator said, read from its JSON. */
sealed interface NovelMessage {
    /** First layout done, or a turn: forward it as NovelAt. [how] is null for the first layout. */
    data class Page(val spine: Int, val page: Int, val pages: Int, val pos: NovelPos?, val how: TurnHow?, val last: Boolean) : NovelMessage

    /** Laid out again with the place kept: nothing to forward. */
    data class Layout(val spine: Int, val page: Int, val pages: Int) : NovelMessage

    /** A turn past the last page of a document. */
    data class End(val spine: Int) : NovelMessage

    /** A turn before the first page of a document. */
    data class Start(val spine: Int) : NovelMessage

    data class Link(val href: String, val internal: Boolean) : NovelMessage

    /** A tap off any link, as fractions of the view. */
    data class Tap(val x: Float, val y: Float) : NovelMessage

    data class Key(val key: String) : NovelMessage

    data class Selection(val text: String) : NovelMessage

    data object Unselected : NovelMessage

    data class Failed(val why: String) : NovelMessage

    companion object {
        /** One message, or null for one this host does not know. */
        fun parse(text: String): NovelMessage? = runCatching {
            val m = JSONObject(text)
            val spine = m.optInt("spine")
            when (m.optString("type")) {
                "ready", "page" -> Page(
                    spine = spine,
                    page = m.optInt("page"),
                    pages = m.optInt("pages"),
                    pos = m.optJSONObject("pos")?.let { p ->
                        val path = p.optJSONArray("path")
                        NovelPos(p.optInt("spine").toUInt(), List(path?.length() ?: 0) { path!!.optInt(it).toUInt() }, p.optInt("offset").toUInt())
                    },
                    how = if (m.optString("type") == "ready") null else turnHow(m.optString("how")),
                    last = m.optBoolean("last"),
                )
                "layout" -> Layout(spine, m.optInt("page"), m.optInt("pages"))
                "end" -> End(spine)
                "start" -> Start(spine)
                "link" -> Link(m.optString("href"), m.optBoolean("internal"))
                "tap" -> Tap(m.optDouble("x").toFloat(), m.optDouble("y").toFloat())
                "key" -> Key(m.optString("key"))
                "selection" -> Selection(m.optString("text"))
                "unselected" -> Unselected
                "failed" -> Failed(m.optString("why"))
                else -> null
            }
        }.getOrNull()

        private fun turnHow(word: String): TurnHow = when (word) {
            "swipe" -> TurnHow.SWIPE
            "key" -> TurnHow.KEY
            "jump" -> TurnHow.JUMP
            "scroll" -> TurnHow.SCROLL
            "placed" -> TurnHow.PLACED
            else -> TurnHow.TURN
        }

        /** The paginator's word for [how]. */
        fun word(how: TurnHow): String = how.name.lowercase()
    }
}

/**
 * One novel's web view. [resource] is the engine's book server (FfiEngine.bookResource), called on
 * the web view's request thread; [assets] is novelAssets(). Messages arrive on the main thread.
 */
@SuppressLint("SetJavaScriptEnabled")
class NovelWebHost(
    context: Context,
    private val assets: NovelAssets,
    private val resource: (String) -> BookResource?,
    private val onMessage: (NovelMessage) -> Unit,
) {
    val view: WebView = WebView(context)
    private var spine: List<NovelSpineItem> = emptyList()
    private var index = 0
    private var scriptByHand = false

    init {
        // A web view left at wrap-content gives vh and percentages no height, and the columns one line.
        view.layoutParams = android.view.ViewGroup.LayoutParams(
            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
        )
        view.settings.apply {
            javaScriptEnabled = true
            allowFileAccess = false
            allowContentAccess = false
            domStorageEnabled = false
            setSupportZoom(false)
            builtInZoomControls = false
            textZoom = 100
            mixedContentMode = android.webkit.WebSettings.MIXED_CONTENT_NEVER_ALLOW
            javaScriptCanOpenWindowsAutomatically = false
            setSupportMultipleWindows(false)
        }
        val origins = setOf(ORIGIN)
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(view, assets.script, origins)
        } else {
            // Older web views: the script goes in once the page is parsed, and lays it out then.
            scriptByHand = true
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(view, HOST, origins) { _, message, source, isMainFrame, _ ->
                if (isMainFrame && source.toString().startsWith(ORIGIN)) message.data?.let(::received)
            }
        } else {
            view.addJavascriptInterface(Bridge(), HOST)
        }
        view.webViewClient = object : WebViewClient() {
            // Nothing navigates: the paginator reports links, and the host decides.
            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean = true

            override fun shouldInterceptRequest(view: WebView, request: WebResourceRequest): WebResourceResponse =
                answer(request.url)

            override fun onPageFinished(view: WebView, url: String) {
                if (scriptByHand && url.startsWith(PREFIX)) view.evaluateJavascript(assets.script, null)
            }
        }
    }

    private inner class Bridge {
        @JavascriptInterface
        fun postMessage(text: String) {
            view.post { received(text) }
        }
    }

    private fun received(text: String) {
        val message = NovelMessage.parse(text) ?: return
        when (message) {
            // The next linear document on a turn past the end, the previous one before the start.
            is NovelMessage.End -> nextDocument(+1)?.let { turnInto(it) } ?: onMessage(message)
            is NovelMessage.Start -> nextDocument(-1)?.let { turnInto(it, "kaeri:end") } ?: onMessage(message)
            // A document turned into is a page turn, not a jump that Back to page would undo.
            is NovelMessage.Page -> {
                val how = turnedInto?.takeIf { message.how == null }
                turnedInto = null
                onMessage(if (how != null) message.copy(how = how) else message)
            }
            else -> onMessage(message)
        }
    }

    /** How the document being loaded was reached, when a turn crossed into it. */
    private var turnedInto: TurnHow? = null

    private fun turnInto(document: Int, fragment: String? = null) {
        turnedInto = TurnHow.TURN
        load(document, fragment)
    }

    private fun nextDocument(step: Int): Int? {
        var i = index + step
        while (i in spine.indices) {
            if (spine[i].linear) return i
            i += step
        }
        return null
    }

    /** Open the book: its spine, and the place to start at (NovelPos text), or its first page. */
    fun open(spine: List<NovelSpineItem>, entry: String, restore: NovelPos?) {
        this.spine = spine
        val at = restore?.spine?.toInt()?.takeIf { it in spine.indices }
        index = at ?: spine.indexOfFirst { it.url == entry }.coerceAtLeast(0)
        val url = spine.getOrNull(index)?.url ?: entry
        view.loadUrl(if (restore != null) "$url#kaeri:pos=${restore.text()}" else url)
    }

    private fun load(document: Int, fragment: String? = null) {
        index = document
        val url = spine[document].url
        view.loadUrl(if (fragment != null) "$url#$fragment" else url)
    }

    /** Go to a contents entry: its document, and the element its fragment names. */
    fun openEntry(spineIndex: Int, url: String) {
        turnedInto = TurnHow.JUMP
        index = spineIndex.coerceIn(0, (spine.size - 1).coerceAtLeast(0))
        view.loadUrl(url)
    }

    fun next(how: TurnHow = TurnHow.TURN) = call("next(${quote(NovelMessage.word(how))})")

    fun prev(how: TurnHow = TurnHow.TURN) = call("prev(${quote(NovelMessage.word(how))})")

    fun goTo(page: Int, how: TurnHow = TurnHow.JUMP) = call("goTo($page, ${quote(NovelMessage.word(how))})")

    fun goToPos(pos: NovelPos, how: TurnHow = TurnHow.JUMP) {
        if (pos.spine.toInt() != index && pos.spine.toInt() in spine.indices) {
            index = pos.spine.toInt()
            view.loadUrl("${spine[index].url}#kaeri:pos=${pos.text()}")
        } else {
            call("goToPos(${quote(pos.text())}, ${quote(NovelMessage.word(how))})")
        }
    }

    /** A live settings change: NovelStyle's style. */
    fun configure(style: String) = call("configure(${quote(style)})")

    fun destroy() {
        view.stopLoading()
        view.destroy()
    }

    private fun call(js: String) = view.evaluateJavascript("window.kaeriReader && window.kaeriReader.$js", null)

    private fun answer(url: Uri): WebResourceResponse {
        val text = url.toString().substringBefore('#')
        if (!text.startsWith(PREFIX)) return refused()
        val found = runCatching { resource(text) }.getOrNull() ?: return notFound()
        val body: InputStream = when (val b = found.body) {
            is BookBody.Bytes -> ByteArrayInputStream(b.data)
            is BookBody.File -> runCatching { FileInputStream(File(b.path)) }.getOrNull() ?: return notFound()
        }
        val headers = found.headers.filterNot { it.name.equals("content-type", ignoreCase = true) }.associate { it.name to it.value }
        val mime = found.mime.substringBefore(';').trim()
        val charset = found.mime.substringAfter("charset=", "").trim().ifEmpty { if (mime.startsWith("text/") || mime.endsWith("xml") || mime.endsWith("javascript")) "utf-8" else "" }
        val status = found.status.toInt()
        return WebResourceResponse(mime, charset.ifEmpty { null }, status, reason(status), headers, body)
    }

    companion object {
        const val ORIGIN = "https://appassets.androidplatform.net"
        const val PREFIX = "$ORIGIN/book/"

        /** The name the paginator posts to. */
        const val HOST = "kaeriHost"

        private fun quote(text: String): String = JSONObject.quote(text)

        private fun reason(status: Int): String = when (status) {
            in 200..299 -> "OK"
            404 -> "Not Found"
            403 -> "Forbidden"
            else -> "Status"
        }

        private fun refused() = WebResourceResponse("text/plain", "utf-8", 403, "Forbidden", emptyMap(), ByteArrayInputStream(ByteArray(0)))

        private fun notFound() = WebResourceResponse("text/plain", "utf-8", 404, "Not Found", emptyMap(), ByteArrayInputStream(ByteArray(0)))
    }
}

/** A novel place as the store and the paginator write it: `3/0.4.1/120`. */
fun NovelPos.text(): String = "$spine/${path.joinToString(".")}/$offset"

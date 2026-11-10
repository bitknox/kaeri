// Credit pages and chapter parts, staged for shots on the fixtures engine: its sources have no
// group that repeats a page, so after each layout of the chapter opened the reader hears what the
// engine would say of one that does. The credit page is drawn here, a made-up card on a dark plate.

package app.kaeri.store

import android.content.Context
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.ColorMatrix
import android.graphics.ColorMatrixColorFilter
import android.graphics.Paint
import android.graphics.RectF
import android.os.Handler
import android.os.Looper
import app.kaeri.engine.model.ChapterPart
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReadingMsg
import java.io.File

object CreditsStage {
    /**
     * start, end, both, parts; two (two pages at the start), missing (parts 1, 2 and 4, part 3 no
     * group has) and broken (a start page whose picture cannot be read).
     */
    val STATES = listOf("start", "end", "both", "parts", "two", "missing", "broken")

    private var unlisten: (() -> Unit)? = null

    /**
     * From now on, every layout the reader hears comes with [what]'s credit pages or parts, the
     * page in greys for a grey e-ink panel. False for a name it does not know.
     */
    fun stage(context: Context, store: AppStore, what: String, grey: Boolean = false): Boolean {
        if (what !in STATES) return false
        val split = what == "parts" || what == "missing"
        val file = when {
            split -> null
            what == "broken" -> File(context.cacheDir, "credit-broken.png").let { out ->
                out.writeText("not a picture") // i18n-skip
                PageFile(out.path, PageEncoding.Original("image/png"), 800u, 1200u, null, true, out.length().toULong())
            }
            else -> draw(context, grey)
        }
        val main = Handler(Looper.getMainLooper())
        unlisten?.invoke()
        unlisten = store.listenToReading { event ->
            val leaves = ((event as? Event.Reader)?.v1 as? ReaderEvent.Leaves) ?: return@listenToReading
            val count = leaves.leaves.flatMap { leaf -> leaf.parts.map { it.page } }.distinct().size.toUInt()
            val said = if (file == null && what == "missing") {
                // Part 3 no group has: part 4 starts two thirds of the way in.
                val second = maxOf(count / 3u, 2u) + 1u
                val fourth = maxOf(count * 2u / 3u, second + 1u) + 1u
                ReaderEvent.ChapterParts(
                    leaves.session,
                    leaves.chapter,
                    listOf(ChapterPart(1u, 1u, null, false), ChapterPart(2u, second, null, false), ChapterPart(4u, fourth, null, false)),
                    4u,
                    listOf(3u),
                )
            } else if (file == null) {
                val second = maxOf(count / 2u, 2u) + 1u
                ReaderEvent.ChapterParts(
                    leaves.session,
                    leaves.chapter,
                    listOf(ChapterPart(1u, 1u, null, false), ChapterPart(2u, second, null, false)),
                    2u,
                    emptyList(),
                )
            } else {
                val pages = buildList {
                    if (what == "start" || what == "both" || what == "broken") add(CreditPage(0u, false, file, 0xC0FFEEuL))
                    if (what == "two") {
                        add(CreditPage(0u, false, file, 0xC0FFEEuL))
                        add(CreditPage(1u, false, file, 0xC0FFF0uL))
                    }
                    if (what == "end" || what == "both") add(CreditPage(count + size.toUInt(), true, file, 0xC0FFEFuL))
                }
                ReaderEvent.CreditPages(leaves.session, leaves.chapter, pages, Msg.Reading(ReadingMsg.ReaderCreditPagesSkipped(pages.size.toLong())))
            }
            // After the layout itself has reached the reader, as the engine sends them.
            main.post { store.stageReaderEvent(Event.Reader(said)) }
        }
        return true
    }

    /** A card with lines of type on a deep blue plate in a gold frame, as a group's page tends to look. */
    private fun draw(context: Context, grey: Boolean): PageFile {
        val width = 800
        val height = 1200
        val bitmap = Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        canvas.drawColor(0xFF1B2140.toInt())
        val gold = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFFD9B26A.toInt(); style = Paint.Style.STROKE; strokeWidth = 8f }
        canvas.drawRect(RectF(40f, 40f, width - 40f, height - 40f), gold)
        val card = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFFEDE7D9.toInt() }
        canvas.drawRoundRect(RectF(140f, 380f, width - 140f, 820f), 24f, 24f, card)
        val type = Paint(Paint.ANTI_ALIAS_FLAG).apply { color = 0xFF2A2F45.toInt() }
        for (line in 0 until 6) {
            val inset = if (line == 0) 200f else 220f + (line % 2) * 40f
            canvas.drawRoundRect(RectF(inset, 440f + line * 60f, width - inset, 466f + line * 60f), 8f, 8f, type)
        }
        // A lantern in gold lines above the card.
        canvas.drawCircle(width / 2f, 230f, 70f, gold)
        canvas.drawLine(width / 2f, 120f, width / 2f, 160f, gold)
        val page = if (!grey) bitmap else Bitmap.createBitmap(width, height, Bitmap.Config.ARGB_8888).also {
            val flat = Paint().apply { colorFilter = ColorMatrixColorFilter(ColorMatrix().apply { setSaturation(0f) }) }
            Canvas(it).drawBitmap(bitmap, 0f, 0f, flat)
        }
        val out = File(context.cacheDir, "credit-shot.png")
        out.outputStream().use { page.compress(Bitmap.CompressFormat.PNG, 100, it) }
        return PageFile(out.path, PageEncoding.Original("image/png"), width.toUInt(), height.toUInt(), null, !grey, out.length().toULong())
    }
}

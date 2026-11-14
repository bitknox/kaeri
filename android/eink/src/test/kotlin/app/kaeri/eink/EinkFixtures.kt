// Made-up reader records for the e-ink tests: a work, its chapter, and leaves of grey pages.

package app.kaeri.eink

import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.ChapterReadState
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.LeafPart
import app.kaeri.engine.model.PageEncoding
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.PageKind
import app.kaeri.engine.model.PixelFormat
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReadPlace
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderSurface
import app.kaeri.engine.model.ReaderView
import app.kaeri.engine.model.UnitRect
import java.io.File

object EinkFixtures {
    const val WORK = "local:lantern-stair"
    const val CHAPTER = "local:lantern-stair/12"
    const val SESSION = 7uL

    val whole = UnitRect(0f, 0f, 1f, 1f)

    fun chapter(id: String = CHAPTER, number: String = "12", title: String? = "The Long Stair", pages: UInt? = 3u) = ChapterRow(
        id = id,
        label = "Chapter $number",
        state = ChapterReadState.Unread,
        number = number,
        title = title,
        volume = null,
        published = null,
        pages = pages,
        newChapter = false,
        download = null,
        versions = 1u,
        bookmarked = false,
    )

    fun view(
        start: UInt? = null,
        direction: ReadDirection = ReadDirection.RIGHT_TO_LEFT,
        mode: ReaderMode = ReaderMode.PAGED,
        next: ChapterRow? = chapter("local:lantern-stair/13", "13", "Lanterns Out", 26u),
        medium: Medium = Medium.MANGA,
    ) = ReaderView(
        session = SESSION,
        work = WORK,
        title = "The Lantern Stair",
        medium = medium,
        chapter = chapter(),
        mode = mode,
        direction = direction,
        start = ReadPlace(WORK, CHAPTER, start, null, null),
        next = next,
        previous = null,
        settings = null,
    )

    fun part(page: UInt, file: PageFile? = null, place: UnitRect = whole) =
        LeafPart(page = page, kind = PageKind.STORY, file = file, place = place, rotate = false, half = null)

    /** [pages] single-page leaves and the end page after them. */
    fun leaves(pages: Int, file: (UInt) -> PageFile? = { null }): List<Leaf> =
        List(pages) { Leaf(it.toUInt(), listOf(part(it.toUInt(), file(it.toUInt()))), false) } +
            Leaf(pages.toUInt(), emptyList(), true)

    val surface = ReaderSurface(widthPx = 1080u, heightPx = 1920u, scale = 2.625f, eink = false, twoPages = false, dark = true)

    /** A grey page of [width] by [height] in rows [stride] bytes apart, each pixel its column's grey. */
    fun grey(dir: File, name: String, width: Int, height: Int, stride: Int = width): PageFile {
        val bytes = ByteArray(stride * height)
        for (y in 0 until height) for (x in 0 until width) bytes[y * stride + x] = (x * 255 / (width - 1).coerceAtLeast(1)).toByte()
        val file = File(dir, name).apply { writeBytes(bytes) }
        return PageFile(
            path = file.path,
            encoding = PageEncoding.Raw(PixelFormat.GRAY8, stride.toUInt()),
            width = width.toUInt(),
            height = height.toUInt(),
            crop = null,
            colour = false,
            bytes = bytes.size.toULong(),
        )
    }
}

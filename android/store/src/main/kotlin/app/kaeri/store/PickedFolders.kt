// Book folders picked in the system folder picker: the engine reads them through the content
// resolver, since a picked folder has no path the app may open.

package app.kaeri.store

import android.content.ContentResolver
import android.database.Cursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import app.kaeri.engine.ffi.FfiFolderAccess
import app.kaeri.engine.ffi.FfiFolderEntry
import app.kaeri.engine.ffi.FfiOpenedFile
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.channels.FileChannel
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong

/**
 * The app's [FfiFolderAccess]: the folders picked with the system folder picker, whose read
 * access the app keeps across launches with a persisted permission.
 *
 * A file or folder is named by its document's URI under the picked tree, which stays the same
 * across launches, with its name after a `#` so the engine sees its extension. Anything the
 * resolver refuses, a folder whose access was taken back included, answers null, and the engine
 * marks the folder as not found.
 *
 * The engine calls it on its blocking threads, never the main thread.
 */
class PickedFolders(private val resolver: ContentResolver) : FfiFolderAccess {
    private class Opened(val file: ParcelFileDescriptor, val channel: FileChannel)

    private val opened = ConcurrentHashMap<Long, Opened>()
    private val next = AtomicLong(1)

    override fun claims(reference: String): Boolean =
        reference.startsWith("content://") && uriOf(reference).pathSegments.firstOrNull() == TREE

    override fun list(folder: String): List<FfiFolderEntry>? = answer {
        val uri = uriOf(folder)
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(uri, documentOf(uri))
        resolver.query(children, COLUMNS, null, null, null)?.use { rows ->
            buildList { while (rows.moveToNext()) add(entryOf(uri, rows)) }
        }
    }

    override fun entry(reference: String): FfiFolderEntry? = answer {
        val uri = uriOf(reference)
        val document = DocumentsContract.buildDocumentUriUsingTree(uri, documentOf(uri))
        resolver.query(document, COLUMNS, null, null, null)?.use { rows ->
            if (rows.moveToFirst()) entryOf(uri, rows) else null
        }
    }

    override fun openFile(reference: String): FfiOpenedFile? = answer {
        val uri = uriOf(reference)
        val document = DocumentsContract.buildDocumentUriUsingTree(uri, documentOf(uri))
        val file = resolver.openFileDescriptor(document, "r") ?: return@answer null
        // A pipe or a stream from the network has no size and can't be read at an offset.
        val size = file.statSize
        if (size < 0) {
            file.close()
            return@answer null
        }
        val handle = next.getAndIncrement()
        opened[handle] = Opened(file, FileInputStream(file.fileDescriptor).channel)
        FfiOpenedFile(handle.toULong(), size.toULong())
    }

    override fun read(file: ULong, offset: ULong, length: UInt): ByteArray? = answer {
        val open = opened[file.toLong()] ?: return@answer null
        val buffer = ByteBuffer.allocate(length.toInt())
        var at = offset.toLong()
        while (buffer.hasRemaining()) {
            val n = open.channel.read(buffer, at)
            if (n < 0) break
            at += n
        }
        buffer.array().copyOf(buffer.position())
    }

    override fun close(file: ULong) {
        opened.remove(file.toLong())?.let { open ->
            runCatching { open.channel.close() }
            runCatching { open.file.close() }
        }
    }

    /** The row [rows] is on, as an entry named by its document under the tree [under] is in. */
    private fun entryOf(under: Uri, rows: Cursor): FfiFolderEntry {
        val id = rows.getString(0)
        val name = rows.getString(1).orEmpty()
        val folder = rows.getString(2) == Document.MIME_TYPE_DIR
        val size = if (rows.isNull(3)) 0L else rows.getLong(3)
        val modified = if (rows.isNull(4)) 0L else rows.getLong(4) / 1000
        val document = DocumentsContract.buildDocumentUriUsingTree(under, id)
        return FfiFolderEntry(
            reference = "$document#${Uri.encode(name)}",
            name = name,
            folder = folder,
            size = if (folder) 0uL else size.coerceAtLeast(0).toULong(),
            modified = modified,
        )
    }

    private companion object {
        const val TREE = "tree"

        val COLUMNS = arrayOf(
            Document.COLUMN_DOCUMENT_ID,
            Document.COLUMN_DISPLAY_NAME,
            Document.COLUMN_MIME_TYPE,
            Document.COLUMN_SIZE,
            Document.COLUMN_LAST_MODIFIED,
        )

        /** The URI a reference names, without the name after its `#`. */
        fun uriOf(reference: String): Uri = Uri.parse(reference).buildUpon().fragment(null).build()

        /** The document a tree URI names: the picked folder itself, or a document under it. */
        fun documentOf(uri: Uri): String {
            val segments = uri.pathSegments
            return if (segments.size >= 4 && segments[2] == "document") {
                DocumentsContract.getDocumentId(uri)
            } else {
                DocumentsContract.getTreeDocumentId(uri)
            }
        }

        /** [work]'s answer, or null when the resolver refused or the document is gone. */
        inline fun <T> answer(work: () -> T?): T? = try {
            work()
        } catch (_: Exception) {
            null
        }
    }
}

// Reading's notifications: new chapters of the series on the Reading list, and the chapters asked
// for all downloaded. Each has its own channel, so either can be silenced on its own.

package app.kaeri.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.graphics.Bitmap
import androidx.compose.ui.graphics.toArgb
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import app.kaeri.design.ObiSilk
import app.kaeri.engine.core.WorkId
import app.kaeri.i18n.Strings
import app.kaeri.store.NewChapterNote
import app.kaeri.ui.AppLink

/**
 * One notification per series with new chapters, on the "New chapters" channel at the default
 * importance, drawn as the new-episode ones are: the mark on amber, the series' title, "Chapter 12
 * is new." (or "3 new chapters"), the cover on the right, and expanded, the art in full with Read,
 * which opens the chapter the person is up to. A tap opens the series' page. With previews hidden,
 * the lock screen says only "New chapter". A series said again replaces its notification.
 *
 * Downloads finished posts on a channel of its own, with the engine's words, and opens the series
 * when the chapters were all of one, or Downloads.
 */
class ReadingNotifier(private val context: Context) {
    private val manager = context.getSystemService(NotificationManager::class.java)

    init {
        manager.createNotificationChannel(
            NotificationChannel(CHAPTERS, Strings.channelChaptersName(context), NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = Strings.channelChaptersDescription(context)
            },
        )
        manager.createNotificationChannel(
            NotificationChannel(DOWNLOADS_FINISHED, Strings.channelDownloadsFinishedName(context), NotificationManager.IMPORTANCE_DEFAULT).apply {
                description = Strings.channelDownloadsFinishedDescription(context)
            },
        )
    }

    /** Whether a notification on [channel] posted now would show. */
    fun canPost(channel: String = CHAPTERS): Boolean = NotificationPermission.granted(context) &&
        NotificationManagerCompat.from(context).areNotificationsEnabled() &&
        manager.getNotificationChannel(channel)?.importance != NotificationManager.IMPORTANCE_NONE

    /** Post [found], a notification for each series. */
    fun chapters(found: List<NewChapterNote>) {
        if (found.isEmpty() || !canPost(CHAPTERS)) return
        for ((work, chapters) in found.groupBy { it.work }) {
            manager.notify(tagOf(work), CHAPTER_ID, chapter(work, chapters))
        }
    }

    /** The chapters asked for are all downloaded: [work] when they were all of one series. */
    fun downloadsDone(work: WorkId?, title: String, body: String) {
        if (!canPost(DOWNLOADS_FINISHED)) return
        val link = work?.let(AppLink::Series) ?: AppLink.Downloads
        val tag = "downloads:${work ?: "all"}"
        val notification = frame(DOWNLOADS_FINISHED)
            .setContentTitle(title)
            .setContentText(body)
            .setContentIntent(EpisodeNotifier.openIntent(context, link, tag))
            .build()
        manager.notify(tag, DOWNLOADS_ID, notification)
    }

    /** Take [tag]'s notification away, as a tap on its Read does. */
    fun dismiss(tag: String) {
        manager.cancel(tag, CHAPTER_ID)
    }

    private fun frame(channel: String): NotificationCompat.Builder = NotificationCompat.Builder(context, channel)
        .setSmallIcon(R.drawable.ic_launcher_monochrome)
        .setColor(ObiSilk.Amber.toArgb())
        .setAutoCancel(true)

    private fun chapter(work: WorkId, chapters: List<NewChapterNote>): Notification {
        val newest = chapters.maxBy { it.published ?: 0L }
        val tag = tagOf(work)
        val text = if (chapters.size == 1) Strings.notifChapterNew(context, newest.label) else Strings.notifChaptersNew(context, chapters.size)
        val builder = frame(CHAPTERS)
            .setContentTitle(newest.title)
            .setContentText(text)
            .setOnlyAlertOnce(true)
            .setPublicVersion(frame(CHAPTERS).setContentTitle(context.getString(R.string.app_name)).setContentText(Strings.notifChapterHidden(context)).build())
            .setContentIntent(EpisodeNotifier.openIntent(context, AppLink.Series(work), tag))
            .addAction(0, Strings.notifChapterRead(context), EpisodeNotifier.openIntent(context, AppLink.Read(work, newest.opens), tag))
        newest.published?.let { builder.setWhen(it * 1000).setShowWhen(true) }
        newest.art?.let(builder::setLargeIcon)
        (newest.banner ?: newest.art)?.let { picture ->
            builder.setStyle(NotificationCompat.BigPictureStyle().bigPicture(picture).bigLargeIcon(null as Bitmap?))
        }
        return builder.build()
    }

    companion object {
        /** The channel, "New chapters". */
        const val CHAPTERS = "new_chapters"

        /** The channel, "Downloads finished". */
        const val DOWNLOADS_FINISHED = "downloads_finished"

        const val CHAPTER_ID = 11
        const val DOWNLOADS_ID = 12

        /** A series' new-chapter notification, by its tag. */
        fun tagOf(work: WorkId): String = "$TAG_PREFIX$work"

        /** Whether [tag] is a new-chapter notification's. */
        fun isChapters(tag: String): Boolean = tag.startsWith(TAG_PREFIX)

        private const val TAG_PREFIX = "chapters:"
    }
}

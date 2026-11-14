// A source with several groups, staged for shots on the fixtures engine: its series come from one
// group each. The series page then has a Group row and its picker, and the chapter rows show a
// chapter in parts and chapters from other groups. Names are made up.

package app.kaeri.store

import app.kaeri.engine.model.BookEditionRow
import app.kaeri.engine.model.ChapterDownloadState
import app.kaeri.engine.model.GroupChoice
import app.kaeri.engine.model.Msg
import app.kaeri.engine.model.PartGroup
import app.kaeri.engine.model.ReadingMsg
import app.kaeri.engine.model.VolumeGroup

object GroupsStage {
    /**
     * Every series and chapter list from now on lists three groups; off puts the engine's back.
     * With [downloaded] the chapter read in two parts is downloaded.
     */
    fun stage(store: AppStore, on: Boolean = true, downloaded: Boolean = false) {
        store.reading.seriesShot = if (on) { view -> view.copy(edition = view.edition?.let(::withGroups)) } else null
        store.reading.chaptersShot = if (on) { list -> list.copy(edition = list.edition?.let(::withGroups), volumes = withParts(list.volumes, downloaded)) } else null
    }

    fun withGroups(edition: BookEditionRow): BookEditionRow {
        val total = maxOf(edition.chapters, 20u)
        return edition.copy(
            versionLabel = "Lanternlight Scans", // i18n-skip
            groups = listOf(
                GroupChoice(label = "Lanternlight Scans", chapters = total - 2u, inUse = true, language = "en"), // i18n-skip
                GroupChoice(label = "Saltmarsh Team", chapters = total * 3u / 5u, inUse = false, language = "en"), // i18n-skip
                GroupChoice(label = "Bluefinch Group", chapters = total / 4u, inUse = false, language = "es"), // i18n-skip
            ),
            groupChosen = false,
        )
    }

    /**
     * The newest chapter from another group, one read in two parts (downloaded with [downloaded]),
     * and one with a part from a group in another language.
     */
    fun withParts(volumes: List<VolumeGroup>, downloaded: Boolean = false): List<VolumeGroup> {
        var seen = 0
        return volumes.map { volume ->
            volume.copy(
                chapters = volume.chapters.map { chapter ->
                    val row = when (seen) {
                        0 -> chapter.copy(
                            otherGroup = true,
                            versionLabel = "Saltmarsh Team", // i18n-skip
                            line = chapter.line + Msg.Reading(ReadingMsg.ReadingRowFromGroup("Saltmarsh Team")), // i18n-skip
                        )
                        2 -> {
                            val inParts = chapter.copy(parts = 2u, line = chapter.line + Msg.Reading(ReadingMsg.ReadingRowParts(2)))
                            if (!downloaded) inParts else inParts.copy(
                                download = ChapterDownloadState.Done,
                                line = inParts.line.map { if (it == Msg.Reading(ReadingMsg.ReadingRowNotDownloaded)) Msg.Reading(ReadingMsg.ReadingRowDownloaded) else it },
                            )
                        }
                        4 -> chapter.copy(
                            parts = 2u,
                            partGroups = listOf(PartGroup(part = 2u, group = "Bluefinch Group", language = "es")), // i18n-skip
                            line = chapter.line + Msg.Reading(ReadingMsg.ReadingRowParts(2)) +
                                Msg.Reading(ReadingMsg.ReadingRowPartFromGroupIn(2, "Bluefinch Group", "Spanish")), // i18n-skip
                        )
                        else -> chapter
                    }
                    seen += 1
                    row
                },
            )
        }
    }
}

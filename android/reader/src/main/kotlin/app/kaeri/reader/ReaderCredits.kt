// Credit pages and chapter parts in the reader: the pages a group repeats at the start or end of
// its chapters, which the engine leaves out, and a chapter the source lists in parts, which reads
// as one. The engine counts without the credit pages; the reader shows them again when asked, and
// keeps one as a page with Not a credit page, which waits out its Undo before the engine hears.
// What the session holds is in ReaderState; what follows from it is here.

package app.kaeri.reader

import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.model.ChapterPart
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.ReaderMode
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue

/** The open chapter's parts: one count of pages, with a tick where each later part starts. */
data class ReaderParts(val parts: List<ChapterPart>, val total: UInt, val missing: List<UInt>)

/** A credit page of a chapter, by its place among all the chapter's pages. */
data class CreditKey(val chapter: ChapterId, val page: UInt)

/** A credit page kept as a page, waiting out its Undo. [anchor] is the page it becomes, from 0. */
data class CreditKeep(val chapter: ChapterId, val credit: CreditPage, val anchor: Int)

/**
 * A mark on the scrubber where a later part starts, or where a missing one would be. [page] is
 * its first page from 0, which may be the chapter's page count for a part missing at the end.
 */
data class PartTick(val part: UInt, val page: Int, val missing: Boolean)

/** Where a turn goes when credit pages brought back are in its way. */
sealed interface CreditStep {
    /** The credit page [page] over the leaf [leaf]. */
    data class Credit(val page: UInt, val leaf: Int) : CreditStep

    /** The leaf itself, with no credit page over it. */
    data class Leaf(val leaf: Int) : CreditStep
}

/** The reader's own notice about a credit page, at the foot on a phone, in a box on e-ink. */
enum class CreditNotice {
    /** Skipped as a credit page, with Not a credit page. */
    SHOWN,

    /** This page will show from now on, with Undo. */
    KEPT,
}

/** The open chapter is read in pages, not a strip or a novel. */
val ReaderState.paged: Boolean
    get() = view != null && view.mode != ReaderMode.STRIP && view.mode != ReaderMode.VERTICAL && novel == null

/** A chapter's credit pages in the order the source had them. */
fun ReaderState.chapterCredits(chapter: ChapterId): List<CreditPage> = credits[chapter].orEmpty().sortedBy { it.page }

/** The open chapter's credit pages. */
val ReaderState.openCredits: List<CreditPage> get() = chapter?.let(::chapterCredits).orEmpty()

/**
 * The page, from 0 among the pages the chapter shows, a credit page sits before: the chapter's
 * page count when it comes after the last page.
 */
fun ReaderState.anchor(credit: CreditPage, chapter: ChapterId): Int =
    credit.page.toInt() - credits[chapter].orEmpty().count { it.page < credit.page }

fun ReaderState.isRevealed(credit: CreditPage, chapter: ChapterId): Boolean = CreditKey(chapter, credit.page) in revealed

/** The leaf a credit page of the open chapter sits before: the end page for those after the last page. */
fun ReaderState.creditLeaf(credit: CreditPage): Int {
    val chapter = chapter ?: return 0
    val anchor = anchor(credit, chapter)
    val fallback = (leaves.size - 1).coerceAtLeast(0)
    if (credit.atEnd || anchor >= pages) return leaves.indexOfFirst { it.endPage }.takeIf { it >= 0 } ?: fallback
    return leaves.indexOfFirst { leaf -> !leaf.endPage && leaf.parts.any { it.page.toInt() >= anchor } }.takeIf { it >= 0 } ?: fallback
}

/** The open chapter's credit pages brought back that sit before the leaf at [leaf]. */
fun ReaderState.revealedBefore(leaf: Int): List<CreditPage> {
    val chapter = chapter ?: return emptyList()
    if (!paged) return emptyList()
    return chapterCredits(chapter).filter { isRevealed(it, chapter) && creditLeaf(it) == leaf }
}

/** The credit page on screen, while the paged reader shows one. */
val ReaderState.shownCredit: CreditPage?
    get() = onCredit?.let { page -> openCredits.firstOrNull { it.page == page } }

/** The open chapter's credit pages at its start that are still skipped, which page 1's line names. */
val ReaderState.startSkipped: List<CreditPage>
    get() {
        val chapter = chapter ?: return emptyList()
        return chapterCredits(chapter).filter { !it.atEnd && anchor(it, chapter) == 0 && !isRevealed(it, chapter) }
    }

/** A chapter's credit pages at its end that are still skipped, which the end page and the strip's divider name. */
fun ReaderState.endSkipped(chapter: ChapterId): List<CreditPage> =
    chapterCredits(chapter).filter { it.atEnd && !isRevealed(it, chapter) }

/**
 * "credit page 1 of 2": where the credit page on screen is among those shown before the same
 * page, as (n, of). Null off a credit page.
 */
val ReaderState.creditPlace: Pair<Int, Int>?
    get() {
        val credit = shownCredit ?: return null
        val group = revealedBefore(creditLeaf(credit))
        val n = group.indexOfFirst { it.page == credit.page }.coerceAtLeast(0) + 1
        return n to group.size.coerceAtLeast(1)
    }

/**
 * The page kept from the open chapter, while its Undo runs and then until the chapter comes laid
 * out with it, so its number and the notice hold through the engine's answer.
 */
val ReaderState.keptHere: CreditKeep? get() = (keeping ?: committed)?.takeIf { it.chapter == chapter }

/** The page count the scrubber shows: a kept page counts as soon as it is kept, page 1 of 38. */
val ReaderState.shownPages: Int get() = pages + if (keptHere == null) 0 else 1

/** The page, from 0, the scrubber shows for [page], counting a kept page from the moment it is kept. */
fun ReaderState.shownPage(page: Int): Int {
    val keep = keptHere ?: return page
    if (onCredit == keep.credit.page) return keep.anchor
    return if (page >= keep.anchor) page + 1 else page
}

/** The credit page on screen is only shown, not kept: it has no number of its own. */
val ReaderState.onUnkeptCredit: Boolean get() = shownCredit != null && keptHere?.credit?.page != onCredit

/** The marks on the scrubber: where each part after the first starts, and where a missing part would be. */
val ReaderState.partTicks: List<PartTick>
    get() {
        val parts = parts ?: return emptyList()
        if (!paged || leaves.isEmpty()) return emptyList()
        val ticks = parts.parts.drop(1).map { PartTick(it.number, it.firstPage.toInt() - 1, missing = false) }.toMutableList()
        for (number in parts.missing) {
            if (number <= 1u) continue
            // Where it would start: the first page of the part after it, or after the last page.
            val after = parts.parts.firstOrNull { it.number > number }
            ticks += PartTick(number, after?.let { it.firstPage.toInt() - 1 } ?: pages, missing = true)
        }
        return ticks.sortedBy { it.page }
    }

/** The part a page, from 0, starts, when it is the first page of a later part. */
fun ReaderState.partStarting(page: Int): UInt? =
    parts?.parts?.drop(1)?.firstOrNull { it.firstPage.toInt() - 1 == page }?.number

/** The part no source has that would start where page [page], from 0, does: the one before the part there. */
fun ReaderState.partMissingAt(page: Int): UInt? = partTicks.lastOrNull { it.missing && it.page == page }?.part

/** Where a turn from [leaf] goes across credit pages brought back; null for an ordinary turn. */
fun ReaderState.creditStep(forward: Boolean, leaf: Int): CreditStep? {
    if (leaf !in leaves.indices || !paged) return null
    val credit = shownCredit
    if (credit != null) {
        val group = revealedBefore(leaf)
        val at = group.indexOfFirst { it.page == credit.page }.coerceAtLeast(0)
        if (forward) return if (at + 1 < group.size) CreditStep.Credit(group[at + 1].page, leaf) else CreditStep.Leaf(leaf)
        if (at > 0) return CreditStep.Credit(group[at - 1].page, leaf)
        return if (leaf > 0) CreditStep.Leaf(leaf - 1) else null
    }
    if (forward) {
        // Past the end page is the next chapter, never its credit pages.
        if (leaves[leaf].endPage) return null
        val next = leaf + 1
        val first = revealedBefore(next).firstOrNull() ?: return null
        return CreditStep.Credit(first.page, next)
    }
    val last = revealedBefore(leaf).lastOrNull() ?: return null
    return CreditStep.Credit(last.page, leaf)
}

/**
 * The reader's own notice: Not a credit page while a credit page not kept shows in pages, else
 * Undo while a kept page waits, which outlasts a turn.
 */
val ReaderState.creditNotice: CreditNotice?
    get() = when {
        paged && onUnkeptCredit -> CreditNotice.SHOWN
        keeping != null -> CreditNotice.KEPT
        else -> null
    }

/**
 * What a Fixtures shot asks the open reader to show once its credit pages or parts have come:
 * line (page 1's line, held, with the Undo after it timed as in the app), chrome (with the
 * chrome), shown, shownchrome, kept (with the chrome, its Undo held), parts (the chrome over a
 * chapter in parts) or end (the end page). Set by the Fixtures app's `creditshot` extra; null in
 * the app.
 */
object CreditShot {
    var step by mutableStateOf<String?>(null)
}

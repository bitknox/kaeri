// The reader's half of the engine's reader session: the commands a reader screen sends and the
// state the engine's reader events build, for one session at a time (protocol.md, sections 4.3,
// 5.2 and 6). The engine decides; this only reports what was painted and keeps what it was told.

package app.kaeri.reader

import app.kaeri.engine.core.ChapterId
import app.kaeri.engine.core.WorkId
import app.kaeri.engine.model.ChapterRow
import app.kaeri.engine.model.ChapterVersion
import app.kaeri.engine.model.Command
import app.kaeri.engine.model.CreditPage
import app.kaeri.engine.model.Event
import app.kaeri.engine.model.Leaf
import app.kaeri.engine.model.LeafPart
import app.kaeri.engine.core.Medium
import app.kaeri.engine.model.NoNext
import app.kaeri.engine.model.NovelOpenView
import app.kaeri.engine.model.NovelPos
import app.kaeri.engine.model.OpenAt
import app.kaeri.engine.model.PageFile
import app.kaeri.engine.model.PageHalf
import app.kaeri.engine.model.PageKind
import app.kaeri.engine.model.ReadDirection
import app.kaeri.engine.model.ReadPlace
import app.kaeri.engine.model.ReaderCommand
import app.kaeri.engine.model.ReaderEvent
import app.kaeri.engine.model.ReaderMode
import app.kaeri.engine.model.ReaderNotice
import app.kaeri.engine.model.ReaderSettingKey
import app.kaeri.engine.model.ReaderSettingsView
import app.kaeri.engine.model.ReaderSurface
import app.kaeri.engine.model.ReaderValue
import app.kaeri.engine.model.ReaderView
import app.kaeri.engine.model.ReaderWait
import app.kaeri.engine.model.ReadingAbout
import app.kaeri.engine.model.ReadingCommand
import app.kaeri.engine.model.VolumeGroup
import app.kaeri.engine.model.ReadingEvent
import app.kaeri.engine.model.ReadingFailure
import app.kaeri.engine.model.SettingScope
import app.kaeri.engine.model.StripPos
import app.kaeri.engine.model.TurnHow
import app.kaeri.store.AppStore
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

/** Where one page file goes: a page, and for a spread split in two, which half. */
data class PageKey(val page: UInt, val half: PageHalf? = null)

/** What the end page names: the next chapter and the source it came from, or why there is none. */
data class EndPageView(val next: ChapterRow?, val whyNone: NoNext?, val source: String? = null)

/** The next chapter's first page, made while this one is read, for the end page. */
data class NextPreview(val chapter: ChapterId, val file: PageFile)

/** A notice to show, with an id of its own so the same words twice still show twice. */
data class ShownNotice(val notice: ReaderNotice, val id: Long)

/**
 * A leaf to show that the reader did not turn to: the start place, Back to page, Start over, Go
 * back, a place another device reached. [strip] is the place inside a strip image.
 */
data class ReaderJump(val leaf: Int, val strip: StripPos?, val id: Long)

/** A place in the open novel the engine asked for: Back to page, Start over, Go back. */
data class NovelGoTo(val pos: NovelPos, val id: Long)

/** The clock a chapter's opening wait counts on, in milliseconds. */
fun openingClock(): Long = System.nanoTime() / 1_000_000

/** What the engine said of the open session. */
data class ReaderState(
    /** Asked to open and not yet answered. */
    val opening: Boolean = false,
    val view: ReaderView? = null,
    val leaves: List<Leaf> = emptyList(),
    /** Bumped with each layout the engine sends, for a surface to place itself again. */
    val layout: Int = 0,
    /** Files that landed after the leaves came: [file] puts them in their parts. */
    val files: Map<PageKey, PageFile> = emptyMap(),
    /**
     * The files a page had before the chapter was laid out again (a page that landed in another
     * shape): drawn until the engine sends the page for the new layout, so the page on screen
     * never drops to its plate.
     */
    val held: Map<PageKey, PageFile> = emptyMap(),
    val failures: Map<UInt, ReadingFailure> = emptyMap(),
    val end: EndPageView? = null,
    val notice: ShownNotice? = null,
    val moved: ReaderEvent.Moved? = null,
    val settings: ReaderSettingsView? = null,
    val novel: NovelOpenView? = null,
    val novelStyle: ReaderEvent.NovelStyle? = null,
    /** Chapters the read rule counted while this reader was open. */
    val read: Set<ChapterId> = emptySet(),
    val jump: ReaderJump? = null,
    val novelGoTo: NovelGoTo? = null,
    /**
     * Back to page in the slider bar: the page counted from 1, offered after the person's own jump
     * of more than two pages and gone at the next turn.
     */
    val backTo: UInt? = null,
    /** The series' chapters by volume, once asked for. */
    val volumes: List<VolumeGroup> = emptyList(),
    /** The trackers each chapter read here was queued for. */
    val readTrackers: Map<ChapterId, List<String>> = emptyMap(),
    /** The open itself failed: the chapter could not be had. */
    val failure: ReadingFailure? = null,
    val closed: Boolean = false,
    /** While opening: the chapter, once the engine knows it, and what it waits on. */
    val openingChapter: ChapterRow? = null,
    val openingWait: ReaderWait? = null,
    /** When the chapter on screen was asked for, on [openingClock]; 0 when nothing was asked. */
    val openingSince: Long = 0L,
    val preview: NextPreview? = null,
    /** The chapter a strip flowed on from, drawn above this one in the same scroll. */
    val previous: ReaderState? = null,
    /** The other sources a failed chapter can be read from; null until the engine answers. */
    val sources: List<ChapterVersion>? = null,
    /**
     * The credit pages the engine left out, by chapter: the open one's, and those of the chapter a
     * strip flowed on from, which stays above it.
     */
    val credits: Map<ChapterId, List<CreditPage>> = emptyMap(),
    /** The open chapter's parts, when its source lists it in parts the reader runs through as one. */
    val parts: ReaderParts? = null,
    /** The credit pages brought back. They stay until their chapter closes. */
    val revealed: Set<CreditKey> = emptySet(),
    /** The credit page the paged reader shows over its leaf, by its page in the chapter. */
    val onCredit: UInt? = null,
    /** Not a credit page was used: the page is kept once its Undo has run out. */
    val keeping: CreditKeep? = null,
    /** A keep the engine has heard, until the chapter comes laid out with the page in it. */
    val committed: CreditKeep? = null,
) {
    val session: ULong? get() = view?.session
    val chapter: ChapterId? get() = view?.chapter?.id

    /**
     * The chapter's pages, without the end page: as many as the leaves on one page a leaf, more
     * where a leaf holds a spread's two pages, fewer where a spread is split into halves.
     */
    val pages: Int get() = leaves.filter { !it.endPage }.maxOfOrNull { leaf -> leaf.parts.maxOfOrNull { it.page.toInt() + 1 } ?: 0 } ?: 0

    /** The file for [part]: the one the leaf came with, or the one that landed since. */
    fun file(part: LeafPart): PageFile? = PageKey(part.page, part.half).let { key -> files[key] ?: part.file ?: held[key] }

    /** The file made for [part] in this layout, without the one [held] from the layout before. */
    fun landed(part: LeafPart): PageFile? = files[PageKey(part.page, part.half)] ?: part.file

    /** The leaf that shows [page], or null when no leaf does. */
    fun leafOf(page: UInt): Int? = leaves.indexOfFirst { leaf -> leaf.parts.any { it.page == page } }.takeIf { it >= 0 }

    /** The leaf a place shows: its page, its strip image, or the first. */
    fun leafOf(place: ReadPlace): Int = place.page?.let(::leafOf) ?: place.strip?.image?.let(::leafOf) ?: 0
}

/**
 * This view with the mode and direction [settings] now give, for a paged or strip work; a novel's
 * pages are its own. On e-ink a vertical scroll reads as pages, as the engine lays it out.
 */
internal fun ReaderView.following(settings: ReaderSettingsView, eink: Boolean): ReaderView {
    if (medium == Medium.NOVEL || settings.medium != medium) return this
    fun word(key: ReaderSettingKey) = (settings.rows.firstOrNull { it.key == key }?.value as? ReaderValue.Choice)?.name
    val direction = when (word(ReaderSettingKey.DIRECTION)) {
        "ltr" -> ReadDirection.LEFT_TO_RIGHT
        "rtl" -> ReadDirection.RIGHT_TO_LEFT
        else -> direction
    }
    val mode = when (word(ReaderSettingKey.MODE)) {
        "strip" -> ReaderMode.STRIP
        "vertical" -> if (eink) ReaderMode.PAGED else ReaderMode.VERTICAL
        "paged" -> ReaderMode.PAGED
        else -> mode
    }
    return if (mode == this.mode && direction == this.direction) this else copy(mode = mode, direction = direction)
}

/**
 * One reader session as the engine runs it. A screen opens it with the surface it has, reports
 * each leaf it paints, and draws [state]. Bound to the store with [bind]; commands go out through
 * [send].
 */
class ReaderSession(private val send: (Command) -> Unit) {
    private val _state = MutableStateFlow(ReaderState())
    val state: StateFlow<ReaderState> = _state.asStateFlow()

    private var ids = 0L
    private var work: WorkId? = null
    private var asked: Pair<ChapterId?, OpenAt>? = null
    private var surface: ReaderSurface? = null
    /** A chapter asked for and not yet opened, so its failure is shown in its name. */
    private var awaiting: ChapterRow? = null
    private var awaitingOpen = false
    /** A strip flowing on: the chapter it flows into and the one it leaves, kept above. */
    private var flowTo: ChapterId? = null
    private var flowFrom: ReaderState? = null
    private var startAt: ReadPlace? = null
    private var shownPage: UInt? = null
    /** Where the reader is: the chapter on screen and the page, strip spot or text place shown. */
    private var here: ReadPlace? = null
    private val askedPages = mutableSetOf<PageKey>()
    /** The leaf last reported shown, so a new layout of the same place is not a turn. */
    private var shownLeaf: Int? = null
    /** A kept page's chapter is laid out again with it: the pages after it move up one. */
    private var relayKeep: CreditKeep? = null

    /** Hear the store's reader events until what is returned is called. */
    fun bind(store: AppStore): () -> Unit = store.listenToReading(::apply)

    // What the screen sends.

    fun open(work: WorkId, surface: ReaderSurface, chapter: ChapterId? = null, at: OpenAt = OpenAt.Resume) {
        this.work = work
        this.asked = chapter to at
        this.surface = surface
        awaitingOpen = true
        flowTo = null
        flowFrom = null
        startAt = null
        shownPage = null
        here = null
        shownLeaf = null
        askedPages.clear()
        commitKeep()
        _state.value = ReaderState(
            opening = true,
            openingChapter = state.value.openingChapter?.takeIf { it.id == chapter },
            openingSince = openingClock(),
        )
        reader(ReaderCommand.Open(work, chapter, at, surface))
    }

    /** The surface changed size, room for two pages or look: sent only when it did. */
    fun surface(surface: ReaderSurface) {
        if (surface == this.surface) return
        this.surface = surface
        val session = state.value.session ?: return
        reader(ReaderCommand.Surface(session, surface))
    }

    /** Leaf [leaf] was painted. An e-ink notice clears at the next turn, not at a jump or the app's own move. */
    fun shown(leaf: Int, how: TurnHow) {
        val now = state.value
        val session = now.session ?: return
        val chapter = now.chapter ?: return
        // A leaf that holds the page kept, as a new layout's spread holds a page shown alone
        // before it, keeps that page: single pages again show it, not the spread's first.
        val pages = now.leaves.getOrNull(leaf)?.parts?.map { it.page }.orEmpty()
        shownPage = if (shownPage in pages) shownPage else pages.firstOrNull() ?: shownPage
        here = here?.takeIf { it.chapter == chapter }?.copy(page = shownPage, strip = null)
        // Off the leaf a credit page shows over, the credit page goes. A page kept from it waits
        // on: its Undo outlasts the turn, and the engine hears when it runs out or the chapter closes.
        val moved = shownLeaf != null && shownLeaf != leaf
        shownLeaf = leaf
        val credit = now.shownCredit
        if (moved && credit != null && now.creditLeaf(credit) != leaf) showCredit(null)
        reader(ReaderCommand.Shown(session, chapter, leaf.toUInt(), how))
        val notice = now.notice
        if (notice != null && notice.notice.lastsMs == 0u && how != TurnHow.JUMP && how != TurnHow.PLACED) dismissNotice(notice.id)
    }

    /** A strip settled at [at]; [footVisible] when the last image's foot is in view. */
    fun stripAt(at: StripPos, footVisible: Boolean) {
        val now = state.value
        val chapter = now.chapter ?: return
        here = here?.takeIf { it.chapter == chapter }?.copy(page = at.image, strip = at)
        reader(ReaderCommand.StripAt(now.session ?: return, chapter, at, footVisible))
    }

    /** The paginator turned in a novel. A notice lies over the text, so the first turn takes it. */
    fun novelAt(pos: NovelPos, page: UInt, pages: UInt, how: TurnHow) {
        val now = state.value
        val chapter = now.novel?.chapter ?: now.chapter ?: return
        val work = work
        if (work != null) here = (here?.takeIf { it.chapter == chapter } ?: ReadPlace(work, chapter, null, null, null)).copy(novel = pos)
        reader(ReaderCommand.NovelAt(now.session ?: return, chapter, pos, page, pages, how))
        val notice = now.notice
        if (notice != null && how != TurnHow.JUMP && how != TurnHow.PLACED) dismissNotice(notice.id)
    }

    /** The hidden web view counted the volume's pages for these settings and this screen. */
    fun novelCounted(key: String, chapterPages: List<UInt>) {
        reader(ReaderCommand.NovelCounted(state.value.session ?: return, key, chapterPages))
    }

    /** E-ink only: a fixed-layout page rendered to the file the engine named. */
    fun fixedRendered(page: UInt, path: String) {
        val now = state.value
        reader(ReaderCommand.FixedRendered(now.session ?: return, now.novel?.chapter ?: now.chapter ?: return, page, path))
    }

    /** A file the session still shows is gone: made again, asked once per layout. */
    fun needPage(part: LeafPart) {
        val now = state.value
        if (!askedPages.add(PageKey(part.page, part.half))) return
        _state.value = now.copy(files = now.files - PageKey(part.page, part.half))
        reader(ReaderCommand.NeedPage(now.session ?: return, now.chapter ?: return, part.page))
    }

    /** Try a failed page again, or the whole chapter with no page. */
    fun retry(page: UInt?) {
        val now = state.value
        if (page != null) _state.value = now.copy(failures = now.failures - page)
        reader(ReaderCommand.Retry(now.session ?: return, now.chapter ?: return, page))
    }

    fun openChapter(chapter: ChapterId) {
        commitKeep()
        val now = state.value
        val session = now.session ?: return
        awaiting = now.view?.next?.takeIf { it.id == chapter } ?: now.volumes.flatMap { it.chapters }.firstOrNull { it.id == chapter }
        awaitingOpen = true
        reader(ReaderCommand.OpenChapter(session, chapter))
    }

    /**
     * A forward turn on the end page, by swipe, tap or key: the next chapter opens, as the page's
     * hint says. False when there is no next chapter; a second turn while it opens asks nothing.
     */
    fun crossOn(): Boolean {
        val now = state.value
        val next = now.end?.next ?: now.view?.next ?: return false
        if (awaitingOpen && awaiting?.id == next.id) return true
        openChapter(next.id)
        return true
    }

    /**
     * A strip scrolled onto its divider: open [chapter] and keep this one above it, so the next
     * chapter's images follow in the same scroll.
     */
    fun flowInto(chapter: ChapterId) {
        if (flowTo == chapter) return
        flowFrom = state.value.copy(previous = null)
        flowTo = chapter
        openChapter(chapter)
    }

    /** Try the open again, from the start, when it failed before any chapter opened. */
    fun retryOpen() {
        val now = state.value
        if (now.session != null && now.view != null) return retry(null)
        val work = work ?: return
        val surface = surface ?: return
        val (chapter, at) = asked ?: (null to OpenAt.Resume)
        open(work, surface, now.openingChapter?.id ?: chapter, at)
    }

    /** Ask which other sources hold the chapter that would not open. */
    fun loadSources() {
        val now = state.value
        val work = work ?: return
        val chapter = now.openingChapter?.id ?: now.chapter ?: asked?.first ?: return
        send(Command.Reading(ReadingCommand.LoadVersions(work, chapter)))
    }

    /** Read the series from [version]'s source from now on, and open the reader again with it. */
    fun chooseSource(version: ChapterVersion) {
        val work = work ?: return
        val surface = surface ?: return
        val edition = version.edition
        send(Command.Reading(ReadingCommand.ChooseEdition(work, edition.sourceId, edition.versionLabel, edition.language)))
        open(work, surface, null, OpenAt.Resume)
    }

    /** Read the series again from its first chapter: fresh marks, the old ones set aside. */
    fun readAgain() {
        val work = work ?: return
        val surface = surface ?: return
        send(Command.Reading(ReadingCommand.ReadAgain(work)))
        val first = state.value.volumes.firstOrNull()?.chapters?.firstOrNull()?.id
        open(work, surface, first, OpenAt.Start)
    }

    /** Ask for the series' chapters, for the chapter list. */
    fun loadChapters() {
        val work = state.value.view?.work ?: return
        send(Command.Reading(ReadingCommand.LoadChapters(work, null)))
    }

    /** Back to the page before the jump. The engine takes the offer away as it goes. */
    fun backTo() {
        val now = state.value
        val session = now.session ?: return
        if (now.backTo == null) return
        _state.value = now.copy(backTo = null)
        reader(ReaderCommand.BackTo(session))
    }

    fun startOver() = state.value.session?.let { reader(ReaderCommand.StartOver(it)) }

    fun goBack() = state.value.session?.let { reader(ReaderCommand.GoBack(it)) }

    /** Change one setting, live: the engine answers with the rows and lays out again if it must. */
    fun setSetting(key: ReaderSettingKey, scope: SettingScope, value: ReaderValue) {
        val now = state.value
        val medium = now.settings?.medium ?: now.view?.medium ?: return
        reader(ReaderCommand.SetSetting(now.session, medium, now.view?.work, key, scope, value))
    }

    /** Drop the series' own choice for [key]. */
    fun useDefault(key: ReaderSettingKey) {
        val now = state.value
        val medium = now.settings?.medium ?: now.view?.medium ?: return
        reader(ReaderCommand.UseDefault(medium, now.view?.work ?: return, key))
    }

    fun loadSettings(medium: Medium, work: WorkId?) = reader(ReaderCommand.LoadSettings(medium, work))

    /**
     * Where the reader is, to open there again when the activity is made anew: the chapter on
     * screen and the page, strip spot or text place it shows. Null before a chapter opened.
     */
    fun place(): ReadPlace? = here

    fun close() {
        commitKeep()
        val session = state.value.session ?: return
        reader(ReaderCommand.Close(session))
    }

    /** The notice went: its time ran out or the next turn took it. */
    fun dismissNotice(id: Long) {
        val now = state.value
        if (now.notice?.id == id) _state.value = now.copy(notice = null)
    }

    /** The jump was shown. */
    fun jumped(id: Long) {
        val now = state.value
        if (now.jump?.id == id) _state.value = now.copy(jump = null)
    }

    // Credit pages (what follows from them is in ReaderCredits.kt).

    /**
     * The paged reader shows the credit page [page] over its leaf, or the leaf alone with null. A
     * page kept waits out its Undo across turns.
     */
    fun showCredit(page: UInt?) {
        if (state.value.onCredit != page) _state.value = state.value.copy(onCredit = page)
    }

    /**
     * Page 1's line was tapped: the credit pages it names come back. In pages the first of them
     * is returned, for the reader to show by the same crossfade as a turn.
     */
    fun showStartCredits(): CreditStep.Credit? {
        val now = state.value
        val chapter = now.chapter ?: return null
        val group = now.startSkipped
        val first = group.firstOrNull() ?: return null
        val revealed = now.copy(revealed = now.revealed + group.map { CreditKey(chapter, it.page) })
        _state.value = revealed
        return if (now.paged) CreditStep.Credit(first.page, revealed.creditLeaf(first)) else null
    }

    /**
     * Show on the end page, or on a strip's divider: the pages skipped at the end of [chapter]
     * come back. In pages, the first of them is returned to show.
     */
    fun showEndCredits(chapter: ChapterId? = null): CreditStep.Credit? {
        val now = state.value
        val id = chapter ?: now.chapter ?: return null
        val group = now.endSkipped(id)
        val first = group.firstOrNull() ?: return null
        val revealed = now.copy(revealed = now.revealed + group.map { CreditKey(id, it.page) })
        _state.value = revealed
        return if (now.paged && id == now.chapter) CreditStep.Credit(first.page, revealed.creditLeaf(first)) else null
    }

    /** A strip's line brought the pages it names back in place. */
    fun reveal(pages: List<CreditPage>, chapter: ChapterId) {
        val now = state.value
        _state.value = now.copy(revealed = now.revealed + pages.map { CreditKey(chapter, it.page) })
    }

    /** Not a credit page, for [credit] of [chapter]: the credit page on screen when none is named. */
    fun keepCredit(credit: CreditPage? = null, chapter: ChapterId? = null) {
        val now = state.value
        val page = credit ?: now.shownCredit ?: return
        val id = chapter ?: now.chapter ?: return
        if (now.keeping != null) commitKeep()
        val after = state.value
        // The page counts from now; the engine hears once the Undo has run out.
        _state.value = after.copy(keeping = CreditKeep(id, page, after.anchor(page, id)))
    }

    /** Undo, while the kept page waits: it is a credit page again, still shown. */
    fun undoKeep() {
        val now = state.value
        if (now.keeping != null) _state.value = now.copy(keeping = null)
    }

    /**
     * The kept page goes to the engine, which keeps it in the series from now on and lays the
     * chapter out again with it.
     */
    fun commitKeep() {
        val now = state.value
        val keep = now.keeping ?: return
        _state.value = now.copy(keeping = null, committed = keep)
        val session = now.session ?: return
        relayKeep = keep
        reader(ReaderCommand.NotACreditPage(session, keep.credit.fingerprint))
    }

    /**
     * The chapter came laid out again. When it is the layout with a kept page, the pages from that
     * page on move up one, so what has landed keeps its place, and the screen stays on the page it
     * showed: the kept page itself when it was on screen.
     */
    private fun relaidWithKept(placed: ReaderState, before: ReaderState): ReaderState {
        val keep = relayKeep?.takeIf { it.chapter == placed.chapter } ?: return placed
        relayKeep = null
        val anchor = keep.anchor.toUInt()
        fun moved(files: Map<PageKey, PageFile>) = files.mapKeys { (key, _) -> if (key.page >= anchor) key.copy(page = key.page + 1u) else key }
        val held = moved(before.held + before.files) + (PageKey(anchor) to keep.credit.file)
        val pictures = placed.leaves.flatMap { it.parts }.filter { it.kind != PageKind.STRIP_CUT }.map { PageKey(it.page, it.half) }.toSet()
        val strip = !before.paged
        val here = this.here
        if (strip) {
            val at = here?.strip
            if (at != null && at.image > anchor) {
                val image = at.image + 1u
                this.here = here.copy(page = image, strip = at.copy(image = image))
                return placed.copy(
                    held = held.filterKeys { it in pictures },
                    failures = emptyMap(),
                    revealed = placed.revealed - CreditKey(keep.chapter, keep.credit.page),
                    jump = ReaderJump(placed.leafOf(image) ?: 0, at.copy(image = image), ++ids),
                )
            }
        } else {
            shownPage = when {
                before.onCredit == keep.credit.page -> anchor
                else -> shownPage?.let { if (it >= anchor) it + 1u else it }
            }
        }
        return placed.copy(
            held = held.filterKeys { it in pictures },
            failures = emptyMap(),
            revealed = placed.revealed - CreditKey(keep.chapter, keep.credit.page),
            onCredit = if (before.onCredit == keep.credit.page) null else placed.onCredit,
        )
    }

    // What the engine says.

    /** One event from the store; anything that is not the reader's, or another session's, is left. */
    fun apply(event: Event) {
        when (event) {
            is Event.Reader -> apply(event.v1)
            is Event.Reading -> {
                val chapters = event.v1 as? ReadingEvent.Chapters
                if (chapters != null) {
                    if (chapters.work == state.value.view?.work) _state.value = state.value.copy(volumes = chapters.volumes)
                    return
                }
                val versions = event.v1 as? ReadingEvent.Versions
                if (versions != null) {
                    if (versions.work == work) _state.value = state.value.copy(sources = versions.versions)
                    return
                }
                val failed = event.v1 as? ReadingEvent.Failed ?: return
                if (failed.about != ReadingAbout.READER || !awaitingOpen) return
                awaitingOpen = false
                val now = state.value
                flowTo = null
                flowFrom = null
                // A chapter asked for from another one: the reader shows that chapter's failure in its place.
                val chapter = awaiting ?: now.openingChapter
                awaiting = null
                _state.value = ReaderState(
                    failure = failed.failure,
                    openingChapter = chapter,
                    read = now.read,
                    readTrackers = now.readTrackers,
                    volumes = now.volumes,
                    settings = now.settings,
                )
            }
            else -> {}
        }
    }

    fun apply(event: ReaderEvent) {
        val now = state.value
        fun ours(session: ULong) = session == now.session
        _state.value = when (event) {
            // Each open and each chapter opened: a fresh chapter, its start place kept for the leaves.
            is ReaderEvent.Opened -> {
                if (!now.opening && now.session != null && event.v1.session != now.session) return
                awaitingOpen = false
                awaiting = null
                startAt = event.v1.start
                shownPage = null
                here = event.v1.start
                askedPages.clear()
                // A strip that flowed on keeps the chapter it left above this one.
                val previous = flowFrom?.takeIf { flowTo == event.v1.chapter.id }
                flowTo = null
                flowFrom = null
                shownLeaf = null
                ReaderState(
                    view = event.v1,
                    settings = event.v1.settings ?: now.settings,
                    read = now.read,
                    readTrackers = now.readTrackers,
                    volumes = now.volumes,
                    layout = now.layout + 1,
                    previous = previous,
                    // A chapter opened again skips its credit pages again; a strip that flowed on
                    // keeps those of the chapter above.
                    credits = if (previous != null) now.credits else emptyMap(),
                    revealed = if (previous != null) now.revealed else emptySet(),
                    // A novel's text comes after this; the wait it shows counts from the ask.
                    openingSince = now.openingSince,
                )
            }
            is ReaderEvent.Leaves -> {
                if (!ours(event.session) || event.chapter != now.chapter) return
                askedPages.clear()
                // The start place on the first layout; after that the page shown stays shown.
                // Laid out again: a page keeps the picture it had until its new one lands. Strip cuts
                // are new pictures each layout, and a strip asks for its own.
                val held = if (now.leaves.isEmpty()) emptyMap() else {
                    val pictures = event.leaves.flatMap { it.parts }.filter { it.kind != PageKind.STRIP_CUT }
                        .map { PageKey(it.page, it.half) }.toSet()
                    (now.held + now.files).filterKeys { it in pictures }
                }
                // The credit pages and parts come again after every layout; until they do, those of
                // the last layout stay, so a turn of the device does not drop them for a moment. A
                // page just kept is no longer one of them.
                val kept = relayKeep?.takeIf { it.chapter == event.chapter }?.credit?.fingerprint
                val credits = now.credits[event.chapter]?.filter { it.fingerprint != kept }
                val laid = now.copy(
                    leaves = event.leaves,
                    layout = now.layout + 1,
                    files = emptyMap(),
                    held = held,
                    credits = if (credits == null) now.credits else now.credits + (event.chapter to credits),
                    committed = null,
                )
                val placed = relaidWithKept(laid, now)
                if (placed.jump != null && placed.jump != now.jump) return run { _state.value = placed }
                val start = startAt?.also { startAt = null }
                // Flowed into from above: the scroll is already where the chapter starts.
                if (start != null && now.previous != null) return run { _state.value = placed }
                val leaf = when {
                    start != null -> placed.leafOf(start)
                    else -> shownPage?.let(placed::leafOf)
                }
                placed.copy(jump = leaf?.let { ReaderJump(it, start?.strip, ++ids) } ?: now.jump)
            }
            is ReaderEvent.PageReady -> {
                if (!ours(event.session) || event.chapter != now.chapter) return
                val key = PageKey(event.page, event.half)
                askedPages -= key
                now.copy(files = now.files + (key to event.file), failures = now.failures - event.page)
            }
            is ReaderEvent.PageFailed -> {
                if (!ours(event.session) || event.chapter != now.chapter) return
                now.copy(failures = now.failures + (event.page to event.failure))
            }
            is ReaderEvent.NovelOpen -> {
                if (!ours(event.v1.session)) return
                now.copy(novel = event.v1)
            }
            is ReaderEvent.NovelStyle -> if (ours(event.session)) now.copy(novelStyle = event) else return
            is ReaderEvent.ChapterRead -> if (event.work == now.view?.work) now.copy(read = now.read + event.chapter, readTrackers = now.readTrackers + (event.chapter to event.trackers)) else return
            is ReaderEvent.EndPage -> if (ours(event.session)) now.copy(end = EndPageView(event.next, event.whyNone, event.nextSource)) else return
            is ReaderEvent.NextPreview -> if (ours(event.session)) now.copy(preview = NextPreview(event.chapter, event.file)) else return
            // Only while opening: a chapter opened from another one keeps its own page up meanwhile.
            is ReaderEvent.Opening -> if (now.opening) now.copy(openingChapter = event.chapter ?: now.openingChapter, openingWait = event.wait) else return
            is ReaderEvent.Notice -> now.copy(notice = ShownNotice(event.v1, ++ids))
            is ReaderEvent.Moved -> if (ours(event.session)) now.copy(moved = event) else return
            is ReaderEvent.Settings -> {
                val work = now.view?.work
                if (now.view != null && event.v1.work != null && event.v1.work != work) return
                // A mode or direction changed in the reader lays the chapter out again without
                // opening it anew: the view follows, so the tap zones and the scrubber turn with it.
                // Only a change counts: the engine's own layout (with any guess) stands until one.
                val before = now.settings
                val changed = before != null && before.medium == event.v1.medium &&
                    listOf(ReaderSettingKey.MODE, ReaderSettingKey.DIRECTION).any { key ->
                        before.rows.firstOrNull { it.key == key }?.value != event.v1.rows.firstOrNull { it.key == key }?.value
                    }
                now.copy(settings = event.v1, view = if (changed) now.view?.following(event.v1, surface?.eink == true) else now.view)
            }
            is ReaderEvent.GoTo -> {
                if (!ours(event.session) || event.place.chapter != now.chapter) return
                val pos = event.place.novel
                if (pos != null && now.novel != null) return run { _state.value = now.copy(novelGoTo = NovelGoTo(pos, ++ids)) }
                now.copy(jump = ReaderJump(now.leafOf(event.place), event.place.strip, ++ids))
            }
            is ReaderEvent.BackToPage -> if (ours(event.session)) now.copy(backTo = event.page) else return
            is ReaderEvent.Closed -> if (ours(event.session)) now.copy(closed = true) else return
            // The page between chapters does not show the server's answer yet.
            is ReaderEvent.ProgressSent -> return
            is ReaderEvent.CreditPages -> {
                if (!ours(event.session)) return
                // A page brought back that is no longer skipped is no longer on screen as a credit page.
                val off = event.chapter == now.chapter && now.onCredit != null && event.pages.none { it.page == now.onCredit }
                now.copy(credits = now.credits + (event.chapter to event.pages), onCredit = if (off) null else now.onCredit)
            }
            is ReaderEvent.ChapterParts -> {
                if (!ours(event.session) || event.chapter != now.chapter) return
                now.copy(parts = ReaderParts(event.parts, event.total, event.missing))
            }
        }
    }

    private fun reader(command: ReaderCommand) {
        send(Command.Reader(command))
    }
}

//! The novel side of a session: books opened off the actor and served to the web view, the
//! paginator's reports, and sources' chapters.
//!
//! An EPUB is opened on a thread of its own (the archive's limits bound the work, so there is
//! no timeout); a source's chapter is fetched on the runtime. Either way the result lands in
//! the session's slot, tagged with the layout's generation, and a [`ReaderWake::Prepared`]
//! brings it to the actor. A book of pictures goes on to the page worker as manga does; a text
//! book is put in [`NovelBooks`](crate::reading::novel::NovelBooks) and the app is sent
//! `NovelOpen`.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use tokio::time::Instant;

use kaeri_books::Limits;
use kaeri_core::books::{ChapterContent, ChapterId, WorkId};
use kaeri_model::{
    ReadingMsg,
    eink::NovelSupport,
    event::Event,
    reader::{
        NovelOpenView, NovelPos, NovelSpineItem, NovelTocEntry, PageEncoding, PageFile,
        PixelFormat, ReaderAction, ReaderCommand, ReaderEvent, ReaderMode,
        ReaderSettingKey as Key, TurnHow,
    },
    reading::{ReadingAbout, ReadingEvent, ReadingFailure, VolumeGroup},
};
use kaeri_pages::{Renderer, resize::Fit};

use super::{Cx, RESUMED_MS, Session, notice, place};
use crate::{
    outbox::{Internal, Outbox, ReaderWake},
    reading::{
        novel::{self as served, EinkImages, NovelBook, Opened, Opening, ServedBook},
        pages::{Pages, PeekJob},
        shelf::ShelfChapter,
    },
};

/// What a chapter opened off the actor gave, by the layout's generation.
pub(super) type Slot = Arc<Mutex<Option<(u64, Prepared)>>>;

#[derive(Debug)]
pub(super) enum Prepared {
    /// Pictures: read through the page worker from this file.
    Pages(PathBuf),
    Novel(Box<NovelBook>),
    Failed(ReadingFailure),
}

/// The novel open in the web view.
#[derive(Debug)]
pub(super) struct Open {
    pub chapter: ChapterId,
    pub book: Arc<ServedBook>,
    pub identity: String,
    pub style: String,
    pub count_key: String,
    pub counted: Vec<u32>,
    pub fixed: bool,
    pub cjk: bool,
    pub render_dir: Option<PathBuf>,
    pub first_linear: u32,
    pub last_linear: u32,
    pub spine_len: u32,
    /// Each spine document's words.
    pub words: Vec<u32>,
    /// The person's pace in words a minute.
    pub pace: u32,
    /// The last page turned to, and when: a turn to the next page teaches the pace.
    pub turned: Option<(Instant, u32, u32)>,
}

/// Where the person's reading pace is kept, in words a minute.
const PACE: &str = "read:pace";
/// The pace until page turns teach the person's own: an adult reading fiction.
pub(crate) const DEFAULT_PACE: u32 = 230;
/// A page read faster or slower than this is a skim or a pause, and teaches nothing.
const PAGE_TIME: std::ops::RangeInclusive<Duration> =
    Duration::from_secs(3)..=Duration::from_secs(600);

/// The person's pace, as page turns taught it.
pub(super) fn pace(store: &kaeri_store::Store) -> u32 {
    store
        .get_meta_text(PACE)
        .ok()
        .flatten()
        .and_then(|t| t.trim().parse::<u32>().ok())
        .filter(|p| (50..=1_500).contains(p))
        .unwrap_or(DEFAULT_PACE)
}

/// The pace after a page of `words` read in `took`: a slow-moving average, so one page read
/// while distracted barely moves it. `None` for a page that teaches nothing.
pub(super) fn learnt(pace: u32, words: u32, took: Duration) -> Option<u32> {
    if words == 0 || !PAGE_TIME.contains(&took) {
        return None;
    }
    let sample = (f64::from(words) * 60.0 / took.as_secs_f64()).clamp(50.0, 1_500.0);
    let next = (f64::from(pace) * 0.85 + sample * 0.15).round() as u32;
    Some(next.clamp(50, 1_500))
}

impl Open {
    /// The page in the whole volume, when the hidden web view counted it; else in the document.
    fn volume_page(&self, spine: u32, page: u32) -> u32 {
        if self.counted.len() as u32 != self.spine_len {
            return page;
        }
        self.counted.iter().take(spine as usize).sum::<u32>() + page
    }

    fn total(&self) -> u32 {
        if self.counted.len() as u32 == self.spine_len { self.counted.iter().sum() } else { 0 }
    }
}

/// Whether a chapter opens off the actor before it is laid out.
pub(super) fn prepares(chapter: &ShelfChapter) -> bool {
    chapter.format == "epub" || chapter.format == SOURCE
}

/// The folder a session's chapters of pictures from a source are fetched into, one folder per
/// layout. It goes when the session closes.
pub(super) fn streamed_dir(cache: &Path, session: u64) -> PathBuf {
    cache_dir(cache).join(format!("{session}-pages"))
}

/// Fetch every page of a source's chapter of pictures into `dir`, named in reading order with
/// the type their bytes are, so the page worker opens the folder as a book.
async fn fetch_pages(
    sources: &crate::read_sources::BookSources,
    opened: &mut crate::read_sources::Opened,
    dir: &Path,
    now: i64,
) -> Prepared {
    let ChapterContent::Images(pages) = &opened.content else {
        return Prepared::Failed(ReadingFailure::NotFound);
    };
    let total = pages.len();
    if total == 0 {
        return Prepared::Failed(ReadingFailure::NotFound);
    }
    let io = |e: std::io::Error| ReadingFailure::Other { reason: e.to_string() };
    if let Err(e) = tokio::fs::create_dir_all(dir).await {
        return Prepared::Failed(io(e));
    }
    for index in 0..total {
        let bytes = match sources.page(opened, index, now).await {
            Ok(bytes) => bytes,
            Err(failure) => return Prepared::Failed(failure),
        };
        let Some(ext) = page_extension(&bytes) else {
            return Prepared::Failed(ReadingFailure::Unsupported { extension: "page".into() });
        };
        if let Err(e) =
            tokio::fs::write(dir.join(format!("{:05}.{ext}", index + 1)), &bytes).await
        {
            return Prepared::Failed(io(e));
        }
    }
    Prepared::Pages(dir.to_owned())
}

/// The extension a page's bytes are of, by their first bytes, for the page worker.
fn page_extension(bytes: &[u8]) -> Option<&'static str> {
    let ext = kaeri_books::mime::sniff_image(bytes)?.strip_prefix("image/")?;
    Some(if ext == "jpeg" { "jpg" } else { ext })
}

/// Make the end page's preview of the next chapter, one from a source, as a shelf chapter's is
/// made: its first page alone is fetched, off the actor, into a folder of the session that the
/// page worker opens as a book. Nothing waits on it. A chapter of text, or one only a whole
/// file's download would open, has no preview, and neither has one that fails.
pub(super) fn peek_source(
    cx: &Cx<'_>,
    pages: &Pages,
    session: u64,
    work: &WorkId,
    next: (usize, &ShelfChapter),
) {
    let (index, next) = next;
    let sources = cx.services.book_sources.clone();
    let work = work.clone();
    let chapter = next.id.clone();
    let now = cx.now();
    let dir = streamed_dir(&cx.services.paths.cache_dir, session).join(format!("next-{index}"));
    let pages = pages.clone();
    let out = cx.tx.clone();
    tokio::spawn(async move {
        let bytes = match sources.first_page(&work, &chapter, now).await {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return,
            Err(failure) => {
                tracing::debug!(?failure, "no preview of the next chapter");
                return;
            }
        };
        let Some(ext) = page_extension(&bytes) else { return };
        if tokio::fs::create_dir_all(&dir).await.is_err()
            || tokio::fs::write(dir.join(format!("00001.{ext}")), &bytes).await.is_err()
        {
            return;
        }
        pages.peek(PeekJob { session, chapter, path: dir, out });
    });
}

/// The format of a chapter read from a source rather than a file on the shelf.
pub(crate) const SOURCE: &str = "source";

/// The folder novels' files are written to, under the cache.
pub(super) fn cache_dir(cache: &Path) -> PathBuf {
    cache.join("novel")
}

/// Tells the actor the work is done. It holds the sender it was lent until then: the engine
/// keeps none between one piece of work and the next, so a weak one could be closed by the
/// time the book is open.
fn wake(tx: &Outbox, session: u64, wake: ReaderWake) -> impl Fn() + Send + 'static {
    let tx = tx.clone();
    move || {
        let _ = tx.internal(Internal::Reader { session, wake });
    }
}

fn failed(cx: &Cx<'_>, failure: ReadingFailure) {
    let _ = cx
        .tx
        .send(Event::Reading(ReadingEvent::Failed { about: ReadingAbout::Reader, failure }));
}

/// Opens a book file: a novel, or pictures for the page worker.
fn open_file(path: &Path, opening: Opening) -> Prepared {
    let epub = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("epub"));
    if !epub {
        return Prepared::Pages(path.to_owned());
    }
    match served::open_epub(path, opening) {
        Ok(Opened::Pages) => Prepared::Pages(path.to_owned()),
        Ok(Opened::Novel(book)) => Prepared::Novel(Box::new(book)),
        Err(failure) => Prepared::Failed(failure),
    }
}

/// The meta key a chapter's whole-volume counts are kept under.
fn counts_key(work: &WorkId, chapter: &ChapterId) -> String {
    format!("novel-pages:{}/{}", work.as_str(), chapter.as_str())
}

/// The counts kept under `key`, written as `<key> n,n,n`. Only the newest key is kept.
fn kept_counts(cx: &Cx<'_>, work: &WorkId, chapter: &ChapterId, key: &str) -> Vec<u32> {
    let text = cx.services.store.get_meta_text(&counts_key(work, chapter)).ok().flatten();
    let Some((kept, counts)) = text.as_deref().and_then(|t| t.split_once(' ')) else {
        return Vec::new();
    };
    if kept != key {
        return Vec::new();
    }
    counts.split(',').map(str::parse).collect::<Result<_, _>>().unwrap_or_default()
}

/// The place a person left a novel chapter at, when not read through.
pub(super) fn kept_novel(cx: &Cx<'_>, work: &WorkId, chapter: &ChapterId) -> Option<NovelPos> {
    cx.services
        .store
        .read_chapters(work.as_str())
        .ok()?
        .into_iter()
        .find(|k| k.chapter == chapter.as_str() && !k.read)?
        .novel_pos
        .as_deref()
        .and_then(NovelPos::decode)
}

/// A source's chapters as the reader orders them.
pub(super) fn source_chapters(groups: Vec<VolumeGroup>) -> Vec<ShelfChapter> {
    groups
        .into_iter()
        .flat_map(|g| g.chapters)
        .map(|row| ShelfChapter {
            id: row.id,
            label: row.title.clone().filter(|t| !t.is_empty()).unwrap_or(row.label),
            title: None,
            volume: row.volume,
            path: PathBuf::new(),
            format: SOURCE.into(),
            pages: row.pages,
            comicinfo: None,
            source: None,
        })
        .collect()
}

/// A work with nothing on the shelf, listed from its source off the actor.
pub(super) type Listed = Arc<Mutex<Option<Result<Vec<ShelfChapter>, ReadingFailure>>>>;

pub(super) fn list(cx: &Cx<'_>, session: u64, work: &WorkId) -> Option<Listed> {
    let refs = cx.services.store.book_refs_for_work(work.as_str()).unwrap_or_default();
    if refs.is_empty() {
        return None;
    }
    let slot = Listed::default();
    let out = slot.clone();
    let sources = cx.services.book_sources.clone();
    let work = work.clone();
    let now = cx.now();
    let done = wake(cx.tx, session, ReaderWake::Listed);
    tokio::spawn(async move {
        let listed = sources.chapters(&work, None, now).await.map(|(_, g)| source_chapters(g));
        // The listing asked the source how the series reads, if it had not said lately.
        if let Some(lookup) = sources.hint_lookup(&work) {
            crate::read_sources::settle_hint(lookup).await;
        }
        if let Ok(mut s) = out.lock() {
            *s = Some(listed);
        }
        done();
    });
    Some(slot)
}

/// A work on the shelf whose source is being asked how it reads, with no answer kept: its open
/// waits a moment for the answer, then goes on with what is kept.
pub(super) fn after_hint(
    cx: &Cx<'_>,
    session: u64,
    work: &WorkId,
    chapters: &[ShelfChapter],
) -> Option<Listed> {
    let lookup = cx.services.book_sources.hint_lookup(work)?;
    let slot = Listed::default();
    let out = slot.clone();
    let chapters = chapters.to_vec();
    let done = wake(cx.tx, session, ReaderWake::Listed);
    tokio::spawn(async move {
        crate::read_sources::settle_hint(lookup).await;
        if let Ok(mut s) = out.lock() {
            *s = Some(Ok(chapters));
        }
        done();
    });
    Some(slot)
}

impl Session {
    fn opening(&self, cx: &Cx<'_>) -> Opening {
        let plan = self.plan(cx);
        Opening {
            session: self.id,
            soft_hyphens: kaeri_books::novel::HYPHENATES,
            dir: cache_dir(&cx.services.paths.cache_dir).join(self.id.to_string()),
            eink: self
                .surface
                .eink
                .then_some(EinkImages { target: plan.target, look: plan.look }),
            limits: Limits::default(),
        }
    }

    /// What the web view this app runs in can do with a novel.
    fn support(&self, cx: &Cx<'_>) -> NovelSupport {
        match cx.webview {
            Some(version) => NovelSupport::of_webview(version),
            None => cx.eink.as_ref().map_or(NovelSupport::Full, |p| p.novels),
        }
    }

    /// Opens the chapter off the actor: an EPUB on a thread, a source's chapter on the runtime.
    pub(super) fn prepare(&mut self, cx: &Cx<'_>, chapter: &ShelfChapter) {
        let generation = self.generation;
        let slot = self.prepared.clone();
        let opening = self.opening(cx);
        let done = wake(cx.tx, self.id, ReaderWake::Prepared(generation));
        // Opens can finish out of order: one for an older layout never replaces a newer one.
        let put = move |p: Prepared| {
            if let Ok(mut s) = slot.lock()
                && s.as_ref().is_none_or(|(g, _)| *g < generation)
            {
                *s = Some((generation, p));
            }
            done();
        };
        if chapter.format != SOURCE {
            let path = chapter.path.clone();
            std::thread::spawn(move || put(open_file(&path, opening)));
            return;
        }
        let sources = cx.services.book_sources.clone();
        let work = self.work.clone();
        let id = chapter.id.clone();
        let label = chapter.label.clone();
        let now = cx.now();
        let streamed =
            streamed_dir(&cx.services.paths.cache_dir, self.id).join(generation.to_string());
        tokio::spawn(async move {
            let prepared = match sources.open(&work, &id, None, now).await {
                Err(failure) => Prepared::Failed(failure),
                // A source's pictures are fetched into a folder the page worker reads as a book.
                Ok(mut opened) if matches!(opened.content, ChapterContent::Images(_)) => {
                    fetch_pages(&sources, &mut opened, &streamed, now).await
                }
                Ok(opened) => match opened.content {
                    ChapterContent::Text(xhtml) => {
                        let title = opened.chapter.title.clone().unwrap_or(label);
                        let identity = format!("{}/{}", opened.source_id, opened.chapter.id);
                        let lang = opened.chapter.language.clone();
                        Prepared::Novel(Box::new(served::open_text(
                            &xhtml, &title, lang, identity, opening,
                        )))
                    }
                    ChapterContent::Book(file) => {
                        match tokio::task::spawn_blocking(move || {
                            open_file(&file.path, opening)
                        })
                        .await
                        {
                            Ok(p) => p,
                            Err(e) => Prepared::Failed(ReadingFailure::Other {
                                reason: e.to_string(),
                            }),
                        }
                    }
                    ChapterContent::Images(_) => Prepared::Failed(ReadingFailure::NotFound),
                },
            };
            put(prepared);
        });
    }

    /// What was opened off the actor arrived.
    pub(super) fn prepared_now(&mut self, cx: &mut Cx<'_>, pages: &Pages, generation: u64) {
        if generation != self.generation {
            return;
        }
        // Only this layout's open is taken; anything else in the slot is left as it is.
        let taken =
            self.prepared.lock().ok().and_then(|mut s| s.take_if(|(g, _)| *g == generation));
        let Some((_, prepared)) = taken else { return };
        match prepared {
            Prepared::Failed(failure) => failed(cx, failure),
            Prepared::Pages(path) => {
                let chapter = &mut self.chapters[self.at];
                chapter.path = path;
                chapter.format = "pages".into();
                let page = self.start.page.unwrap_or(0);
                self.lay_out(cx, pages, page);
            }
            Prepared::Novel(book) => self.novel_opened(cx, *book),
        }
    }

    fn novel_style(&self, cx: &Cx<'_>, fixed: bool, cjk: bool) -> String {
        served::style(
            &self.layers,
            &self.surface,
            cx.screen.handheld,
            self.support(cx),
            fixed,
            cjk,
        )
    }

    fn novel_opened(&mut self, cx: &mut Cx<'_>, book: NovelBook) {
        let support = self.support(cx);
        if support == NovelSupport::Refused {
            let version = cx.webview.unwrap_or(0);
            failed(cx, ReadingFailure::WebViewTooOld { version });
            return;
        }
        let cjk = book.served.cjk();
        let style = self.novel_style(cx, book.fixed, cjk);
        book.served.set_style(style.clone());
        cx.services.novels.put(book.served.clone());
        let chapter = self.chapter().id.clone();
        let count_key = served::count_key(&book.identity, &style, &self.surface);
        let counted = kept_counts(cx, &self.work, &chapter, &count_key);
        let spine_len = book.spine.len() as u32;
        let restore = self.restore.take().filter(|p| p.spine < spine_len);
        let novels = &cx.services.novels;
        let first = restore.as_ref().map_or(book.first_linear(), |p| p.spine);
        let mut entry = novels.url(self.id, &book.spine[first as usize].0);
        if let Some(pos) = &restore {
            entry.push_str("#kaeri:pos=");
            entry.push_str(&pos.encode());
        }
        let render_dir = (self.surface.eink && book.fixed).then(|| {
            let dir =
                cache_dir(&cx.services.paths.cache_dir).join(format!("{}-fixed", self.id));
            let _ = std::fs::create_dir_all(&dir);
            dir
        });
        let view = NovelOpenView {
            session: self.id,
            chapter: chapter.clone(),
            entry,
            writing_mode: book.writing_mode,
            progression: book.progression,
            restore: restore.clone(),
            fixed_layout: book.fixed,
            support,
            spine: book
                .spine
                .iter()
                .enumerate()
                .map(|(i, (path, linear))| NovelSpineItem {
                    url: novels.url(self.id, path),
                    linear: *linear,
                    words: book.words.get(i).copied().unwrap_or(0),
                })
                .collect(),
            toc: book
                .toc
                .iter()
                .map(|(label, href, spine, depth)| {
                    let (path, fragment) = match href.split_once('#') {
                        Some((p, f)) => (p, Some(f)),
                        None => (href.as_str(), None),
                    };
                    let mut url = novels.url(self.id, path);
                    if let Some(f) = fragment.filter(|f| !f.is_empty()) {
                        url.push('#');
                        url.push_str(f);
                    }
                    NovelTocEntry { label: label.clone(), url, spine: *spine, depth: *depth }
                })
                .collect(),
            style: style.clone(),
            count_key: count_key.clone(),
            counted: counted.clone(),
            render_dir: render_dir.as_ref().map(|d| d.display().to_string()),
            words_per_minute: pace(&cx.services.store),
        };
        self.mode = if self.layers.word(Key::Mode) == "scrolled" && !self.surface.eink {
            ReaderMode::Scrolled
        } else {
            ReaderMode::Paged
        };
        self.direction = book.progression;
        self.novel = Some(Open {
            chapter,
            first_linear: book.first_linear(),
            last_linear: book.last_linear(),
            book: book.served,
            identity: book.identity,
            style,
            count_key,
            counted,
            fixed: book.fixed,
            cjk,
            render_dir,
            spine_len,
            pace: view.words_per_minute,
            words: book.words,
            turned: None,
        });
        self.pages_total = self.novel.as_ref().map_or(0, Open::total);
        // The resumed notice waits for the first page, which says the page number.
        self.novel_resumed = restore.is_some() && self.resumed.take().is_some();
        if !self.opened {
            self.opened = true;
            cx.send(ReaderEvent::Opened(Box::new(self.view(cx))));
            self.downloads_follow(cx, false);
        }
        cx.send(ReaderEvent::NovelOpen(view));
    }

    /// The settings or the screen changed: the paginator is told, and keeps its place.
    pub(super) fn restyle(&mut self, cx: &Cx<'_>) {
        let style = match &self.novel {
            Some(open) => self.novel_style(cx, open.fixed, open.cjk),
            None => return,
        };
        let surface = self.surface;
        let Some(open) = self.novel.as_mut() else { return };
        let count_key = served::count_key(&open.identity, &style, &surface);
        if style == open.style && count_key == open.count_key {
            return;
        }
        open.book.set_style(style.clone());
        open.style = style.clone();
        if count_key != open.count_key {
            open.counted = kept_counts(cx, &self.work, &open.chapter, &count_key);
            open.count_key = count_key.clone();
        }
        let counted = open.counted.clone();
        self.pages_total = open.total();
        cx.send(ReaderEvent::NovelStyle { session: self.id, style, count_key, counted });
    }

    /// The paginator's reports.
    pub(super) fn novel_report(&mut self, cx: &mut Cx<'_>, command: ReaderCommand) {
        match command {
            ReaderCommand::NovelAt { chapter, pos, page, pages, how, .. } => {
                if self.novel.as_ref().is_some_and(|o| o.chapter == chapter) {
                    self.novel_at(cx, pos, page, pages, how);
                }
            }
            ReaderCommand::NovelCounted { key, chapter_pages, .. } => {
                let Some(open) = self.novel.as_mut() else { return };
                if key != open.count_key || chapter_pages.len() as u32 != open.spine_len {
                    return;
                }
                let line =
                    chapter_pages.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
                let meta = counts_key(&self.work, &open.chapter);
                if let Err(e) = cx.services.store.set_meta_text(&meta, &format!("{key} {line}"))
                {
                    tracing::debug!(error = %e, "a novel's page counts were not kept");
                }
                open.counted = chapter_pages;
                self.pages_total = open.total();
            }
            ReaderCommand::FixedRendered { chapter, page, path, .. } => {
                self.fixed_rendered(cx, chapter, page, &path)
            }
            _ => {}
        }
    }

    fn novel_at(
        &mut self,
        cx: &mut Cx<'_>,
        pos: NovelPos,
        page: u32,
        pages: u32,
        how: TurnHow,
    ) {
        if self.novel.is_none() {
            return;
        }
        if !matches!(how, TurnHow::Jump | TurnHow::Placed) {
            self.drop_back(cx);
        }
        let Some(open) = self.novel.as_mut() else { return };
        if pos.spine >= open.spine_len {
            return;
        }
        // A turn to the next page of the same document teaches the pace.
        let now = Instant::now();
        if !matches!(how, TurnHow::Jump | TurnHow::Placed)
            && pages > 0
            && let Some((at, spine, before)) = open.turned
            && spine == pos.spine
            && page == before + 1
        {
            let words = open.words.get(pos.spine as usize).copied().unwrap_or(0) / pages;
            if let Some(pace) = learnt(open.pace, words, now.duration_since(at))
                && pace != open.pace
            {
                open.pace = pace;
                if let Err(e) = cx.services.store.set_meta_text(PACE, &pace.to_string()) {
                    tracing::debug!(error = %e, "the reading pace was not kept");
                }
            }
        }
        open.turned = Some((now, pos.spine, page));
        let open = &*open;
        let last_doc = pos.spine >= open.last_linear;
        if pages > 0 && page >= pages {
            // The end page: read at once.
            self.chapter_read(cx, super::Reached::MovedOn);
            self.say_end_page(cx);
            return;
        }
        let shown = open.volume_page(pos.spine, page);
        let mut here = place(&self.work, &open.chapter, Some(shown));
        here.novel = Some(pos);
        if how == TurnHow::Jump {
            self.jumped(cx, &here);
        }
        self.place = Some(here);
        self.note_place(cx);
        // Nothing to start over from at the chapter's first page.
        if std::mem::take(&mut self.novel_resumed) && shown > 0 {
            let msg = ReadingMsg::ReaderResumed { page: i64::from(shown) + 1 };
            cx.send(notice(msg, Some(ReaderAction::StartOver), RESUMED_MS, self.eink()));
        }
        if last_doc && page + 1 >= pages {
            self.say_end_page(cx);
            self.wait_to_read(cx);
        }
    }

    /// The first page of the book's reading flow.
    pub(super) fn novel_start(&self) -> Option<NovelPos> {
        let open = self.novel.as_ref()?;
        Some(NovelPos { spine: open.first_linear, path: Vec::new(), offset: 0 })
    }

    /// E-ink, fixed layout: the app's render of a page, processed for the panel off the actor.
    fn fixed_rendered(&mut self, cx: &Cx<'_>, chapter: ChapterId, page: u32, path: &str) {
        let Some(open) = self.novel.as_ref().filter(|o| o.chapter == chapter) else { return };
        let Some(dir) = open.render_dir.clone() else { return };
        // Only the file the engine named is read.
        if Path::new(path) != dir.join(format!("{page}.png")) {
            tracing::debug!(page, "a fixed-layout render outside its folder was ignored");
            return;
        }
        let plan = self.plan(cx);
        let session = self.id;
        let tx = cx.tx.clone();
        let path = PathBuf::from(path);
        std::thread::spawn(move || {
            let event = match process_fixed(&path, &dir, page, plan.target, plan.look) {
                Ok(file) => ReaderEvent::PageReady { session, chapter, page, half: None, file },
                Err(failure) => ReaderEvent::PageFailed { session, chapter, page, failure },
            };
            let _ = tx.send(Event::Reader(event));
        });
    }
}

fn process_fixed(
    path: &Path,
    dir: &Path,
    page: u32,
    target: kaeri_pages::Target,
    look: kaeri_pages::Look,
) -> Result<PageFile, ReadingFailure> {
    let other = |e: &dyn std::fmt::Display| ReadingFailure::Other { reason: e.to_string() };
    let bytes = std::fs::read(path).map_err(|e| other(&e))?;
    let mut renderer = Renderer::new(target, look);
    let source = renderer.open(&bytes).map_err(|e| other(&e))?;
    let fit = Fit::Within { width: target.width.max(1), height: target.height.max(1) };
    let scaled = renderer.scale(&source, None, fit, false).map_err(|e| other(&e))?;
    let raw = renderer.finish(scaled, source.colour).map_err(|e| other(&e))?;
    let out = dir.join(format!("{page}.raw"));
    let temp = dir.join(format!(".{page}.raw.tmp"));
    std::fs::write(&temp, raw.bytes()).map_err(|e| other(&e))?;
    std::fs::rename(&temp, &out).map_err(|e| other(&e))?;
    let format = match raw.format() {
        kaeri_pages::PixelFormat::Gray8 => PixelFormat::Gray8,
        _ => PixelFormat::Rgba8,
    };
    Ok(PageFile {
        path: out.display().to_string(),
        encoding: PageEncoding::Raw { format, stride: raw.stride() },
        width: raw.width(),
        height: raw.height(),
        crop: None,
        colour: raw.colour,
        bytes: raw.bytes().len() as u64,
    })
}

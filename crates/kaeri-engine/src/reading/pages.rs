//! The page service: one worker thread that opens a chapter's book, lays its pages out, and
//! writes each page the reader shows into the page cache, sending [`ReaderEvent::PageReady`].
//!
//! Pages cross as files, never as bytes. Every file is written under a temporary name and
//! renamed, so a path the app is given is always whole. The current leaf is made first, then
//! the ones around it once it has been painted. A book is opened on its own thread with a time
//! budget: a crafted PDF can take many seconds to open, and only one such open runs at a time.

use std::{
    collections::{HashSet, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use kaeri_books::{Book, Limits};
use kaeri_core::books::ChapterId;
use kaeri_model::{
    event::Event,
    reader::{
        CreditPage, Leaf, LeafPart, PageEncoding, PageFile, PageHalf, PageKind, PixelFormat,
        PixelRect, ReadDirection, ReaderEvent, ReaderMode, StripPos, UnitRect,
    },
    reading::ReadingFailure,
};
use kaeri_pages::{
    Bitmap, Delivery, Look, Renderer, SourceFormat, Target,
    layout::{self, Direction, LayoutOptions, PageShape, Screen, SpreadOnSingle},
    resize::{Fit, resize},
    strip::{StripCut, StripOptions},
};

use crate::{
    host::PageDecoder,
    outbox::{Internal, Outbox, ReaderWake},
    reading::feed::{Feed, FeedPage},
};

/// How long a book may take to open before the chapter fails.
pub const OPEN_BUDGET: Duration = Duration::from_secs(8);
/// How long one PDF page may take to render. The PDF reader cannot be stopped, so a page over
/// this fails the chapter and the render is left to finish on its own thread.
pub const PDF_PAGE_BUDGET: Duration = Duration::from_secs(2);
/// How long the platform's decoder may take over one page it was asked to decode. It cannot be
/// stopped, so a page over this fails and the call is left to finish on its own thread, which
/// holds the decoder until it ends.
pub const HOST_DECODE_BUDGET: Duration = Duration::from_secs(2);

/// What the session decided before the book is open; the rest is decided from the book.
#[derive(Debug, Clone)]
pub struct Plan {
    pub target: Target,
    pub look: Look,
    /// Two pages side by side.
    pub two_pages: bool,
    pub spreads: SpreadOnSingle,
    /// The series' or the medium's mode and direction, and whether each may still be guessed
    /// from the book.
    pub mode: ReaderMode,
    pub direction: ReadDirection,
    pub guess_mode: bool,
    pub guess_direction: bool,
    /// Pages are fitted to the screen's width (the Fit setting's Width), to be scrolled.
    pub fit_width: bool,
}

/// What the book said, for the session: the layout and the guesses made.
#[derive(Debug, Clone, Default)]
pub struct Laid {
    /// The plan this layout answers; older ones are ignored.
    pub generation: u64,
    pub mode: Option<ReaderMode>,
    pub direction: Option<ReadDirection>,
    /// What was guessed from the book, to keep and show once.
    pub guessed_mode: bool,
    pub guessed_direction: bool,
    pub pages: u32,
    pub leaves: Vec<Leaf>,
    /// For a strip cut into screens on e-ink: where each leaf starts in the strip.
    pub cuts: Vec<StripPos>,
    /// Still extracting: more leaves may come.
    pub growing: bool,
    pub failure: Option<ReadingFailure>,
    /// Credit pages left out of `pages`, by their place in the chapter as fetched.
    pub credits: Vec<CreditPage>,
}

pub type Shared = Arc<Mutex<Laid>>;

/// A chapter to open.
#[derive(Debug)]
pub struct OpenJob {
    pub session: u64,
    pub generation: u64,
    pub chapter: ChapterId,
    pub path: PathBuf,
    pub plan: Plan,
    /// The page to make first.
    pub start_page: u32,
    pub shared: Shared,
    /// A way to answer, held while the worker has pages to make.
    pub out: Outbox,
    /// A source's chapter whose pages land in `path` as they are fetched: the chapter is laid
    /// out from the feed, and a page not landed yet is made once it lands.
    pub feed: Option<Arc<Feed>>,
    /// Find and leave out the series' credit pages, when the person has that on.
    pub credits: Option<super::credits::CreditCheck>,
}

#[derive(Debug)]
enum Job {
    Open(Box<OpenJob>),
    /// Make these leaves, in this order, replacing what was waiting.
    Want {
        session: u64,
        generation: u64,
        leaves: Vec<u32>,
        out: Outbox,
    },
    /// Make this page again: its file went missing.
    Again {
        session: u64,
        page: u32,
        out: Outbox,
    },
    Close {
        session: u64,
    },
    /// Make the first page of the next chapter, for the end page to show.
    Peek(Box<PeekJob>),
    /// A page of a feed landed, or failed.
    Landed {
        session: u64,
        chapter: ChapterId,
        page: u32,
        out: Outbox,
    },
}

/// The next chapter's first page, made while the reader nears the end of this one.
#[derive(Debug)]
pub struct PeekJob {
    pub session: u64,
    pub chapter: ChapterId,
    pub path: PathBuf,
    pub out: Outbox,
}

/// The handle the engine keeps: jobs go in, events come out through the outbox.
#[derive(Debug, Clone)]
pub struct Pages {
    jobs: mpsc::Sender<Job>,
}

impl Pages {
    /// The page worker, with the host's `decoder` for the formats it cannot read, when it has one.
    pub fn start(cache: PathBuf, decoder: Option<Arc<dyn PageDecoder>>) -> Self {
        let (jobs, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("kaeri-pages".into())
            .spawn(move || Worker::new(cache, HostDecoder::new(decoder)).run(rx));
        if let Err(e) = spawned {
            tracing::error!(error = %e, "the page worker did not start");
        }
        Self { jobs }
    }

    pub fn open(&self, job: OpenJob) {
        let _ = self.jobs.send(Job::Open(Box::new(job)));
    }

    pub fn want(&self, session: u64, generation: u64, leaves: Vec<u32>, out: &Outbox) {
        let _ = self.jobs.send(Job::Want { session, generation, leaves, out: out.clone() });
    }

    pub fn again(&self, session: u64, page: u32, out: &Outbox) {
        let _ = self.jobs.send(Job::Again { session, page, out: out.clone() });
    }

    pub fn close(&self, session: u64) {
        let _ = self.jobs.send(Job::Close { session });
    }

    pub fn peek(&self, job: PeekJob) {
        let _ = self.jobs.send(Job::Peek(Box::new(job)));
    }

    /// A page of `chapter`'s feed landed, or failed.
    pub fn landed(&self, session: u64, chapter: ChapterId, page: u32, out: &Outbox) {
        let _ = self.jobs.send(Job::Landed { session, chapter, page, out: out.clone() });
    }
}

/// The host's decoder for page formats the engine cannot read, called with a time budget.
#[derive(Clone)]
struct HostDecoder {
    decoder: Option<Arc<dyn PageDecoder>>,
    /// A call still running: one at a time, so a decoder that hangs costs one thread.
    busy: Arc<AtomicBool>,
    budget: Duration,
}

impl HostDecoder {
    fn new(decoder: Option<Arc<dyn PageDecoder>>) -> Self {
        Self { decoder, busy: Arc::new(AtomicBool::new(false)), budget: HOST_DECODE_BUDGET }
    }

    /// The page in RGBA8 inside `room`, or why not: no decoder or a refusal is unsupported, and
    /// a call over its budget or an answer that does not add up is unreadable.
    fn decode(&self, bytes: Vec<u8>, format: SourceFormat, room: (u32, u32)) -> Hosted {
        let Some(decoder) = self.decoder.clone() else { return Hosted::Unsupported };
        if self.busy.swap(true, Ordering::AcqRel) {
            tracing::warn!("the host's decoder is still on an earlier page");
            return Hosted::Unreadable;
        }
        let (tx, rx) = mpsc::channel();
        let busy = self.busy.clone();
        let mime = format.mime().to_owned();
        let spawned =
            std::thread::Builder::new().name("kaeri-page-decode".into()).spawn(move || {
                let image = decoder.decode(bytes, mime, room.0, room.1);
                busy.store(false, Ordering::Release);
                let _ = tx.send(image);
            });
        if spawned.is_err() {
            self.busy.store(false, Ordering::Release);
            return Hosted::Unreadable;
        }
        match rx.recv_timeout(self.budget) {
            Ok(Some(image)) => Hosted::Image(image),
            Ok(None) => Hosted::Unsupported,
            Err(_) => {
                tracing::warn!("the host's decoder took too long over a page");
                Hosted::Unreadable
            }
        }
    }

    /// As [`Self::decode`], as a bitmap the pipeline works on.
    fn bitmap(&self, bytes: Vec<u8>, format: SourceFormat, room: (u32, u32)) -> Hosted<Bitmap> {
        match self.decode(bytes, format, room) {
            Hosted::Image(image) => {
                match kaeri_pages::decode::from_rgba(
                    image.width,
                    image.height,
                    &image.rgba,
                    kaeri_pages::DecodeLimits::default(),
                ) {
                    Ok(bitmap) => Hosted::Image(bitmap),
                    Err(e) => {
                        tracing::warn!(error = %e, "the host's decoder gave an unusable page");
                        Hosted::Unreadable
                    }
                }
            }
            Hosted::Unsupported => Hosted::Unsupported,
            Hosted::Unreadable => Hosted::Unreadable,
        }
    }
}

enum Hosted<T = crate::host::DecodedImage> {
    Image(T),
    Unsupported,
    Unreadable,
}

/// The size to ask the host to decode a `width` by `height` page down to: at least twice the
/// largest it is ever drawn (whole, turned, or one half of a spread), as the JPEG decode keeps,
/// so the resize after does the last shrink. Never larger than the page.
fn decode_room(width: u32, height: u32, screen: (u32, u32)) -> (u32, u32) {
    let (w, h) = (width.max(1) as f64, height.max(1) as f64);
    let (rw, rh) = (screen.0.max(1) as f64, screen.1.max(1) as f64);
    let whole = (rw / w).min(rh / h);
    let turned = (rh / w).min(rw / h);
    let half = if w > h { (2.0 * rw / w).min(rh / h) } else { 0.0 };
    let scale = (whole.max(turned).max(half) * 2.0).min(1.0);
    ((w * scale).ceil() as u32, (h * scale).ceil() as u32)
}

/// One page's original, extracted into the cache.
#[derive(Debug, Clone)]
struct Original {
    path: PathBuf,
    format: Option<SourceFormat>,
    width: u32,
    height: u32,
}

/// A strip scaled to the screen's width, for cutting into screens on e-ink.
#[derive(Debug)]
struct Strip {
    /// Each image's scaled file, its height and where it starts in the strip.
    images: Vec<(PathBuf, u32, u32)>,
    width: u32,
    format: kaeri_pages::PixelFormat,
    cuts: Vec<StripCut>,
}

/// The chapter the worker has open.
struct Chapter {
    session: u64,
    generation: u64,
    chapter: ChapterId,
    source: PathBuf,
    dir: PathBuf,
    renderer: Renderer,
    originals: Vec<Option<Original>>,
    /// Each leaf's parts as the layout made them, with their source rectangles.
    leaves: Vec<Vec<layout::LeafPart>>,
    strip: Option<Strip>,
    /// Parts already sent, by page and half.
    sent: HashSet<(u32, Option<PageHalf>)>,
    queue: VecDeque<u32>,
    /// Leaves an app asked for again and not yet made: a later wish keeps them queued.
    asked: HashSet<u32>,
    /// Pages made before the session sent the leaves, held until it asks for any: an app
    /// never hears of a page before the leaf it belongs to.
    held: Option<Vec<ReaderEvent>>,
    decoder: HostDecoder,
    /// The pages landing from a source, and how they were laid out before they had.
    feed: Option<Fed>,
    /// Each page is drawn as wide as its share of the screen however tall it is: on an LCD in
    /// the strip, vertical mode or Fit width.
    by_width: bool,
}

/// A chapter laid out from a feed before all its pages landed.
struct Fed {
    feed: Arc<Feed>,
    /// Leaves wanted whose pages had not landed yet: made once they do.
    waiting: HashSet<u32>,
    /// What the layout took, to lay the pages out again once their shapes are all known.
    kinds: Vec<layout::PageKind>,
    options: Option<LayoutOptions>,
    /// Some pages' shapes were guessed from the first: the layout is checked once all land.
    guessed: bool,
    /// An e-ink strip, shown image by image until every image is there to cut.
    eink_strip: bool,
}

struct Worker {
    cache: PathBuf,
    /// The engine's way to answer, lent with each job and let go once the queue is empty, so
    /// an engine with nothing outstanding can go idle.
    out: Option<Outbox>,
    open: Option<Chapter>,
    /// One book opens at a time; a stuck open keeps this until it finishes.
    opening: Arc<AtomicBool>,
    decoder: HostDecoder,
}

impl Worker {
    fn new(cache: PathBuf, decoder: HostDecoder) -> Self {
        Self {
            cache,
            out: None,
            open: None,
            opening: Arc::new(AtomicBool::new(false)),
            decoder,
        }
    }

    fn send(&self, event: ReaderEvent) {
        if let Some(out) = &self.out {
            let _ = out.send(Event::Reader(event));
        }
    }

    fn wake(&self, session: u64) {
        if let Some(out) = &self.out {
            let _ = out.internal(Internal::Reader { session, wake: ReaderWake::Laid });
        }
    }

    fn run(mut self, rx: mpsc::Receiver<Job>) {
        loop {
            // Take every job waiting before making the next leaf, so the newest wish wins.
            let first = if self.open.as_ref().is_some_and(|c| !c.queue.is_empty()) {
                match rx.try_recv() {
                    Ok(job) => Some(job),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => return,
                }
            } else {
                match rx.recv() {
                    Ok(job) => Some(job),
                    Err(_) => return,
                }
            };
            if let Some(job) = first {
                self.take(job);
                while let Ok(job) = rx.try_recv() {
                    self.take(job);
                }
            }
            self.make_next();
            if self.open.as_ref().is_none_or(|c| c.queue.is_empty()) {
                self.out = None;
            }
        }
    }

    fn take(&mut self, job: Job) {
        match job {
            Job::Open(job) => {
                self.out = Some(job.out.clone());
                self.open_chapter(*job)
            }
            Job::Want { session, generation, leaves, out } => {
                self.out = Some(out);
                let mut held = Vec::new();
                if let Some(c) = self
                    .open
                    .as_mut()
                    .filter(|c| c.session == session && c.generation == generation)
                {
                    c.queue = wished(leaves, &c.queue, &c.asked);
                    held = c.held.take().unwrap_or_default();
                }
                for event in held {
                    self.send(event);
                }
            }
            Job::Again { session, page, out } => {
                if let Some(c) = self.open.as_mut().filter(|c| c.session == session) {
                    // A page from a source that failed is fetched again, and made once it lands.
                    if let Some(fed) = &c.feed {
                        fed.feed.retry(page as usize, &out);
                    }
                    c.sent.retain(|(p, _)| *p != page);
                    if let Some(leaf) =
                        c.leaves.iter().position(|l| l.iter().any(|p| p.page == page))
                    {
                        c.queue.push_front(leaf as u32);
                        c.asked.insert(leaf as u32);
                    } else if c.strip.is_some() {
                        c.queue.push_front(page);
                        c.asked.insert(page);
                    }
                }
                self.out = Some(out);
            }
            Job::Landed { session, chapter, page, out } => {
                self.out = Some(out);
                if let Some(c) =
                    self.open.as_mut().filter(|c| c.session == session && c.chapter == chapter)
                    && c.landed(page)
                {
                    self.relay(session);
                }
            }
            Job::Close { session } => {
                if self.open.as_ref().is_some_and(|c| c.session == session) {
                    self.open = None;
                }
            }
            Job::Peek(job) => {
                if let Some(file) = self.first_page(&job.path) {
                    let event = ReaderEvent::NextPreview {
                        session: job.session,
                        chapter: job.chapter,
                        file,
                    };
                    let _ = job.out.send(Event::Reader(event));
                }
            }
        }
    }

    /// A book's first picture as the book has it, kept in that book's cache folder.
    fn first_page(&self, path: &Path) -> Option<PageFile> {
        let dir = self.cache.join(book_key(path));
        std::fs::create_dir_all(&dir).ok()?;
        let kept = ["jpg", "png", "webp", "gif", "avif"]
            .iter()
            .find_map(|ext| std::fs::read(dir.join(format!("p0.{ext}"))).ok());
        let bytes = match kept {
            Some(bytes) => bytes,
            None => match open_book(path, &self.opening).ok()? {
                Book::Comic(mut comic) => {
                    let mut first = None;
                    let mut take = |_: usize, bytes: Vec<u8>| {
                        first = Some(bytes);
                        std::ops::ControlFlow::Break(())
                    };
                    let _ = comic.extract_pages(0, &mut take);
                    first?
                }
                Book::Epub(mut epub) => {
                    let page = epub.image_pages()?.into_iter().next()?;
                    epub.resource(&page.path).ok()?.data
                }
                Book::Pdf(pdf) => pdf.page_image(0).map(|i| i.data).or_else(|| {
                    let page = pdf.render(0, 600, 900).ok()?;
                    png_of(page.width, page.height, page.rgba)
                })?,
            },
        };
        touch(&dir);
        page_file(&keep_original(&dir, 0, &bytes)?)
    }

    /// Every page of the open chapter's feed landed, and they lay out otherwise than guessed.
    fn relay(&self, session: u64) {
        let Some(generation) = self.open.as_ref().map(|c| c.generation) else { return };
        if let Some(out) = &self.out {
            let _ =
                out.internal(Internal::Reader { session, wake: ReaderWake::Relay(generation) });
        }
    }

    fn fail(&self, job: &OpenJob, failure: ReadingFailure) {
        if let Ok(mut laid) = job.shared.lock() {
            laid.generation = job.generation;
            laid.failure = Some(failure);
            laid.growing = false;
        }
        self.wake(job.session);
    }

    fn open_chapter(&mut self, job: OpenJob) {
        self.open = None;
        let dir = self.cache.join(book_key(&job.path));
        if let Err(e) = std::fs::create_dir_all(&dir) {
            tracing::warn!(error = %e, "the page cache could not be made");
            return self.fail(&job, ReadingFailure::Other { reason: e.to_string() });
        }
        touch(&dir);
        evict(&self.cache, &dir);
        let path = job.path.display().to_string();
        // A feed's pages are the ones landed so far; a book is opened.
        let book = match &job.feed {
            Some(_) => None,
            None => match open_book(&job.path, &self.opening) {
                Ok(book) => Some(book),
                Err(failure) => return self.fail(&job, failure),
            },
        };
        let mut originals = Vec::new();
        let mut kinds = Vec::new();
        let mut credits = Vec::new();
        let mut info_rtl = None;
        if let Some(feed) = &job.feed {
            for (i, page) in feed.pages().iter().enumerate() {
                kinds.push(layout::PageKind::Story);
                originals.push(match page {
                    FeedPage::Landed(file) => {
                        std::fs::read(file).ok().and_then(|b| keep_original(&dir, i, &b))
                    }
                    FeedPage::Coming | FeedPage::Failed(_) => None,
                });
            }
            // The feed left its credit pages out as they came; they are a tap away.
            for credit in &feed.credits {
                if let Some(file) = original_at(&credit.path).as_ref().and_then(page_file) {
                    credits.push(CreditPage {
                        page: credit.page,
                        at_end: credit.at_end,
                        file,
                        fingerprint: credit.print,
                    });
                }
            }
        }
        match book {
            None => {}
            Some(Book::Comic(mut comic)) => {
                if let Some(info) = comic.info() {
                    use kaeri_books::comicinfo::MangaFlag;
                    info_rtl = match info.manga {
                        Some(MangaFlag::YesAndRightToLeft | MangaFlag::Yes) => Some(true),
                        Some(MangaFlag::No) => Some(false),
                        _ => None,
                    };
                }
                for page in comic.pages() {
                    use kaeri_books::comicinfo::ComicPageType as T;
                    kinds.push(match page.kind {
                        Some(T::Deleted) => layout::PageKind::Skip,
                        Some(T::FrontCover) => layout::PageKind::Cover,
                        _ if page.double_page => layout::PageKind::Spread,
                        _ => layout::PageKind::Story,
                    });
                }
                originals = vec![None; comic.pages().len()];
                let mut write = |i: usize, bytes: Vec<u8>| {
                    let original = keep_original(&dir, i, &bytes);
                    if let Some(slot) = originals.get_mut(i) {
                        *slot = original;
                    }
                    std::ops::ControlFlow::Continue(())
                };
                if let Err(e) = comic.extract_pages(0, &mut write) {
                    tracing::warn!(error = %e, "a chapter's pages did not all extract");
                    if originals.iter().all(Option::is_none) {
                        let format = job
                            .path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_ascii_uppercase();
                        return self.fail(&job, ReadingFailure::Unreadable { path, format });
                    }
                }
                // Credit pages leave the chapter's pages, so its count and progress go without
                // them; their files stay in the cache, a tap away.
                if let Some(check) = &job.credits {
                    let files: Vec<Option<&Path>> = originals
                        .iter()
                        .map(|o| o.as_ref().map(|o| o.path.as_path()))
                        .collect();
                    for found in check.find(&files).into_iter().rev() {
                        let at = found.page as usize;
                        if kinds.get(at) == Some(&layout::PageKind::Skip) {
                            continue;
                        }
                        let original = originals.remove(at);
                        kinds.remove(at);
                        if let Some(file) = original.as_ref().and_then(page_file) {
                            credits.insert(
                                0,
                                CreditPage {
                                    page: found.page,
                                    at_end: found.at_end,
                                    file,
                                    fingerprint: found.print,
                                },
                            );
                        }
                    }
                }
            }
            #[allow(unreachable_patterns)]
            Some(Book::Pdf(pdf)) => {
                let room = (job.plan.target.width * 2, job.plan.target.height * 2);
                let pages = match pdf_pages(pdf, &dir, room, &self.opening) {
                    Ok(pages) => pages,
                    Err(()) => {
                        let format = "PDF".to_owned();
                        return self.fail(&job, ReadingFailure::Unreadable { path, format });
                    }
                };
                for (i, bytes) in pages.into_iter().enumerate() {
                    kinds.push(layout::PageKind::Story);
                    originals.push(bytes.and_then(|b| keep_original(&dir, i, &b)));
                }
            }
            Some(Book::Epub(mut epub)) => {
                // A manga made into an EPUB: every spine item one picture. Anything with text
                // is a novel, which the session serves to the web view instead.
                let Some(images) = epub.image_pages() else {
                    return self
                        .fail(&job, ReadingFailure::Unsupported { extension: "epub".into() });
                };
                info_rtl =
                    epub.package().direction.map(|d| d == kaeri_books::epub::Direction::Rtl);
                for (i, page) in images.iter().enumerate() {
                    use kaeri_books::epub::PageSpread;
                    kinds.push(match page.spread {
                        Some(PageSpread::Center) => layout::PageKind::Spread,
                        _ if i == 0 && epub.cover().is_some_and(|c| c.path == page.path) => {
                            layout::PageKind::Cover
                        }
                        _ => layout::PageKind::Story,
                    });
                    let bytes = epub.resource(&page.path).ok().map(|r| r.data);
                    originals.push(bytes.and_then(|b| keep_original(&dir, i, &b)));
                }
            }
        }
        // A feed's pages not landed yet are laid out in the shape of the first that has: the
        // pages of one chapter mostly share one. Their parts have no ratio until they land.
        let fed = job.feed.as_ref().is_some_and(|f| !f.complete());
        let guess = originals
            .iter()
            .flatten()
            .next()
            .filter(|_| fed)
            .map_or((1000, 1400), |o| (o.width, o.height));
        let shapes: Vec<(u32, u32)> = originals
            .iter()
            .map(|o| o.as_ref().map_or(guess, |o| (o.width, o.height)))
            .collect();
        let known: Vec<(u32, u32)> = if fed {
            originals
                .iter()
                .map(|o| o.as_ref().map_or((0, 0), |o| (o.width, o.height)))
                .collect()
        } else {
            shapes.clone()
        };
        // The guesses, in the board's order after the series' choice: ComicInfo, then the shape
        // of the pages.
        let plan = &job.plan;
        let tall = is_strip(&shapes);
        let (mode, guessed_mode) = if plan.guess_mode && tall {
            (ReaderMode::Strip, true)
        } else {
            (plan.mode, false)
        };
        let (direction, guessed_direction) = match info_rtl {
            Some(rtl) if plan.guess_direction && !mode.flows() => (
                if rtl { ReadDirection::RightToLeft } else { ReadDirection::LeftToRight },
                true,
            ),
            _ if mode.flows() => (ReadDirection::TopToBottom, false),
            _ => (plan.direction, false),
        };
        let eink = plan.target.panel.is_eink();
        let mut chapter = Chapter {
            session: job.session,
            generation: job.generation,
            chapter: job.chapter.clone(),
            source: job.path.clone(),
            dir,
            renderer: Renderer::new(plan.target, plan.look).with_threads(if eink {
                2
            } else {
                1
            }),
            originals,
            leaves: Vec::new(),
            strip: None,
            sent: HashSet::new(),
            queue: VecDeque::new(),
            asked: HashSet::new(),
            held: Some(Vec::new()),
            decoder: self.decoder.clone(),
            feed: None,
            by_width: !eink && (mode.flows() || plan.fit_width),
        };
        let mut options_used = None;
        let (leaves, cuts) = if mode == ReaderMode::Strip && eink && !fed {
            chapter.strip = cut_strip(&chapter, plan.target);
            let strip = chapter.strip.as_ref();
            let leaves: Vec<Leaf> = strip
                .map(|s| {
                    (0..s.cuts.len() as u32)
                        .map(|i| Leaf {
                            index: i,
                            parts: vec![LeafPart {
                                page: i,
                                kind: PageKind::StripCut,
                                file: None,
                                place: UnitRect::WHOLE,
                                rotate: false,
                                half: None,
                                ratio: None,
                            }],
                            end_page: false,
                        })
                        .collect()
                })
                .unwrap_or_default();
            let cuts = strip.map(strip_places).unwrap_or_default();
            (leaves, cuts)
        } else {
            let pixels = known;
            let shapes: Vec<PageShape> = shapes
                .iter()
                .zip(&kinds)
                .map(|(&(width, height), &kind)| PageShape { width, height, kind })
                .collect();
            let options = LayoutOptions {
                screen: if plan.two_pages && mode == ReaderMode::Paged {
                    Screen::Double
                } else {
                    Screen::Single
                },
                direction: if direction == ReadDirection::RightToLeft {
                    Direction::RightToLeft
                } else {
                    Direction::LeftToRight
                },
                spread_on_single: if mode.flows() {
                    SpreadOnSingle::Whole
                } else {
                    plan.spreads
                },
                first_is_cover: plan.two_pages,
            };
            let laid = layout::lay_out(&shapes, options);
            options_used = Some(options);
            chapter.leaves = laid.iter().map(|l| l.parts.clone()).collect();
            (
                laid.iter()
                    .enumerate()
                    .map(|(i, l)| model_leaf(i as u32, l, &pixels))
                    .collect(),
                Vec::new(),
            )
        };
        let start_leaf = if chapter.strip.is_some() {
            cuts.iter().position(|c| c.image >= job.start_page).unwrap_or(0)
        } else {
            chapter
                .leaves
                .iter()
                .position(|l| l.iter().any(|p| p.page >= job.start_page))
                .unwrap_or(0)
        };
        chapter.queue = [start_leaf, start_leaf + 1].into_iter().map(|l| l as u32).collect();
        chapter.feed = job.feed.clone().map(|feed| Fed {
            feed,
            waiting: HashSet::new(),
            kinds,
            options: options_used,
            guessed: fed,
            eink_strip: fed && mode == ReaderMode::Strip && eink,
        });
        if let Ok(mut laid) = job.shared.lock() {
            *laid = Laid {
                generation: job.generation,
                mode: Some(mode),
                direction: Some(direction),
                guessed_mode,
                guessed_direction,
                pages: chapter.originals.len() as u32,
                leaves,
                cuts,
                growing: false,
                failure: None,
                credits,
            };
        }
        self.open = Some(chapter);
        self.wake(job.session);
    }

    fn make_next(&mut self) {
        let Some(chapter) = self.open.as_mut() else { return };
        let Some(leaf) = chapter.queue.pop_front() else { return };
        chapter.asked.remove(&leaf);
        let events = chapter.make_leaf(leaf);
        if let Some(held) = chapter.held.as_mut() {
            held.extend(events);
            return;
        }
        for event in events {
            self.send(event);
        }
    }
}

/// The queue after a wish: the wished leaves in order, then any leaf an app asked for again
/// that the wish leaves out, so a row that asked keeps its turn. A leaf made already costs
/// nothing when wished again.
fn wished(leaves: Vec<u32>, queue: &VecDeque<u32>, asked: &HashSet<u32>) -> VecDeque<u32> {
    let kept: Vec<u32> =
        queue.iter().copied().filter(|l| asked.contains(l) && !leaves.contains(l)).collect();
    let mut next: VecDeque<u32> = leaves.into();
    next.extend(kept);
    next
}

/// Opens the book on its own thread, waiting at most [`OPEN_BUDGET`].
fn open_book(path: &Path, opening: &Arc<AtomicBool>) -> Result<Book, ReadingFailure> {
    let shown = path.display().to_string();
    let format = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_uppercase();
    let slow = || ReadingFailure::Unreadable { path: shown.clone(), format: format.clone() };
    let since = Instant::now();
    // A book still opening from before holds the slot: wait for it within the same budget.
    while opening.swap(true, Ordering::AcqRel) {
        if since.elapsed() > OPEN_BUDGET {
            return Err(slow());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let (tx, rx) = mpsc::channel();
    let owned = path.to_owned();
    let slot = opening.clone();
    let spawned = std::thread::Builder::new().name("kaeri-book-open".into()).spawn(move || {
        let book = kaeri_books::open(&owned, &Limits::default());
        slot.store(false, Ordering::Release);
        let _ = tx.send(book);
    });
    if spawned.is_err() {
        opening.store(false, Ordering::Release);
        return Err(slow());
    }
    match rx.recv_timeout(OPEN_BUDGET.saturating_sub(since.elapsed())) {
        Ok(Ok(book)) => Ok(book),
        Ok(Err(e)) => Err(failure_of(&e, shown, format)),
        Err(_) => {
            tracing::warn!(path = %shown, "a book took too long to open");
            Err(slow())
        }
    }
}

/// Every page of a PDF as image bytes: the page's own JPEG, a page kept from an earlier open,
/// or a render no larger than `room`. The PDF reader has no way to stop it, so the pages are made
/// on a thread of their own that holds the one-book slot until it ends, and a page that takes
/// longer than [`PDF_PAGE_BUDGET`] gives the chapter up.
fn pdf_pages(
    pdf: kaeri_books::pdf::Pdf,
    dir: &Path,
    room: (u32, u32),
    opening: &Arc<AtomicBool>,
) -> Result<Vec<Option<Vec<u8>>>, ()> {
    let since = Instant::now();
    while opening.swap(true, Ordering::AcqRel) {
        if since.elapsed() > OPEN_BUDGET {
            return Err(());
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let count = pdf.page_count();
    let (tx, rx) = mpsc::channel();
    let slot = opening.clone();
    let dir = dir.to_owned();
    let spawned = std::thread::Builder::new().name("kaeri-pdf-pages".into()).spawn(move || {
        for i in 0..count {
            let kept = ["jpg", "png"]
                .iter()
                .find_map(|ext| std::fs::read(dir.join(format!("p{i}.{ext}"))).ok());
            let bytes = kept.or_else(|| match pdf.page_image(i) {
                Some(image) => Some(image.data),
                None => pdf
                    .render(i, room.0, room.1)
                    .ok()
                    .and_then(|r| png_of(r.width, r.height, r.rgba)),
            });
            if tx.send(bytes).is_err() {
                break;
            }
        }
        slot.store(false, Ordering::Release);
    });
    if spawned.is_err() {
        opening.store(false, Ordering::Release);
        return Err(());
    }
    let mut pages = Vec::with_capacity(count);
    for _ in 0..count {
        match rx.recv_timeout(PDF_PAGE_BUDGET) {
            Ok(bytes) => pages.push(bytes),
            Err(_) => {
                tracing::warn!(page = pages.len(), "a PDF page took too long to make");
                return Err(());
            }
        }
    }
    Ok(pages)
}

pub(crate) fn failure_of(
    e: &kaeri_books::BookError,
    path: String,
    format: String,
) -> ReadingFailure {
    use kaeri_books::FailureKind as K;
    match e.reading_failure() {
        K::FileMissing => ReadingFailure::FileMissing { path },
        K::Protected => ReadingFailure::Protected,
        K::DiskFull => ReadingFailure::DiskFull,
        K::Unsupported => {
            ReadingFailure::Unsupported { extension: format.to_ascii_lowercase() }
        }
        K::Unreadable => ReadingFailure::Unreadable { path, format },
    }
}

/// Whether the pages are a vertical strip: most of them far taller than wide.
pub fn is_strip(shapes: &[(u32, u32)]) -> bool {
    let tall = shapes.iter().filter(|(w, h)| *h as f32 > *w as f32 * 2.2).count();
    shapes.len() >= 2 && tall * 2 > shapes.len()
}

/// The width over the height of a part as the reader draws it, before its file is made.
fn ratio_of(part: &layout::LeafPart, shapes: &[(u32, u32)]) -> Option<f32> {
    let (w, h) = match part.source {
        Some(s) => (s.width, s.height),
        None => *shapes.get(part.page as usize)?,
    };
    let (w, h) = if part.rotate { (h, w) } else { (w, h) };
    (w > 0 && h > 0).then(|| w as f32 / h as f32)
}

fn model_leaf(index: u32, leaf: &layout::Leaf, shapes: &[(u32, u32)]) -> Leaf {
    Leaf {
        index,
        parts: leaf
            .parts
            .iter()
            .map(|p| LeafPart {
                page: p.page,
                kind: match p.kind {
                    layout::PageKind::Spread => PageKind::Spread,
                    layout::PageKind::Cover => PageKind::Cover,
                    layout::PageKind::Skip => PageKind::Skip,
                    layout::PageKind::StripCut => PageKind::StripCut,
                    layout::PageKind::Story => PageKind::Story,
                },
                file: None,
                place: UnitRect {
                    x: p.place.x,
                    y: p.place.y,
                    width: p.place.width,
                    height: p.place.height,
                },
                rotate: p.rotate,
                half: half_of(p),
                ratio: ratio_of(p, shapes),
            })
            .collect(),
        end_page: false,
    }
}

fn half_of(part: &layout::LeafPart) -> Option<PageHalf> {
    part.source.map(|s| if s.x == 0 { PageHalf::Left } else { PageHalf::Right })
}

impl Chapter {
    fn make_leaf(&mut self, leaf: u32) -> Vec<ReaderEvent> {
        if self.strip.is_some() {
            return self.make_cut(leaf).into_iter().collect();
        }
        let Some(parts) = self.leaves.get(leaf as usize).cloned() else { return Vec::new() };
        let target = *self.renderer.target();
        let mut events = Vec::new();
        for part in parts {
            let half = half_of(&part);
            if !self.sent.insert((part.page, half)) {
                continue;
            }
            // A page from a source not landed yet is made once it does; one that failed says so.
            if let Some(fed) = self.feed.as_mut()
                && self.originals.get(part.page as usize).is_some_and(Option::is_none)
            {
                match fed.feed.page(part.page as usize) {
                    Some(FeedPage::Coming) => {
                        self.sent.remove(&(part.page, half));
                        fed.waiting.insert(leaf);
                        // Waited on: fetched before the pages nobody is looking at yet.
                        fed.feed.want(part.page as usize);
                        continue;
                    }
                    // Said once: a retry asks for it again.
                    Some(FeedPage::Failed(failure)) => {
                        events.push(ReaderEvent::PageFailed {
                            session: self.session,
                            chapter: self.chapter.clone(),
                            page: part.page,
                            failure,
                        });
                        continue;
                    }
                    Some(FeedPage::Landed(file)) => {
                        let at = part.page as usize;
                        self.originals[at] = std::fs::read(file)
                            .ok()
                            .and_then(|b| keep_original(&self.dir, at, &b));
                    }
                    None => {}
                }
            }
            let (w, h) = self.part_box(&part, target);
            let event = match self.make_part(&part, w, h) {
                Ok(file) => ReaderEvent::PageReady {
                    session: self.session,
                    chapter: self.chapter.clone(),
                    page: part.page,
                    half,
                    file,
                },
                Err(failure) => {
                    self.sent.remove(&(part.page, half));
                    ReaderEvent::PageFailed {
                        session: self.session,
                        chapter: self.chapter.clone(),
                        page: part.page,
                        failure,
                    }
                }
            };
            events.push(event);
        }
        events
    }

    /// A page of the feed landed or failed: the leaves waiting on it are made. `true` when it
    /// was the last to land and the pages lay out otherwise than guessed before they had.
    fn landed(&mut self, page: u32) -> bool {
        let Some(fed) = self.feed.as_mut() else { return false };
        let leaves = &self.leaves;
        let ready: Vec<u32> = fed
            .waiting
            .iter()
            .copied()
            .filter(|l| {
                leaves.get(*l as usize).is_some_and(|ps| ps.iter().any(|p| p.page == page))
            })
            .collect();
        for leaf in ready {
            fed.waiting.remove(&leaf);
            self.queue.push_front(leaf);
            self.asked.insert(leaf);
        }
        if !fed.guessed || !fed.feed.complete() {
            return false;
        }
        fed.guessed = false;
        for (at, page) in fed.feed.pages().into_iter().enumerate() {
            if let FeedPage::Landed(file) = page
                && self.originals.get(at).is_some_and(Option::is_none)
            {
                self.originals[at] =
                    std::fs::read(file).ok().and_then(|b| keep_original(&self.dir, at, &b));
            }
        }
        if fed.eink_strip {
            return true;
        }
        let Some(options) = fed.options else { return false };
        let shapes: Vec<PageShape> = self
            .originals
            .iter()
            .zip(&fed.kinds)
            .map(|(o, &kind)| {
                let (width, height) = o.as_ref().map_or((1000, 1400), |o| (o.width, o.height));
                PageShape { width, height, kind }
            })
            .collect();
        layout::lay_out(&shapes, options).iter().map(|l| &l.parts).ne(self.leaves.iter())
    }

    /// The box a part is fitted in, in pixels: its share of the screen. Where pages are drawn
    /// by width, the height is the part's own at that width instead. A strip image fitted inside
    /// the screen would shrink to a sliver as tall as the screen, and the column would then
    /// stretch that sliver back across its width.
    fn part_box(&self, part: &layout::LeafPart, target: Target) -> (u32, u32) {
        let (w, h) = (
            (target.width as f32 * part.place.width) as u32,
            (target.height as f32 * part.place.height) as u32,
        );
        if part.rotate {
            return (h.max(1), w.max(1));
        }
        let original = self.originals.get(part.page as usize).and_then(Option::as_ref);
        match original {
            Some(o) if self.by_width => {
                let region = part.source.map_or((o.width, o.height), |s| (s.width, s.height));
                Fit::Width(w.max(1)).size(region.0, region.1)
            }
            _ => (w.max(1), h.max(1)),
        }
    }

    fn unreadable(&self) -> ReadingFailure {
        ReadingFailure::Unreadable {
            path: self.source.display().to_string(),
            format: self
                .source
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_uppercase(),
        }
    }

    fn make_part(
        &mut self,
        part: &layout::LeafPart,
        w: u32,
        h: u32,
    ) -> Result<PageFile, ReadingFailure> {
        let original = self
            .originals
            .get(part.page as usize)
            .cloned()
            .flatten()
            .ok_or_else(|| self.unreadable())?;
        let target = *self.renderer.target();
        let region = part
            .source
            .unwrap_or(kaeri_pages::PixelRect::full(original.width, original.height));
        let format = original.format.ok_or_else(|| self.unreadable())?;
        let fitted = Target { width: w, height: h, ..target };
        if kaeri_pages::delivery(format, (region.width, region.height), &fitted)
            == Delivery::Original
            && !self.renderer.look().crop
        {
            let bytes = std::fs::metadata(&original.path).map(|m| m.len()).unwrap_or(0);
            return Ok(PageFile {
                path: original.path.display().to_string(),
                encoding: PageEncoding::Original { mime: format.mime().to_owned() },
                width: original.width,
                height: original.height,
                crop: part.source.map(|s| PixelRect {
                    x: s.x,
                    y: s.y,
                    width: s.width,
                    height: s.height,
                }),
                colour: false,
                bytes,
            });
        }
        let bytes = std::fs::read(&original.path).map_err(|_| self.unreadable())?;
        // What the page is decoded for: the screen, or the box it is drawn in by width, which
        // is taller than the screen.
        let room = if self.by_width { (w, h) } else { (target.width, target.height) };
        let page = if format.decodes() {
            self.renderer.open_for(&bytes, room).map_err(|e| {
                tracing::warn!(error = %e, "a page did not decode");
                self.unreadable()
            })?
        } else {
            // A format the engine cannot read: the platform decodes it, small if it can.
            let room = decode_room(original.width, original.height, room);
            match self.decoder.bitmap(bytes, format, room) {
                Hosted::Image(bitmap) => {
                    self.renderer.open_bitmap(bitmap, format).map_err(|e| {
                        tracing::warn!(error = %e, "a page did not decode");
                        self.unreadable()
                    })?
                }
                Hosted::Unsupported => {
                    return Err(ReadingFailure::Unsupported {
                        extension: extension_of(format).into(),
                    });
                }
                Hosted::Unreadable => return Err(self.unreadable()),
            }
        };
        // The layout's rectangles are in the file's pixels; a large scan may decode smaller.
        let region = part.source.map(|s| scale_rect(s, original.width, page.bitmap.width()));
        let raw = self.renderer.render(&page, region, w, h).map_err(|e| {
            tracing::warn!(error = %e, "a page was not processed");
            self.unreadable()
        })?;
        let name = format!(
            "r{}{}-{}x{}-{:x}.raw",
            part.page,
            half_tag(half_of(part)),
            w,
            h,
            look_key(self.renderer.look())
        );
        self.write_raw(&name, raw.bytes(), raw.width(), raw.height(), raw.format(), raw.colour)
    }

    fn write_raw(
        &self,
        name: &str,
        bytes: &[u8],
        width: u32,
        height: u32,
        format: kaeri_pages::PixelFormat,
        colour: bool,
    ) -> Result<PageFile, ReadingFailure> {
        let path = self.dir.join(name);
        write_whole(&path, bytes).map_err(|e| {
            if e.raw_os_error() == Some(28) {
                ReadingFailure::DiskFull
            } else {
                ReadingFailure::Other { reason: e.to_string() }
            }
        })?;
        let (format, bpp) = match format {
            kaeri_pages::PixelFormat::Gray8 => (PixelFormat::Gray8, 1),
            _ => (PixelFormat::Rgba8, 4),
        };
        Ok(PageFile {
            path: path.display().to_string(),
            encoding: PageEncoding::Raw { format, stride: width * bpp },
            width,
            height,
            crop: None,
            colour,
            bytes: bytes.len() as u64,
        })
    }

    fn make_cut(&mut self, index: u32) -> Option<ReaderEvent> {
        let strip = self.strip.as_ref()?;
        let cut = *strip.cuts.get(index as usize)?;
        if !self.sent.insert((index, None)) {
            return None;
        }
        let result = (|| {
            let first =
                strip.images.iter().rposition(|(_, _, top)| *top <= cut.top).unwrap_or(0);
            let last =
                strip.images.iter().rposition(|(_, _, top)| *top < cut.bottom).unwrap_or(first);
            let mut images = Vec::new();
            for (path, height, _) in &strip.images[first..=last] {
                let data = std::fs::read(path).ok()?;
                images.push(Bitmap::new(strip.width, *height, strip.format, data).ok()?);
            }
            let offset = strip.images[first].2;
            let shifted = StripCut {
                top: cut.top - offset,
                bottom: cut.bottom - offset,
                overlap: cut.overlap,
            };
            let refs: Vec<&Bitmap> = images.iter().collect();
            kaeri_pages::strip::extract(&refs, shifted).ok()
        })();
        let colour = strip.format != kaeri_pages::PixelFormat::Gray8;
        let made = result
            .and_then(|bitmap| self.renderer.finish(bitmap, colour).ok())
            .ok_or_else(|| self.unreadable())
            .and_then(|raw| {
                let name = format!(
                    "c{index}-{}-{:x}.raw",
                    raw.width(),
                    look_key(self.renderer.look())
                );
                self.write_raw(
                    &name,
                    raw.bytes(),
                    raw.width(),
                    raw.height(),
                    raw.format(),
                    raw.colour,
                )
            });
        Some(match made {
            Ok(file) => ReaderEvent::PageReady {
                session: self.session,
                chapter: self.chapter.clone(),
                page: index,
                half: None,
                file,
            },
            Err(failure) => {
                self.sent.remove(&(index, None));
                ReaderEvent::PageFailed {
                    session: self.session,
                    chapter: self.chapter.clone(),
                    page: index,
                    failure,
                }
            }
        })
    }
}

/// Scales every image of a strip to the screen's width, keeps them in the cache, and plans the
/// screens it is cut into, at gaps between panels where there are any.
fn cut_strip(chapter: &Chapter, target: Target) -> Option<Strip> {
    let width = target.width;
    let colour = target.panel.shows_colour();
    let format =
        if colour { kaeri_pages::PixelFormat::Rgba8 } else { kaeri_pages::PixelFormat::Gray8 };
    let mut images = Vec::new();
    let mut gaps = Vec::new();
    let mut top = 0u32;
    for (i, original) in chapter.originals.iter().enumerate() {
        let Some(original) = original else { continue };
        let Ok(bytes) = std::fs::read(&original.path) else { continue };
        let hosted = original.format.filter(|f| !f.decodes());
        let decoded = match hosted {
            Some(format) => {
                // A strip is drawn at the screen's width: ask for twice that at most.
                let scale = (2.0 * width as f64 / original.width.max(1) as f64).min(1.0);
                let room = (
                    (original.width as f64 * scale).ceil() as u32,
                    (original.height as f64 * scale).ceil() as u32,
                );
                match chapter.decoder.bitmap(bytes, format, room) {
                    Hosted::Image(bitmap) => Some(bitmap),
                    _ => None,
                }
            }
            None => kaeri_pages::decode_for(
                &bytes,
                kaeri_pages::DecodeLimits::default(),
                Some((width, target.height)),
            )
            .ok(),
        };
        let Some(bitmap) = decoded else { continue };
        let (w, h) = Fit::Width(width).size(bitmap.width(), bitmap.height());
        let Ok(scaled) = resize(&bitmap, bitmap.bounds(), w, h) else { continue };
        let scaled = if colour { scaled.into_rgba() } else { scaled.into_gray() };
        gaps.extend(kaeri_pages::strip::gap_rows(&scaled));
        let path = chapter.dir.join(format!("s{i}-{width}.px"));
        if write_whole(&path, scaled.data()).is_err() {
            continue;
        }
        images.push((path, h, top));
        top += h;
    }
    if images.is_empty() {
        return None;
    }
    let cuts = kaeri_pages::strip::plan_cuts(
        &gaps,
        StripOptions::for_screen(target.height, target.height / 12),
    );
    Some(Strip { images, width, format, cuts })
}

/// Where each cut starts: the image and the offset inside it.
fn strip_places(strip: &Strip) -> Vec<StripPos> {
    strip
        .cuts
        .iter()
        .map(|cut| {
            let i = strip.images.iter().rposition(|(_, _, top)| *top <= cut.top).unwrap_or(0);
            let (_, height, top) = strip.images[i];
            StripPos { image: i as u32, offset: (cut.top - top) as f32 / height.max(1) as f32 }
        })
        .collect()
}

/// The extension a failure names for a format.
const fn extension_of(format: SourceFormat) -> &'static str {
    match format {
        SourceFormat::Jpeg => "jpg",
        SourceFormat::Png => "png",
        SourceFormat::WebP => "webp",
        SourceFormat::Gif => "gif",
        SourceFormat::Avif => "avif",
    }
}

fn scale_rect(r: kaeri_pages::PixelRect, from: u32, to: u32) -> kaeri_pages::PixelRect {
    if from == to || from == 0 {
        return r;
    }
    let s = |v: u32| ((v as u64 * to as u64) / from as u64) as u32;
    kaeri_pages::PixelRect::new(s(r.x), s(r.y), s(r.width).max(1), s(r.height).max(1))
}

const fn half_tag(half: Option<PageHalf>) -> &'static str {
    match half {
        Some(PageHalf::Left) => "l",
        Some(PageHalf::Right) => "r",
        None => "",
    }
}

fn look_key(look: &Look) -> u64 {
    let mut h = DefaultHasher::new();
    format!("{look:?}").hash(&mut h);
    h.finish()
}

/// The cache folder's name for a book: its path, size and change time.
fn book_key(path: &Path) -> String {
    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    if let Ok(meta) = std::fs::metadata(path) {
        meta.len().hash(&mut h);
        meta.modified().ok().hash(&mut h);
    }
    format!("{:016x}", h.finish())
}

/// A kept original as the app shows it, untouched.
fn page_file(original: &Original) -> Option<PageFile> {
    let format = original.format?;
    let bytes = std::fs::metadata(&original.path).ok()?.len();
    Some(PageFile {
        path: original.path.display().to_string(),
        encoding: PageEncoding::Original { mime: format.mime().to_owned() },
        width: original.width,
        height: original.height,
        crop: None,
        colour: false,
        bytes,
    })
}

/// A page's file where it is, as an original.
fn original_at(path: &Path) -> Option<Original> {
    let bytes = std::fs::read(path).ok()?;
    let format = SourceFormat::sniff(&bytes);
    let (width, height) = kaeri_pages::inspect(&bytes).map_or((0, 0), |i| (i.width, i.height));
    Some(Original { path: path.to_owned(), format, width, height })
}

fn keep_original(dir: &Path, page: usize, bytes: &[u8]) -> Option<Original> {
    let format = SourceFormat::sniff(bytes);
    let info = kaeri_pages::inspect(bytes).ok();
    let ext = match format {
        Some(SourceFormat::Jpeg) => "jpg",
        Some(SourceFormat::Png) => "png",
        Some(SourceFormat::WebP) => "webp",
        Some(SourceFormat::Gif) => "gif",
        Some(SourceFormat::Avif) => "avif",
        None => return None,
    };
    let path = dir.join(format!("p{page}.{ext}"));
    let whole = std::fs::metadata(&path).is_ok_and(|m| m.len() == bytes.len() as u64);
    if !whole && write_whole(&path, bytes).is_err() {
        return None;
    }
    let (width, height) = info.map_or((0, 0), |i| (i.width, i.height));
    Some(Original { path, format, width, height })
}

fn png_of(width: u32, height: u32, rgba: Vec<u8>) -> Option<Vec<u8>> {
    let image = image::RgbaImage::from_raw(width, height, rgba)?;
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .ok()?;
    Some(out)
}

/// Write under a temporary name in the same folder, then rename.
fn write_whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("page");
    let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    std::fs::write(&temp, bytes)?;
    std::fs::rename(&temp, path)
}

fn touch(dir: &Path) {
    let _ = std::fs::write(dir.join(".used"), b"");
}

/// The page cache's size: a twentieth of the free disk, between 128 MB and 2 GB.
fn budget(cache: &Path) -> u64 {
    let free = crate::downloads::ahead::free_space(cache).unwrap_or(4 << 30);
    (free / 20).clamp(128 << 20, 2 << 30)
}

/// Delete the least recently opened books' pages until the cache fits its budget. The open
/// book's folder is pinned.
fn evict(cache: &Path, keep: &Path) {
    let Ok(entries) = std::fs::read_dir(cache) else { return };
    let mut books: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path() != keep && e.path().is_dir())
        .map(|e| {
            let path = e.path();
            let used = std::fs::metadata(path.join(".used"))
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            let size = std::fs::read_dir(&path)
                .map(|d| d.flatten().filter_map(|f| f.metadata().ok()).map(|m| m.len()).sum())
                .unwrap_or(0);
            (used, size, path)
        })
        .collect();
    let mut total: u64 = books.iter().map(|b| b.1).sum();
    let budget = budget(cache);
    books.sort();
    for (_, size, path) in books {
        if total <= budget {
            break;
        }
        if std::fs::remove_dir_all(&path).is_ok() {
            total = total.saturating_sub(size);
        }
    }
}

/// Folders of pages left by earlier runs are kept for reopening; nothing here is the
/// person's.
pub fn cache_dir(cache: &Path) -> PathBuf {
    cache.join("pages")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tall_pages_are_a_strip() {
        assert!(is_strip(&[(800, 4000), (800, 3800), (800, 1200)]));
        assert!(!is_strip(&[(1000, 1400), (1000, 1400), (2000, 1400)]));
        assert!(!is_strip(&[(800, 4000)]));
    }

    #[test]
    fn the_host_decodes_no_larger_than_twice_what_is_shown() {
        // A page the screen shows smaller than half its size is asked for smaller.
        assert_eq!(decode_room(3000, 4500, (600, 900)), (1200, 1800));
        // One the screen shows larger is asked for whole.
        assert_eq!(decode_room(300, 450, (600, 900)), (300, 450));
        // A spread shown as two halves is asked for at the size one half needs.
        let (w, h) = decode_room(6000, 4000, (600, 900));
        assert!((1200..6000).contains(&w) && h < 4000);
    }

    #[test]
    fn a_wish_keeps_the_leaves_asked_for_again() {
        let queue: VecDeque<u32> = [9, 4, 5].into();
        let asked: HashSet<u32> = [9, 5].into();
        // The wish comes first; 9 was asked for and is kept, 4 was only wished before.
        assert_eq!(wished(vec![5, 6, 7], &queue, &asked), VecDeque::from([5, 6, 7, 9]));
        assert_eq!(wished(vec![1], &VecDeque::new(), &HashSet::new()), VecDeque::from([1]));
    }

    /// A made-up strip image: fine colour stripes a pixel wide, as lettering would be.
    fn stripes_jpeg(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbImage::from_fn(width, height, |x, y| {
            if (x + y / 7) % 2 == 0 {
                image::Rgb([200, 40, 30])
            } else {
                image::Rgb([20, 30, 160])
            }
        });
        let mut out = Vec::new();
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Jpeg)
            .unwrap();
        out
    }

    /// Every page a chapter of `pages` made-up images makes on an LCD screen of `screen`
    /// pixels, opened the way a chapter from a source is: a folder of its pages.
    fn made_pages(
        pages: &[(u32, u32)],
        screen: (u32, u32),
        mode: ReaderMode,
        fit_width: bool,
    ) -> Vec<PageFile> {
        let dir = tempfile::tempdir().unwrap();
        let book = dir.path().join("1-pages");
        std::fs::create_dir_all(&book).unwrap();
        for (i, (w, h)) in pages.iter().enumerate() {
            std::fs::write(book.join(format!("{:05}.jpg", i + 1)), stripes_jpeg(*w, *h))
                .unwrap();
        }
        let worker = Pages::start(dir.path().join("cache"), None);
        let (out, mut inbox) = Outbox::inbox();
        let shared = Shared::default();
        worker.open(OpenJob {
            session: 1,
            generation: 1,
            chapter: ChapterId::new("1"),
            path: book,
            plan: Plan {
                target: Target {
                    panel: kaeri_pages::Panel::Lcd,
                    width: screen.0,
                    height: screen.1,
                },
                look: Look::default(),
                two_pages: false,
                spreads: SpreadOnSingle::Split,
                mode,
                direction: ReadDirection::TopToBottom,
                guess_mode: false,
                guess_direction: false,
                fit_width,
            },
            start_page: 0,
            shared: shared.clone(),
            out: out.clone(),
            feed: None,
            credits: None,
        });
        worker.want(1, 1, (0..pages.len() as u32).collect(), &out);
        let mut made = vec![None; pages.len()];
        let until = Instant::now() + Duration::from_secs(60);
        while made.iter().any(Option::is_none) {
            assert!(Instant::now() < until, "every page was made: {made:?}");
            match inbox.try_recv() {
                Ok(crate::outbox::Envelope::Event(Event::Reader(ReaderEvent::PageReady {
                    page,
                    file,
                    ..
                }))) => made[page as usize] = Some(file),
                Ok(crate::outbox::Envelope::Event(Event::Reader(
                    failed @ ReaderEvent::PageFailed { .. },
                ))) => panic!("{failed:?}"),
                Ok(_) => {}
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
        assert_eq!(shared.lock().unwrap().mode, Some(mode));
        made.into_iter().map(Option::unwrap).collect()
    }

    #[test]
    fn a_strip_image_keeps_its_full_width_on_a_screen_shorter_than_it() {
        // The long-strip shape: 720 wide and several screens tall, on a Mac window 760 points
        // wide at 2x. Fitted inside the screen it would come out 309 pixels wide.
        let made = made_pages(
            &[(720, 4000), (720, 4000), (720, 855)],
            (1520, 1718),
            ReaderMode::Strip,
            false,
        );
        for file in made {
            // Narrower than the screen: the platform draws the original, every pixel of it.
            assert!(matches!(file.encoding, PageEncoding::Original { .. }), "{file:?}");
            assert_eq!(file.width, 720, "{file:?}");
            assert!(file.crop.is_none());
        }
    }

    #[test]
    fn a_strip_image_far_wider_than_the_screen_is_made_at_the_screens_width() {
        // Shrunk past twice, so the engine makes it, at the width the strip draws it.
        let made =
            made_pages(&[(2400, 9000), (2400, 9000)], (1000, 1200), ReaderMode::Strip, false);
        for file in made {
            assert!(matches!(file.encoding, PageEncoding::Raw { .. }), "{file:?}");
            assert_eq!((file.width, file.height), (1000, 3750), "{file:?}");
        }
    }

    #[test]
    fn a_page_turned_one_at_a_time_is_still_fitted_inside_the_screen() {
        let made =
            made_pages(&[(2400, 4000), (2400, 4000)], (600, 900), ReaderMode::Paged, false);
        for file in made {
            assert!(matches!(file.encoding, PageEncoding::Raw { .. }), "{file:?}");
            assert_eq!((file.width, file.height), (540, 900), "{file:?}");
        }
    }

    #[test]
    fn fit_width_makes_a_page_as_wide_as_the_screen() {
        // Scrolled down at the screen's width, not shrunk to its height.
        let made =
            made_pages(&[(2400, 4000), (2400, 4000)], (600, 900), ReaderMode::Paged, true);
        for file in made {
            assert!(matches!(file.encoding, PageEncoding::Raw { .. }), "{file:?}");
            assert_eq!((file.width, file.height), (600, 1000), "{file:?}");
        }
    }

    #[test]
    fn rectangles_follow_a_smaller_decode() {
        let r = kaeri_pages::PixelRect::new(2000, 0, 2000, 2800);
        assert_eq!(scale_rect(r, 4000, 2000), kaeri_pages::PixelRect::new(1000, 0, 1000, 1400));
    }
}

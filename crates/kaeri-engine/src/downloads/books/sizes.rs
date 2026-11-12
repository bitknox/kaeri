//! What downloads take up: each series' line on Downloads, the Reading section's total, and the
//! estimate above the Download button when choosing chapters.

use std::collections::{HashMap, HashSet};

use kaeri_core::books::{ChapterId, WorkId};
use kaeri_model::{
    Msg, ReadingMsg, Size,
    reading::{BookDownloadSeries, ChapterRow, DownloadEstimate, DownloadPick, ReadingFailure},
};
use kaeri_store::{
    Store,
    book_downloads::{BookDownload, BookDownloadState},
};

use super::{BookDownloads, Setup, decode};

/// Pages in a chapter when neither the source nor a past download says.
const PAGES: u32 = 20;
/// A page's bytes when no page was downloaded before: a manga page as sources commonly give it.
const PAGE_BYTES: u64 = 350_000;
/// A text chapter's bytes when none was downloaded before.
const TEXT_CHAPTER_BYTES: u64 = 40_000;

fn r(msg: ReadingMsg) -> Msg {
    Msg::Reading(msg)
}

fn count(n: usize) -> i64 {
    i64::try_from(n).unwrap_or(i64::MAX)
}

/// Whether a work is a light novel, so its chapters are parts.
fn is_novel(store: &Store, work: &str) -> bool {
    store.book_work(work).ok().flatten().is_some_and(|w| w.medium == "novel")
}

/// A row's chapter as people read it: its number, or the source's id for it.
fn label(row: &BookDownload) -> String {
    let id = ChapterId::new(row.chapter.clone());
    id.key().and_then(|k| k.range()).map_or_else(|| row.chapter.clone(), |r| r.to_string())
}

/// "6 chapters · 214 MB", or "5 parts · 9 MB" for a light novel.
fn amount(chapters: usize, bytes: u64, novel: bool) -> Vec<Msg> {
    let n = count(chapters);
    vec![
        if novel {
            r(ReadingMsg::ReadingDlParts { count: n })
        } else {
            r(ReadingMsg::ReadingChapters { count: n })
        },
        r(ReadingMsg::ReadingDlSize { size: Size::of(bytes) }),
    ]
}

/// The Reading section's total, each kind by its own word: "38 chapters · 5 parts · 1.1 GB".
/// Nothing with nothing downloaded.
pub(crate) fn total(kept: Kept, bytes: u64) -> Vec<Msg> {
    let mut out = Vec::new();
    if kept.chapters > 0 {
        out.push(r(ReadingMsg::ReadingChapters { count: count(kept.chapters) }));
    }
    if kept.parts > 0 {
        out.push(r(ReadingMsg::ReadingDlParts { count: count(kept.parts) }));
    }
    if !out.is_empty() {
        out.push(r(ReadingMsg::ReadingDlSize { size: Size::of(bytes) }));
    }
    out
}

/// What is downloaded, by kind: manga chapters, and light novel parts.
#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct Kept {
    pub chapters: usize,
    pub parts: usize,
}

/// Each series with downloads, the one downloading first, then by when each last changed, with
/// what its read chapters take up altogether and what is downloaded by kind.
pub(crate) fn series(
    store: &Store,
    rows: &[BookDownload],
    wifi: bool,
    form: kaeri_store::shelf::TitleForm,
) -> (Vec<BookDownloadSeries>, u64, Kept) {
    let mut kept_all = Kept::default();
    let mut by_work: Vec<(&str, Vec<&BookDownload>)> = Vec::new();
    for row in rows.iter().filter(|d| d.state != BookDownloadState::Removed) {
        match by_work.iter_mut().find(|(w, _)| *w == row.work) {
            Some((_, list)) => list.push(row),
            None => by_work.push((row.work.as_str(), vec![row])),
        }
    }
    let mut read_bytes = 0;
    let mut out: Vec<(bool, i64, BookDownloadSeries)> = Vec::new();
    for (work, list) in by_work {
        let marks = store.read_marks(work).unwrap_or_default();
        let bookmarks: HashSet<String> =
            store.bookmarks(work).unwrap_or_default().into_iter().collect();
        let kept = store.book_work(work).ok().flatten();
        let novel = kept.as_ref().is_some_and(|w| w.medium == "novel");
        let title = store
            .book_work_kind(work)
            .ok()
            .flatten()
            .and_then(|kind| kind.title_in(form))
            .unwrap_or_else(|| work.to_owned());
        let done: Vec<&&BookDownload> =
            list.iter().filter(|d| d.state == BookDownloadState::Done).collect();
        let read: Vec<&&&BookDownload> =
            done.iter().filter(|d| marks.is_read(&d.chapter)).collect();
        read_bytes += read.iter().map(|d| d.bytes).sum::<u64>();
        let clearable = read.iter().filter(|d| !bookmarks.contains(&d.chapter)).count();
        let downloading = list.iter().find(|d| d.state == BookDownloadState::Downloading);
        let waiting = list.iter().filter(|d| d.state == BookDownloadState::Queued).count();
        let failed: Vec<&&BookDownload> =
            list.iter().filter(|d| d.state == BookDownloadState::Failed).collect();
        let bytes: u64 = list
            .iter()
            .filter(|d| {
                matches!(d.state, BookDownloadState::Done | BookDownloadState::Downloading)
            })
            .map(|d| d.bytes)
            .sum();

        if novel {
            kept_all.parts += done.len();
        } else {
            kept_all.chapters += done.len();
        }

        // What is kept and its size first, on every row, then what is happening now.
        let mut line = Vec::new();
        if !done.is_empty() {
            line.extend(amount(done.len(), bytes, novel));
        }
        let mut progress = None;
        if let Some(now) = downloading {
            line.push(r(ReadingMsg::ReadingDlChapter { chapter: label(now) }));
            if let Some(total) = now.pages_total.filter(|t| *t > 0) {
                line.push(r(ReadingMsg::ReadingDlPages {
                    done: i64::from(now.pages_done),
                    total: i64::from(total),
                }));
                progress = Some((now.pages_done as f32 / total as f32).clamp(0.0, 1.0));
            }
            if waiting > 0 {
                line.push(r(ReadingMsg::ReadingDlMoreQueued { count: count(waiting) }));
            }
        } else if let Some(first) = failed.first() {
            line.push(r(ReadingMsg::ReadingDlFailed { chapter: label(first) }));
            // The row keeps a short reason: the downloads banner says what being offline means.
            let failure = first.error.as_deref().map_or(ReadingFailure::Offline, decode);
            line.push(match failure {
                ReadingFailure::Offline => r(ReadingMsg::ReadingDlOffline),
                other => other.msg(),
            });
        } else {
            if clearable > 0 {
                line.push(r(ReadingMsg::ReadingDlRead { count: count(clearable) }));
            }
            if waiting > 0 {
                line.push(r(ReadingMsg::ReadingDlQueued { count: count(waiting) }));
                if wifi {
                    line.push(r(ReadingMsg::ReadingDlWaitsWifi));
                }
            }
        }
        let changed = list.iter().map(|d| d.updated_at).max().unwrap_or(0);
        let series = BookDownloadSeries {
            work: WorkId::new(work.to_owned()),
            title,
            cover: kept.and_then(|w| w.cover),
            chapters: u32::try_from(done.len()).unwrap_or(u32::MAX),
            bytes,
            read: u32::try_from(clearable).unwrap_or(u32::MAX),
            queued: u32::try_from(waiting + usize::from(downloading.is_some()))
                .unwrap_or(u32::MAX),
            failed: failed.iter().map(|d| ChapterId::new(d.chapter.clone())).collect(),
            line,
            progress,
        };
        out.push((downloading.is_some(), changed, series));
    }
    out.sort_by(|a, b| {
        b.0.cmp(&a.0).then(b.1.cmp(&a.1)).then_with(|| a.2.title.cmp(&b.2.title))
    });
    (out.into_iter().map(|(_, _, s)| s).collect(), read_bytes, kept_all)
}

/// Bytes per page and pages per chapter learnt from finished downloads: the source's first,
/// then any.
struct Learnt {
    page_bytes: Option<u64>,
    pages: Option<u32>,
    text_chapter: Option<u64>,
}

fn learn(store: &Store, rows: &[BookDownload], source: Option<&str>) -> Learnt {
    let done = |d: &&BookDownload| d.state == BookDownloadState::Done && d.bytes > 0;
    let paged = |d: &&BookDownload| d.pages_total.is_some_and(|p| p > 0);
    let mut novels: HashMap<String, bool> = HashMap::new();
    let mut novel = |work: &str| -> bool {
        *novels.entry(work.to_owned()).or_insert_with(|| is_novel(store, work))
    };
    let pick = |from: &dyn Fn(&&BookDownload) -> bool| -> (Option<u64>, Option<u32>) {
        let images: Vec<&BookDownload> =
            rows.iter().filter(done).filter(paged).filter(|d| from(d)).collect();
        let pages: u64 = images.iter().filter_map(|d| d.pages_total).map(u64::from).sum();
        let bytes: u64 = images.iter().map(|d| d.bytes).sum();
        let n = images.len() as u64;
        (
            (pages > 0).then(|| bytes / pages),
            (n > 0).then(|| u32::try_from(pages / n).unwrap_or(PAGES)),
        )
    };
    let (mut page_bytes, mut pages) = (None, None);
    if let Some(source) = source {
        (page_bytes, pages) = pick(&|d| d.source_id == source);
    }
    let (any_bytes, any_pages) = pick(&|_| true);
    page_bytes = page_bytes.or(any_bytes);
    pages = pages.or(any_pages);
    let texts: Vec<u64> =
        rows.iter().filter(done).filter(|d| novel(&d.work)).map(|d| d.bytes).collect();
    let text_chapter =
        (!texts.is_empty()).then(|| texts.iter().sum::<u64>() / texts.len() as u64);
    Learnt { page_bytes, pages, text_chapter }
}

/// The words for where the chapters will come over: "over Wi-Fi", "over mobile data", or that
/// they wait for Wi-Fi.
fn network(setup: &Setup) -> Option<Msg> {
    if setup.wifi {
        Some(r(ReadingMsg::ReadingEstimateWaitsWifi))
    } else if setup.mobile {
        Some(r(ReadingMsg::ReadingEstimateMobile))
    } else if setup.wifi_only {
        Some(r(ReadingMsg::ReadingEstimateWifi))
    } else {
        None
    }
}

impl BookDownloads {
    /// What downloading `pick` of `rows` from `source` will take: the chapters not downloaded
    /// yet, each its pages (or the usual count) times the bytes a page usually takes.
    pub(crate) fn estimate(
        &self,
        store: &Store,
        work: &WorkId,
        pick: DownloadPick,
        rows: &[ChapterRow],
        source: Option<&str>,
    ) -> DownloadEstimate {
        let all = store.book_downloads(None).unwrap_or_default();
        let have: HashSet<&str> = all
            .iter()
            .filter(|d| d.work == work.as_str())
            .filter(|d| {
                matches!(
                    d.state,
                    BookDownloadState::Done
                        | BookDownloadState::Queued
                        | BookDownloadState::Downloading
                )
            })
            .map(|d| d.chapter.as_str())
            .collect();
        let picked = Self::to_fetch(&pick, rows);
        let chosen: Vec<&ChapterRow> =
            rows.iter().filter(|row| picked.iter().any(|w| w.chapter == row.id)).collect();
        let wanted: Vec<&ChapterRow> =
            chosen.iter().copied().filter(|row| !have.contains(row.id.as_str())).collect();
        // The chosen chapters already on this device, by what they take there.
        let done: HashMap<&str, u64> = all
            .iter()
            .filter(|d| d.work == work.as_str() && d.state == BookDownloadState::Done)
            .map(|d| (d.chapter.as_str(), d.bytes))
            .collect();
        let learnt = learn(store, &all, source);
        let novel = is_novel(store, work.as_str());
        let page = learnt.page_bytes.unwrap_or(PAGE_BYTES);
        let guess = |row: &&ChapterRow| -> u64 {
            if novel {
                learnt.text_chapter.unwrap_or(TEXT_CHAPTER_BYTES)
            } else {
                let pages = crate::read_catalog::chapters::plausible_pages(row.pages)
                    .filter(|p| *p > 0)
                    .or(learnt.pages)
                    .unwrap_or(PAGES);
                u64::from(pages) * page
            }
        };
        let bytes: u64 = wanted.iter().map(guess).sum();
        let all_bytes: u64 = chosen
            .iter()
            .map(|row| done.get(row.id.as_str()).copied().unwrap_or_else(|| guess(row)))
            .sum();
        let mut line = Vec::new();
        if !wanted.is_empty() {
            line.push(r(ReadingMsg::ReadingEstimateSize { size: Size::of(bytes) }));
            line.extend(network(&self.setup()));
        }
        DownloadEstimate {
            work: work.clone(),
            pick,
            chapters: u32::try_from(wanted.len()).unwrap_or(u32::MAX),
            bytes,
            line,
            all_bytes,
        }
    }
}

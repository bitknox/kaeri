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

use super::{BookDownloads, OnDevice, Setup, decode};

/// Pages in a chapter when neither the source nor a past download says.
const PAGES: u32 = 20;

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
    (wifi, room): (bool, bool),
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
        let title = crate::read_catalog::tiles::title_of(store, work, form);
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
                } else if room {
                    // Paused for a full device: they go on once there is room.
                    line.push(r(ReadingMsg::ReadingDlNoRoom { count: count(waiting) }));
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

/// Bytes per page and pages per chapter learnt from finished downloads: the work's own first,
/// then the source's. Pages differ too much between sources (a webtoon's tall strips against a
/// manga's pages) for one source's downloads to say what another's take.
struct Learnt {
    page_bytes: Option<u64>,
    pages: Option<u32>,
    text_chapter: Option<u64>,
}

fn learn(store: &Store, rows: &[BookDownload], work: &str, source: Option<&str>) -> Learnt {
    let done = |d: &&BookDownload| d.state == BookDownloadState::Done && d.bytes > 0;
    let paged = |d: &&BookDownload| d.pages_total.is_some_and(|p| p > 0);
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
    let text = |from: &dyn Fn(&&BookDownload) -> bool| -> Option<u64> {
        let texts: Vec<u64> =
            rows.iter().filter(done).filter(|d| from(d)).map(|d| d.bytes).collect();
        (!texts.is_empty()).then(|| texts.iter().sum::<u64>() / texts.len() as u64)
    };
    let (mut page_bytes, mut pages) = pick(&|d| d.work == work);
    let mut text_chapter = text(&|d| d.work == work);
    if let Some(source) = source {
        let (source_bytes, source_pages) = pick(&|d| d.source_id == source);
        page_bytes = page_bytes.or(source_bytes);
        pages = pages.or(source_pages);
        // A light novel's chapters from the source, never a manga's images.
        text_chapter = text_chapter
            .or_else(|| text(&|d| d.source_id == source && is_novel(store, &d.work)));
    }
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
    /// yet, each its pages (or the usual count) times the bytes a page of this work or source
    /// took before. With nothing downloaded to go by, the size is not known: no number is
    /// better than a wrong one, so the line is empty and the bytes are 0.
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
        // The chosen chapters already on this device, downloaded or in the person's folders,
        // by what they take there.
        let here = OnDevice::of(store, work.as_str());
        let file = |row: &ChapterRow| here.file(row.id.as_str());
        let picked = Self::to_fetch(&pick, rows);
        let chosen: Vec<&ChapterRow> =
            rows.iter().filter(|row| picked.iter().any(|w| w.chapter == row.id)).collect();
        let wanted: Vec<&ChapterRow> = chosen
            .iter()
            .copied()
            .filter(|row| !have.contains(row.id.as_str()) && file(row).is_none())
            .collect();
        // A finished download whose file is gone still says what it took.
        let done: HashMap<&str, u64> = all
            .iter()
            .filter(|d| d.work == work.as_str() && d.state == BookDownloadState::Done)
            .map(|d| (d.chapter.as_str(), d.bytes))
            .collect();
        let learnt = learn(store, &all, work.as_str(), source);
        let novel = is_novel(store, work.as_str());
        let guess = |row: &&ChapterRow| -> Option<u64> {
            if novel {
                learnt.text_chapter
            } else {
                let pages = crate::read_catalog::chapters::plausible_pages(row.pages)
                    .filter(|p| *p > 0)
                    .or(learnt.pages)
                    .unwrap_or(PAGES);
                learnt.page_bytes.map(|page| u64::from(pages) * page)
            }
        };
        let bytes: Option<u64> = wanted.iter().map(guess).sum();
        let all_bytes: Option<u64> = chosen
            .iter()
            .map(|row| {
                file(row)
                    .and_then(|f| f.bytes())
                    .or_else(|| done.get(row.id.as_str()).copied())
                    .or_else(|| guess(row))
            })
            .sum();
        let mut line = Vec::new();
        if let Some(bytes) = bytes.filter(|_| !wanted.is_empty()) {
            line.push(r(ReadingMsg::ReadingEstimateSize { size: Size::of(bytes) }));
            line.extend(network(&self.setup()));
        }
        let (bytes, all_bytes) = (bytes.unwrap_or(0), all_bytes.unwrap_or(0));
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

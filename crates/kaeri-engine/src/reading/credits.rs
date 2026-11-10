//! Credit pages: a page a series repeats at the start or end of its chapters (a group's credits,
//! a recruitment notice) is learnt from the pages already fetched, and skipped.
//!
//! Each chapter's first two and last two pages are fingerprinted where they are fetched: by the
//! reader as it streams a chapter, by downloads as they write one, and by the page worker for a
//! file opened with none kept. A chapter in parts has the edges of each part. A page whose
//! fingerprint is the same page (see [`kaeri_pages::print::same_page`]) at an edge of
//! [`CHAPTERS`] or more chapters of the series from the same group is a credit page, unless the
//! person said it is not. A chapter whose group is not known counts with any group. Nothing more is asked of the source, and nothing is deleted: the reader leaves the page
//! out of the chapter's pages and says so, and the page is a tap away.

use std::collections::HashSet;
use std::path::Path;

use kaeri_pages::print::{fingerprint, same_page};
use kaeri_store::Store;
use kaeri_store::book_credits::{KeptPrint, PagePrint};

/// Chapters a page must open or close before it counts as a credit page. Two chapters can share
/// a page by chance (a two-part chapter's recap, a spread split across a break); a page in three
/// is the group's.
pub(crate) const CHAPTERS: usize = 3;

/// Pages at each edge of a chapter, or of each part, that are fingerprinted.
const EDGE: u32 = 2;

/// The pages fingerprinted in a chapter whose parts start and run as `spans` (start, pages):
/// each part's first and last [`EDGE`], with whether the page closes its part. A page that is
/// both is taken as a start.
pub(crate) fn edges(spans: &[(u32, u32)]) -> Vec<(u32, bool)> {
    let mut out: Vec<(u32, bool)> = Vec::new();
    for &(start, len) in spans {
        for page in start..start + len.min(EDGE) {
            out.push((page, false));
        }
        for page in (start + len.saturating_sub(EDGE)).max(start)..start + len {
            if !out.iter().any(|(p, _)| *p == page) {
                out.push((page, true));
            }
        }
    }
    out
}

/// What the page worker needs to find a chapter's credit pages.
#[derive(Clone)]
pub struct CreditCheck {
    pub store: Store,
    pub work: String,
    pub chapter: String,
    pub now: i64,
}

impl std::fmt::Debug for CreditCheck {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreditCheck")
            .field("work", &self.work)
            .field("chapter", &self.chapter)
            .finish()
    }
}

/// The pages of `chapter`, read in `group`, that are credit pages, from its own edge
/// fingerprints, those of the work's other chapters in the same group, and the fingerprints the
/// person kept.
pub(crate) fn credit_pages(
    all: &[KeptPrint],
    kept: &[u64],
    (chapter, group): (&str, Option<&str>),
    mine: &[PagePrint],
) -> Vec<PagePrint> {
    let same_group = |other: &KeptPrint| match (other.group_label.as_deref(), group) {
        (Some(theirs), Some(ours)) => theirs == ours,
        _ => true,
    };
    mine.iter()
        .filter(|p| !kept.iter().any(|k| same_page(*k, p.print)))
        .filter(|p| {
            let mut chapters: HashSet<&str> = HashSet::from([chapter]);
            for other in all.iter().filter(|o| same_group(o) && same_page(o.print, p.print)) {
                chapters.insert(other.chapter.as_str());
            }
            chapters.len() >= CHAPTERS
        })
        .copied()
        .collect()
}

impl CreditCheck {
    /// The credit pages of the chapter whose pages are these files, in order. Edge pages with
    /// no fingerprint kept for the chapter are fingerprinted now, from the files, and kept.
    /// Never every page: a chapter that would be all credits is shown whole.
    pub(crate) fn find(&self, files: &[Option<&Path>]) -> Vec<PagePrint> {
        let count = u32::try_from(files.len()).unwrap_or(u32::MAX);
        if count == 0 {
            return Vec::new();
        }
        let mut all = self.store.page_prints(&self.work).unwrap_or_default();
        // The group the chapter was read in, as its fingerprints were kept.
        let group = all
            .iter()
            .find(|p| p.chapter == self.chapter && p.group_label.is_some())
            .and_then(|p| p.group_label.clone());
        let mut mine: Vec<PagePrint> = all
            .iter()
            .filter(|p| p.chapter == self.chapter && p.page < count)
            .map(|p| PagePrint { page: p.page, at_end: p.at_end, print: p.print })
            .collect();
        let fresh: Vec<PagePrint> = edges(&[(0, count)])
            .into_iter()
            .filter(|(page, _)| !mine.iter().any(|p| p.page == *page))
            .filter_map(|(page, at_end)| {
                let path = files.get(page as usize).copied().flatten()?;
                let print = fingerprint(&std::fs::read(path).ok()?)?;
                Some(PagePrint { page, at_end, print })
            })
            .collect();
        if !fresh.is_empty() {
            if let Err(e) =
                self.store.put_page_prints(&self.work, &self.chapter, &fresh, None, self.now)
            {
                tracing::warn!(error = %e, "a chapter's page fingerprints were not kept");
            }
            all.extend(fresh.iter().map(|p| KeptPrint {
                chapter: self.chapter.clone(),
                page: p.page,
                at_end: p.at_end,
                print: p.print,
                group_label: None,
            }));
            mine.extend(fresh);
        }
        let kept = self.store.kept_credit_pages(&self.work).unwrap_or_default();
        whole_or_not(credit_pages(&all, &kept, (&self.chapter, group.as_deref()), &mine), count)
    }
}

/// The credit pages found, in order, unless they are all `count` pages of the chapter: a chapter
/// that would be all credits is shown whole.
fn whole_or_not(mut credits: Vec<PagePrint>, count: u32) -> Vec<PagePrint> {
    credits.sort_by_key(|p| p.page);
    credits.dedup_by_key(|p| p.page);
    if credits.len() >= count as usize {
        return Vec::new();
    }
    credits
}

/// What a source's chapter needs, as it is fetched, to learn the series' credit pages and leave
/// them out.
#[derive(Clone)]
pub(crate) struct Learning {
    pub store: Store,
    pub work: String,
    /// The person has credit pages left out.
    pub skip: bool,
}

impl Learning {
    /// Keep the edge fingerprints `mine` of `chapter`, read in `group`, in place of those kept
    /// for it before, and give the credit pages among them when the person leaves them out.
    pub(crate) fn keep_and_find(
        &self,
        (chapter, group): (&str, Option<&str>),
        mine: &[PagePrint],
        count: u32,
        now: i64,
    ) -> Vec<PagePrint> {
        if mine.is_empty() {
            return Vec::new();
        }
        if let Err(e) = self
            .store
            .forget_page_prints(&self.work, chapter)
            .and_then(|()| self.store.put_page_prints(&self.work, chapter, mine, group, now))
        {
            tracing::warn!(error = %e, "a chapter's page fingerprints were not kept");
        }
        if !self.skip {
            return Vec::new();
        }
        let all = self.store.page_prints(&self.work).unwrap_or_default();
        let kept = self.store.kept_credit_pages(&self.work).unwrap_or_default();
        whole_or_not(credit_pages(&all, &kept, (chapter, group), mine), count)
    }
}

/// Fingerprint the edge pages of a chapter as they are fetched, off the async threads, and keep
/// them with the group the chapter is read in.
pub(crate) struct Recorder {
    spans: Vec<(u32, bool)>,
    prints: Vec<PagePrint>,
}

impl Recorder {
    /// For a chapter whose parts start and run as `spans`.
    pub(crate) fn new(spans: &[(u32, u32)]) -> Self {
        Self { spans: edges(spans), prints: Vec::new() }
    }

    /// Take page `index`'s bytes, when it is at an edge.
    pub(crate) async fn page(&mut self, index: u32, bytes: &[u8]) {
        let Some(&(page, at_end)) = self.spans.iter().find(|(p, _)| *p == index) else {
            return;
        };
        let bytes = bytes.to_vec();
        if let Ok(Some(print)) = tokio::task::spawn_blocking(move || fingerprint(&bytes)).await
        {
            self.prints.push(PagePrint { page, at_end, print });
        }
    }

    /// Keep what was taken. The chapter's earlier fingerprints go first: its pages may have
    /// changed, as when a part was added.
    pub(crate) fn keep(
        self,
        store: &Store,
        work: &str,
        chapter: &str,
        group: Option<&str>,
        now: i64,
    ) {
        if self.prints.is_empty() {
            return;
        }
        if let Err(e) = store
            .forget_page_prints(work, chapter)
            .and_then(|()| store.put_page_prints(work, chapter, &self.prints, group, now))
        {
            tracing::warn!(error = %e, "a chapter's page fingerprints were not kept");
        }
    }
}

#[cfg(test)]
mod tests;

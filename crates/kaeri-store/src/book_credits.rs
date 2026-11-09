//! Credit pages: the fingerprints of the pages at the edges of each chapter, and the pages the
//! person said are not credit pages.
//!
//! `book_page_print` stays on the device, as the chapter lists do; `book_credit_keep` is the
//! person's own and follows the profile.

use rusqlite::params;

use crate::{Result, Store};

/// One fingerprinted page at a chapter's edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PagePrint {
    /// From 0, through the chapter as read, parts and all.
    pub page: u32,
    /// At the end of a part rather than its start.
    pub at_end: bool,
    /// A 64-bit dHash.
    pub print: u64,
}

/// A fingerprint as kept for a work, with the chapter it was taken in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeptPrint {
    pub chapter: String,
    pub page: u32,
    pub at_end: bool,
    pub print: u64,
    pub group_label: Option<String>,
}

/// SQLite keeps 64-bit integers signed; a fingerprint's bits go in as they are.
fn to_sql(print: u64) -> i64 {
    i64::from_ne_bytes(print.to_ne_bytes())
}

fn from_sql(value: i64) -> u64 {
    u64::from_ne_bytes(value.to_ne_bytes())
}

impl Store {
    /// Keep a chapter's edge fingerprints, replacing what was kept for those pages.
    pub fn put_page_prints(
        &self,
        work: &str,
        chapter: &str,
        prints: &[PagePrint],
        group_label: Option<&str>,
        at: i64,
    ) -> Result<()> {
        if prints.is_empty() {
            return Ok(());
        }
        self.with_tx(|tx| {
            let mut put = tx.prepare(
                "INSERT OR REPLACE INTO book_page_print
                     (work, chapter, page, edge, print, group_label, at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for p in prints {
                put.execute(params![
                    work,
                    chapter,
                    p.page,
                    i64::from(p.at_end),
                    to_sql(p.print),
                    group_label,
                    at
                ])?;
            }
            Ok(())
        })
    }

    /// Every edge fingerprint kept for a work.
    pub fn page_prints(&self, work: &str) -> Result<Vec<KeptPrint>> {
        self.with_conn(|c| {
            let mut stmt = c.prepare(
                "SELECT chapter, page, edge, print, group_label FROM book_page_print
                  WHERE work = ?1 ORDER BY chapter, page",
            )?;
            let rows = stmt.query_map([work], |r| {
                Ok(KeptPrint {
                    chapter: r.get(0)?,
                    page: r.get(1)?,
                    at_end: r.get::<_, i64>(2)? != 0,
                    print: from_sql(r.get(3)?),
                    group_label: r.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// Forget a chapter's fingerprints: its pages changed, as a chapter whose parts were merged
    /// counts its pages again.
    pub fn forget_page_prints(&self, work: &str, chapter: &str) -> Result<()> {
        self.with_conn(|c| {
            c.execute(
                "DELETE FROM book_page_print WHERE work = ?1 AND chapter = ?2",
                params![work, chapter],
            )?;
            Ok(())
        })
    }

    /// The person said the page with this fingerprint is not a credit page.
    pub fn keep_credit_page(&self, work: &str, print: u64, at: i64) -> Result<()> {
        self.with_personal(|c| {
            c.execute(
                "INSERT OR REPLACE INTO book_credit_keep (work, print, at) VALUES (?1, ?2, ?3)",
                params![work, to_sql(print), at],
            )?;
            Ok(())
        })
    }

    /// The fingerprints the person said are not credit pages, in a work.
    pub fn kept_credit_pages(&self, work: &str) -> Result<Vec<u64>> {
        self.with_personal(|c| {
            let mut stmt =
                c.prepare("SELECT print FROM book_credit_keep WHERE work = ?1 ORDER BY at")?;
            let rows = stmt.query_map([work], |r| r.get::<_, i64>(0).map(from_sql))?;
            Ok(rows.collect::<rusqlite::Result<_>>()?)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prints_and_keeps_go_round_the_store() {
        let store = Store::open_in_memory().unwrap();
        let work = "a:900301";
        let high = 0xF0F0_0000_0000_0001_u64;
        let prints = [
            PagePrint { page: 0, at_end: false, print: high },
            PagePrint { page: 23, at_end: true, print: 42 },
        ];
        store.put_page_prints(work, "3", &prints, Some("Group Example"), 10).unwrap();
        store.put_page_prints(work, "4", &prints[..1], None, 11).unwrap();
        // The same page again replaces it.
        let again = [PagePrint { page: 23, at_end: true, print: 43 }];
        store.put_page_prints(work, "3", &again, Some("Group Example"), 12).unwrap();
        let kept = store.page_prints(work).unwrap();
        assert_eq!(kept.len(), 3);
        assert_eq!(
            kept[0],
            KeptPrint {
                chapter: "3".into(),
                page: 0,
                at_end: false,
                print: high,
                group_label: Some("Group Example".into()),
            }
        );
        assert_eq!((kept[1].page, kept[1].at_end, kept[1].print), (23, true, 43));
        assert_eq!((kept[2].chapter.as_str(), kept[2].group_label.as_deref()), ("4", None));
        assert!(store.page_prints("a:900302").unwrap().is_empty());
        store.forget_page_prints(work, "3").unwrap();
        assert_eq!(store.page_prints(work).unwrap().len(), 1);

        store.keep_credit_page(work, high, 20).unwrap();
        store.keep_credit_page(work, high, 21).unwrap();
        assert_eq!(store.kept_credit_pages(work).unwrap(), vec![high]);
        assert!(store.kept_credit_pages("a:900302").unwrap().is_empty());
    }
}

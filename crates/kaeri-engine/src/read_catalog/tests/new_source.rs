//! A reading source added, updated or filled while the app runs: a book plugin installed from a
//! store, a book server added, a folder scanned. What reading worked out against the sources
//! before is not the answer once they change: the open series page, Read from, Home, the reading
//! list and Search hear what the sources now say, and series kept as not found off screen are
//! asked again one at a time.

use std::{
    sync::atomic::{AtomicUsize, Ordering as AtomicOrdering},
    time::Duration,
};

use kaeri_core::books::WorkId;
use kaeri_model::{book_sources::BookSourceEvent, reading::LibrarySegmentRead};

use super::*;
use crate::read_catalog::{changed, editions, series};

const WORK_ID: u32 = 914_001;
const TITLE: &str = "The Salt Road Archivist";

/// A source that holds the series named in `titles`, each with three chapters. It counts its
/// searches, and how many run at once at most.
struct Stall {
    manifest: BookSourceManifest,
    titles: Vec<String>,
    searched: AtomicU32,
    running: AtomicUsize,
    most: AtomicUsize,
    /// Its chapter lists come back empty, as a plugin's broken release lists none.
    empty: bool,
    /// How many chapters its lists hold.
    count: u32,
}

impl Stall {
    fn new(id: &str, version: &str, titles: &[&str]) -> Arc<Self> {
        Arc::new(Self {
            manifest: BookSourceManifest {
                id: id.into(),
                name: format!("{id} books"),
                version: version.into(),
                kind: BookSourceKind::Native,
                media: vec![Medium::Manga],
                languages: vec!["en".into()],
                allowed_hosts: Vec::new(),
                capabilities: Vec::new(),
            },
            titles: titles.iter().map(|t| (*t).to_owned()).collect(),
            searched: AtomicU32::new(0),
            running: AtomicUsize::new(0),
            most: AtomicUsize::new(0),
            empty: false,
            count: 3,
        })
    }

    fn listing(self: Arc<Self>, count: u32) -> Arc<Self> {
        let mut this = Arc::into_inner(self).unwrap();
        this.count = count;
        Arc::new(this)
    }

    fn listing_nothing(self: Arc<Self>) -> Arc<Self> {
        let mut this = Arc::into_inner(self).unwrap();
        this.empty = true;
        Arc::new(this)
    }

    fn put(self: &Arc<Self>, kit: &Kit) {
        kit.sources.registry.put(self.clone());
    }

    fn searches(&self) -> u32 {
        self.searched.load(AtomicOrdering::SeqCst)
    }
}

#[async_trait]
impl BookSource for Stall {
    fn manifest(&self) -> &BookSourceManifest {
        &self.manifest
    }

    async fn search(
        &self,
        query: &str,
        _: &BookHints,
    ) -> Result<Vec<SeriesHit>, ProviderError> {
        self.searched.fetch_add(1, AtomicOrdering::SeqCst);
        let now = self.running.fetch_add(1, AtomicOrdering::SeqCst) + 1;
        self.most.fetch_max(now, AtomicOrdering::SeqCst);
        tokio::time::sleep(Duration::from_millis(20)).await;
        self.running.fetch_sub(1, AtomicOrdering::SeqCst);
        Ok(self
            .titles
            .iter()
            .filter(|t| t.eq_ignore_ascii_case(query.trim()))
            .map(|t| SeriesHit::new(format!("s-{}", t.len()), t.clone()))
            .collect())
    }

    async fn series(&self, id: &str, _: &BookHints) -> Result<SeriesDetail, ProviderError> {
        let title = self.titles.iter().find(|t| format!("s-{}", t.len()) == id);
        Ok(SeriesDetail::new(id, title.map_or(id, String::as_str)))
    }

    async fn chapters(
        &self,
        _: &str,
        _: &BookHints,
    ) -> Result<Vec<SourceChapter>, ProviderError> {
        if self.empty {
            return Ok(Vec::new());
        }
        Ok((1..=self.count)
            .map(|n| SourceChapter::new(format!("c-{n}"), n.to_string()))
            .collect())
    }

    async fn content(&self, _: &str, _: &str) -> Result<ChapterContent, ProviderError> {
        Err(ProviderError::NotFound)
    }
}

/// What the page and the other screens hear, in a word each.
fn heard(event: &Event) -> Option<String> {
    match event {
        Event::Reading(ReadingEvent::Series(view)) => {
            Some(format!("series({:?})", view.action))
        }
        Event::Reading(ReadingEvent::Chapters { volumes, failure, .. }) => {
            let rows: usize = volumes.iter().map(|v| v.chapters.len()).sum();
            Some(match failure {
                Some(failure) => format!("chapters failed({failure:?})"),
                None => format!("chapters({rows})"),
            })
        }
        Event::BookSources(BookSourceEvent::ForWork {
            work,
            asking,
            missing,
            editions,
            ..
        }) => Some(format!(
            "read-from({}, asking: {asking}, editions: {}, missing: {})",
            work.as_str(),
            editions.len(),
            missing.len()
        )),
        Event::Reading(ReadingEvent::Home(_)) => Some("home".into()),
        Event::Reading(ReadingEvent::Library { segment, .. }) => {
            Some(format!("library({segment:?})"))
        }
        Event::Reading(ReadingEvent::SourceSearch(part)) => {
            let groups: Vec<String> =
                part.groups.iter().map(|g| format!("{}={:?}", g.source_id, g.state)).collect();
            Some(format!("search[{}]", groups.join(", ")))
        }
        _ => None,
    }
}

/// Everything sent until nothing more comes for `quiet`.
async fn drain(rx: &mut mpsc::UnboundedReceiver<Event>, quiet: Duration) -> Vec<String> {
    let mut out = Vec::new();
    while let Ok(Some(event)) = tokio::time::timeout(quiet, rx.recv()).await {
        out.extend(heard(&event));
    }
    out
}

async fn kit_with(fake: &FakeAniList, dir: &std::path::Path, books: &[(u32, &str)]) -> Kit {
    fake.add_books(
        books.iter().map(|(id, title)| book(*id, title, "MANGA", (2021, 500), &[], &[])),
    );
    let kit = kit(Some(fake), dir);
    for (id, _) in books {
        kit.meta.book(kaeri_core::AnilistId(*id)).await.unwrap();
    }
    kit
}

fn work_of(id: u32) -> WorkId {
    WorkId::new(format!("a:{id}"))
}

/// Open a series as the apps do: the page, its chapters, then Read from.
fn open(kit: &Kit, work: &WorkId, tx: &Outbox) {
    series::load(kit, ask(), work.clone(), tx);
    crate::reading::chapters_of(kit.clone(), ask(), work.clone(), None, tx);
    editions::load(kit, ask(), work.clone(), tx);
}

const QUIET: Duration = Duration::from_millis(400);

/// The bug as reported: a series opened while no source had it, then a plugin that has it
/// installed. The open page fills in, and Read from lists the new source, with nothing tapped.
#[tokio::test]
async fn an_open_page_fills_in_when_a_source_that_has_its_series_joins() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit_with(&fake, dir.path(), &[(WORK_ID, TITLE)]).await;
    Stall::new("first", "1", &[]).put(&kit);
    let work = work_of(WORK_ID);
    let (tx, mut rx) = outbox();
    open(&kit, &work, &tx);
    let before = drain(&mut rx, QUIET).await;
    assert!(before.contains(&"chapters failed(NoSource)".to_owned()), "{before:#?}");

    // Installed from a store: the engine is told the sources changed.
    let added = Stall::new("second", "1", &[TITLE]);
    added.put(&kit);
    changed::sources_changed(&kit, ask(), &tx);
    let after = drain(&mut rx, QUIET).await;
    assert!(
        after.contains(&"chapters(3)".to_owned()),
        "the page lists its chapters: {after:#?}"
    );
    assert!(
        after.iter().any(|e| e.starts_with("series(Read")),
        "its button reads the first chapter: {after:#?}"
    );
    let last_read_from = after.iter().rev().find(|e| e.starts_with("read-from")).unwrap();
    assert_eq!(
        last_read_from,
        &format!("read-from(a:{WORK_ID}, asking: false, editions: 1, missing: 1)")
    );
    assert!(added.searches() > 0);
}

/// A source updated since it said it had no such series is asked again when the series opens,
/// rather than held to its old answer for ten minutes.
#[tokio::test]
async fn a_source_updated_since_it_missed_is_asked_again() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit_with(&fake, dir.path(), &[(WORK_ID, TITLE)]).await;
    let old = Stall::new("lantern", "1.0.0", &[]);
    old.put(&kit);
    let work = work_of(WORK_ID);
    let (tx, mut rx) = outbox();
    editions::load(&kit, ask(), work.clone(), &tx);
    let first = drain(&mut rx, QUIET).await;
    let said = first.iter().rev().find(|e| e.starts_with("read-from")).unwrap();
    assert!(said.ends_with("editions: 0, missing: 1)"), "{first:#?}");

    // Asked again at once, unchanged: its miss holds.
    editions::load(&kit, ask(), work.clone(), &tx);
    drain(&mut rx, QUIET).await;
    let asked = old.searches();

    let updated = Stall::new("lantern", "1.1.0", &[TITLE]);
    updated.put(&kit);
    editions::load(&kit, ask(), work.clone(), &tx);
    let again = drain(&mut rx, QUIET).await;
    assert!(updated.searches() > 0, "the update is asked: {again:#?}");
    assert_eq!(old.searches(), asked);
    let said = again.iter().rev().find(|e| e.starts_with("read-from")).unwrap();
    assert!(said.ends_with("editions: 1, missing: 0)"), "{again:#?}");
}

/// A chapter list an older release answered empty is not held against the update: its edition
/// is asked for its list again.
#[tokio::test]
async fn an_empty_list_from_before_an_update_is_asked_again() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit_with(&fake, dir.path(), &[(WORK_ID, TITLE)]).await;
    Stall::new("lantern", "1.0.0", &[TITLE]).listing_nothing().put(&kit);
    let work = work_of(WORK_ID);
    let (tx, mut rx) = outbox();
    open(&kit, &work, &tx);
    let before = drain(&mut rx, QUIET).await;
    assert!(!before.contains(&"chapters(3)".to_owned()), "{before:#?}");

    Stall::new("lantern", "1.1.0", &[TITLE]).put(&kit);
    changed::sources_changed(&kit, ask(), &tx);
    let after = drain(&mut rx, QUIET).await;
    assert!(after.contains(&"chapters(3)".to_owned()), "{after:#?}");
}

/// Home's "not in your sources" holds only for the sources the series was asked of, and not once
/// a series of a source is matched to it, as a folder's scan matches one.
#[tokio::test]
async fn home_says_not_found_only_while_the_sources_it_was_asked_of_stand() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit_with(&fake, dir.path(), &[(WORK_ID, TITLE)]).await;
    let row = json!({ "at": NOW - 60, "ids": [WORK_ID] });
    kit.store.set_meta_text("read:home:row:trending-manga", &row.to_string()).unwrap();
    Stall::new("first", "1", &[]).put(&kit);
    let work = work_of(WORK_ID);
    let (tx, mut rx) = outbox();
    editions::load(&kit, ask(), work.clone(), &tx);
    drain(&mut rx, QUIET).await;

    let flagged = |rx: &mut mpsc::UnboundedReceiver<Event>| {
        home::resend(&kit, ask(), &tx);
        let Ok(Event::Reading(ReadingEvent::Home(home))) = rx.try_recv() else {
            panic!("Home is sent")
        };
        let row = home.rows.iter().find(|r| r.id == "trending-manga").expect("the row");
        row.tiles.iter().find(|t| t.work == work).expect("the tile").not_in_sources
    };
    assert!(flagged(&mut rx), "no source had it");

    // A source joined that was never asked.
    let second = Stall::new("second", "1", &[]);
    second.put(&kit);
    assert!(!flagged(&mut rx), "asked of other sources");
    editions::load(&kit, ask(), work.clone(), &tx);
    drain(&mut rx, QUIET).await;
    assert!(flagged(&mut rx), "asked of these too, and none had it");

    // A folder's scan matched a series of it.
    kit.store
        .set_book_match(
            crate::read_sources::FOLDERS,
            "group-1",
            work.as_str(),
            95,
            false,
            false,
            NOW,
        )
        .unwrap();
    assert!(!flagged(&mut rx), "a folder has it");
}

/// The reading list, Home and the search with words are sent again when the sources change, and
/// the search asks the source that joined, alone.
#[tokio::test]
async fn home_the_reading_list_and_search_hear_a_new_source() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let kit = kit_with(&fake, dir.path(), &[(WORK_ID, TITLE)]).await;
    let first = Stall::new("first", "1", &[]);
    first.put(&kit);
    kit.store.set_read_status(&format!("a:{WORK_ID}"), Some("reading"), NOW).unwrap();
    let (tx, mut rx) = outbox();
    home::load(&kit, ask(), false, &tx);
    search::library(&kit, ask(), LibrarySegmentRead::Reading, &tx);
    search::search(&kit, ask(), query(TITLE), 0, &tx);
    drain(&mut rx, QUIET).await;
    let asked = first.searches();

    let second = Stall::new("second", "1", &[TITLE]);
    second.put(&kit);
    changed::sources_changed(&kit, ask(), &tx);
    let after = drain(&mut rx, QUIET).await;
    assert!(after.contains(&"home".to_owned()), "{after:#?}");
    assert!(after.contains(&"library(Reading)".to_owned()), "{after:#?}");
    let last = after.iter().rev().find(|e| e.starts_with("search")).expect("search");
    assert_eq!(last, "search[folders=Nothing, first=Nothing, second=Hits]");
    assert_eq!(first.searches(), asked, "the sources that answered are not asked again");
}

/// Series told this session that no source had them, and not on screen, are asked again behind
/// it one at a time once a source that has them joins, and Home stops saying so.
#[tokio::test]
async fn series_not_found_off_screen_are_asked_again_one_at_a_time() {
    let fake = FakeAniList::start().await;
    let dir = tempfile::tempdir().unwrap();
    let others = [(WORK_ID + 1, "Ninefold Kettle"), (WORK_ID + 2, "Winter Orchard Ledger")];
    let kit = kit_with(&fake, dir.path(), &others).await;
    Stall::new("first", "1", &[]).put(&kit);
    let (tx, mut rx) = outbox();
    // Read from was opened for each, and no page is on screen.
    for (id, _) in others {
        editions::load(&kit, ask(), work_of(id), &tx);
    }
    drain(&mut rx, QUIET).await;
    assert_eq!(kit.sources.catalog.not_found.lock().unwrap().len(), 2);

    let second = Stall::new("second", "1", &others.map(|(_, t)| t));
    second.put(&kit);
    changed::sources_changed(&kit, ask(), &tx);
    assert!(kit.sources.catalog.not_found.lock().unwrap().is_empty(), "Home stops saying so");
    let after = drain(&mut rx, Duration::from_secs(3)).await;
    for (id, _) in others {
        let found = format!("read-from(a:{id}, asking: false, editions: 1, missing: 1)");
        assert!(after.contains(&found), "{after:#?}");
    }
    assert_eq!(second.most.load(AtomicOrdering::SeqCst), 1, "one search at a time");
    assert!(!after.iter().any(|e| e.starts_with("series")), "no page was opened: {after:#?}");
}

/// The Sources screen's changes tell the engine to bring reading up to date: a source switched
/// on or off, moved, a server removed, a folder's series matched, and a folder scanned.
#[tokio::test]
async fn the_sources_screen_tells_the_engine_when_the_sources_change() {
    use kaeri_model::book_sources::{BookSourceCommand, FolderRef};

    use crate::outbox::{Envelope, Internal};

    let dir = tempfile::tempdir().unwrap();
    let kit = kit(None, dir.path());
    let books = dir.path().join("Shelf");
    std::fs::create_dir_all(&books).unwrap();
    let commands = [
        BookSourceCommand::SetOff { id: "first".into(), off: true },
        BookSourceCommand::SetOrder { ids: vec!["first".into()] },
        BookSourceCommand::RemoveServer { id: "home-server".into() },
        BookSourceCommand::ChooseMatch {
            group: "group-1".into(),
            anilist: kaeri_core::AnilistId(WORK_ID),
        },
        BookSourceCommand::AddFolder {
            folder: FolderRef {
                path: Some(books.display().to_string()),
                ..FolderRef::default()
            },
        },
    ];
    for command in commands {
        let about = format!("{command:?}");
        let (tx, mut inbox) = Outbox::inbox();
        crate::read_sources::command(&kit.sources, &kit.plugins, |_| None, command, NOW, &tx);
        let told = tokio::time::timeout(PATIENCE, async {
            while let Some(envelope) = inbox.recv().await {
                if matches!(envelope, Envelope::Internal(Internal::BookSourcesChanged)) {
                    return true;
                }
            }
            false
        })
        .await;
        assert_eq!(told, Ok(true), "{about}");
    }
}

/// A folder picked again while it is already a source says so, so the dialog never takes it
/// back out; one waiting out its removal's undo comes back as new; and a folder that can't be
/// read answers as one, so the dialog leaves its scan.
#[tokio::test]
async fn a_folder_picked_again_says_it_is_already_a_source() {
    use kaeri_model::{
        book_sources::{BookSourceCommand, FolderRef},
        came_in::{CameIn, FolderReport, NothingCameIn},
    };

    use crate::outbox::Envelope;

    let dir = tempfile::tempdir().unwrap();
    let kit = kit(None, dir.path());
    let shelf = dir.path().join("Shelf");
    std::fs::create_dir_all(&shelf).unwrap();
    let add = |path: &std::path::Path| BookSourceCommand::AddFolder {
        folder: FolderRef { path: Some(path.display().to_string()), ..FolderRef::default() },
    };
    // The folder's answer, and whether it says the folder couldn't be read.
    let answer = async |command: BookSourceCommand| -> (FolderReport, bool) {
        let (tx, mut inbox) = Outbox::inbox();
        crate::read_sources::command(&kit.sources, &kit.plugins, |_| None, command, NOW, &tx);
        tokio::time::timeout(PATIENCE, async {
            while let Some(envelope) = inbox.recv().await {
                match envelope {
                    Envelope::Event(Event::CameIn(
                        CameIn::Folder(report)
                        | CameIn::Empty(
                            NothingCameIn::EmptyFolder { folder: report }
                            | NothingCameIn::UnsupportedFiles { folder: report },
                        ),
                    )) => return (report, false),
                    Envelope::Event(Event::CameIn(CameIn::Empty(
                        NothingCameIn::FolderUnreadable { folder: report },
                    ))) => return (report, true),
                    _ => {}
                }
            }
            panic!("the inbox closed before the folder's answer");
        })
        .await
        .expect("the folder answers")
    };

    let (first, unreadable) = answer(add(&shelf)).await;
    assert!(!first.already_added && !unreadable, "{first:?}");
    let (again, _) = answer(add(&shelf)).await;
    assert_eq!(again.id, first.id);
    assert!(again.already_added, "picked again: {again:?}");

    let (tx, _inbox) = Outbox::inbox();
    let remove = BookSourceCommand::RemoveFolder { id: first.id.clone() };
    crate::read_sources::command(&kit.sources, &kit.plugins, |_| None, remove, NOW, &tx);
    let (back, _) = answer(add(&shelf)).await;
    assert!(!back.already_added, "it had left the list: {back:?}");

    let (gone, unreadable) = answer(add(&dir.path().join("Gone"))).await;
    assert!(unreadable && !gone.already_added, "{gone:?}");

    // A source that can no longer be read, picked again, says so and is still a source: the
    // dialog's Choose another folder must not take it out.
    std::fs::remove_dir_all(&shelf).unwrap();
    let (lost, unreadable) = answer(add(&shelf)).await;
    assert!(unreadable && lost.already_added, "{lost:?}");
}

/// The bug as reported: a series read from one source, and a source installed since that has
/// more of its chapters. Opening the page searches the new source, which matches and becomes the
/// edition read from. The page then names it and lists its chapters, the ones a read opens,
/// rather than keeping the first source's list.
#[tokio::test]
async fn the_page_follows_when_a_source_found_on_opening_becomes_the_edition_read_from() {
    let fake = FakeAniList::start().await;
    let mut entry = book(WORK_ID, TITLE, "MANGA", (2021, 500), &[], &[]);
    entry["chapters"] = json!(40);
    fake.add_books([entry]);
    let dir = tempfile::tempdir().unwrap();
    let kit = kit(Some(&fake), dir.path());
    kit.meta.book(kaeri_core::AnilistId(WORK_ID)).await.unwrap();
    Stall::new("first", "1", &[TITLE]).put(&kit);
    Stall::new("second", "1", &[TITLE]).listing(40).put(&kit);
    kit.sources.registry.set_order(vec!["first".into(), "second".into()]);
    let work = work_of(WORK_ID);
    let series_ref = format!("s-{}", TITLE.len());
    let matched = editions::MATCHED;
    kit.store
        .set_book_match("first", &series_ref, work.as_str(), matched, false, false, NOW)
        .unwrap();

    let (tx, mut rx) = outbox();
    open(&kit, &work, &tx);
    let mut named = None;
    let mut listed = None;
    while let Ok(Some(event)) = tokio::time::timeout(QUIET, rx.recv()).await {
        match event {
            Event::Reading(ReadingEvent::Series(view)) => {
                named = view.edition.map(|e| e.source_id);
            }
            Event::Reading(ReadingEvent::Chapters { edition, volumes, .. }) => {
                let rows: usize = volumes.iter().map(|v| v.chapters.len()).sum();
                listed = Some((edition.map(|e| e.source_id), rows));
            }
            _ => {}
        }
    }

    let read_from = kit.sources.edition_of(&work, None).map(|(source, _)| source);
    assert_eq!(read_from.as_deref(), Some("second"), "the new source has the work's chapters");
    assert_eq!(named.as_deref(), Some("second"), "the page names the source read from");
    assert_eq!(listed, Some((Some("second".to_owned()), 40)), "and lists its chapters");
}

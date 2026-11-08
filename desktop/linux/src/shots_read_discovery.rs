//! Read's discovery shots: Home's rows on a first run and with history, and Search with words in
//! each of its states. The fixture engine has no AniList discovery and no sources' search, so the
//! shots answer reading's commands themselves with made-up views, at the speed a slow source
//! would, and give the screens back to the engine when they end.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_core::{books::WorkId, media::Medium};
use kaeri_model::{
    Msg, ReadingMsg,
    event::Event,
    reading::{
        ReadAnilistState, ReadHomeEnd, ReadHomeRow, ReadRowFormat, ReadRowKind,
        ReadSearchQuery, ReadSourceGroup, ReadSourceJoin, ReadSourceSearch, ReadSourceState,
        ReadSpotlight, ReadSpotlightCase, ReadingCommand, ReadingEvent, ReadingHomeView,
        WorkTile, count_words, kind_words, status_words,
    },
};

use crate::{hub::ReadAnswer, pages::Dest, window::Shell};

/// Every discovery shot, in the order a run takes them.
pub const NAMES: [&str; 12] = [
    "read-disc-home-first",
    "read-disc-home-first-end",
    "read-disc-home-history",
    "read-disc-home-history-end",
    "read-disc-home-loading",
    "read-disc-search-mid",
    "read-disc-search-mid-end",
    "read-disc-search-failed",
    "read-disc-search-mine",
    "read-disc-search-mine-end",
    "read-disc-search-nothing",
    "read-disc-search-end",
];

const HOME_LIBRARY: &str = "shot-home-library";
const COMICS_FOLDER: &str = "shot-comics-folder";
const NORTHWIND: &str = "shot-northwind";
const DEMO: &str = "demo";

/// A made-up title as AniList would list it: its cover from the fake AniList's art, by number.
struct Book {
    title: &'static str,
    art: u32,
    format: &'static str,
    origin: &'static str,
    status: &'static str,
    chapters: Option<u32>,
    volumes: Option<u32>,
}

const fn book(
    title: &'static str,
    art: u32,
    format: &'static str,
    origin: &'static str,
) -> Book {
    Book { title, art, format, origin, status: "releasing", chapters: None, volumes: None }
}

const fn done(mut b: Book, chapters: u32) -> Book {
    b.status = "finished";
    b.chapters = Some(chapters);
    b
}

fn medium_of(format: &str) -> Medium {
    if format == "novel" { Medium::Novel } else { Medium::Manga }
}

fn art(n: u32) -> Option<String> {
    crate::fixtures::ART_BASE.get().map(|base| format!("{base}cover/{n}.jpg"))
}

fn tile(b: &Book) -> WorkTile {
    let medium = medium_of(b.format);
    let mut facts = vec![kind_words(Some(b.format), medium, Some(b.origin))];
    facts.extend(status_words(b.status));
    if b.status == "finished" || medium == Medium::Novel {
        facts.extend(count_words(medium, b.chapters, b.volumes));
    }
    WorkTile {
        work: WorkId(format!("a:{}", b.art)),
        title: b.title.into(),
        medium,
        cover: art(b.art),
        format_label: Some(b.format.into()),
        progress: None,
        next: None,
        new_chapter: false,
        adult: false,
        facts,
        demo: false,
        not_in_sources: false,
    }
}

/// A source's hit with no AniList entry: its own cover, or none, and its count.
fn hit(id: &str, title: &str, art_n: Option<u32>, medium: Medium, volumes: u32) -> WorkTile {
    WorkTile {
        work: WorkId(format!("s:{id}")),
        title: title.into(),
        medium,
        cover: art_n.and_then(art),
        format_label: None,
        progress: None,
        next: None,
        new_chapter: false,
        adult: false,
        facts: count_words(medium, None, Some(volumes)).into_iter().collect(),
        demo: false,
        not_in_sources: false,
    }
}

const TRENDING_MANGA: [Book; 9] = [
    book("Iron Kite Brigade", 950001, "manga", "JP"),
    book("Saltwind Diary", 950002, "manga", "JP"),
    book("Night Ferry Club", 950003, "manga", "JP"),
    book("The Heron Clerk", 950004, "manga", "JP"),
    done(book("Canal Lights", 950005, "manga", "JP"), 88),
    book("Tidewater Detective", 950006, "manga", "JP"),
    book("Yorugawa Courier", 950007, "manga", "JP"),
    book("Lantern Street Notes", 950008, "manga", "JP"),
    book("The Ninth Lighthouse", 950009, "manga", "JP"),
];

const TRENDING_WEBTOONS: [Book; 8] = [
    book("Kite Season", 950010, "webtoon", "KR"),
    book("The Quiet Forge", 950011, "webtoon", "KR"),
    book("Saltglass Tower", 950012, "webtoon", "KR"),
    Book { status: "hiatus", ..book("Inkwell Station", 950013, "webtoon", "KR") },
    book("Umbrella Choir", 950014, "webtoon", "KR"),
    done(book("Rooftop Gardeners", 950015, "webtoon", "KR"), 120),
    book("Nine-Storey Teahouse", 950016, "webtoon", "CN"),
    book("Paper Comet", 950017, "webtoon", "KR"),
];

const TRENDING_NOVELS: [Book; 8] = [
    book("A Letter for the Ember Post", 950018, "novel", "JP"),
    book("Three Flames of Kiln Row", 950019, "novel", "JP"),
    book("The Candle Archivist", 950020, "novel", "JP"),
    book("Moon over the Book Stalls", 950021, "novel", "JP"),
    book("Forge of Small Wishes", 950022, "novel", "JP"),
    book("Frost Lantern Academy", 950023, "novel", "JP"),
    book("The Dusk Hour Library", 950024, "novel", "JP"),
    book("Ashgrove Night School", 950025, "novel", "JP"),
];

const NEW_MANGA: [Book; 8] = [
    book("A Quiet Sea of Paper", 950026, "manga", "JP"),
    book("The Tea Merchant's Apprentice", 950027, "manga", "JP"),
    book("Winter Letters from Asahi", 950028, "manga", "JP"),
    book("The Sixth Bell of Otowa", 950029, "manga", "JP"),
    book("Ashgrove Night Scouts", 950030, "manga", "JP"),
    book("The Clockmaker's Daughter", 950031, "manga", "JP"),
    book("Harbour of Folded Cranes", 950032, "manga", "JP"),
    book("Snowfield Postmen", 950033, "manga", "JP"),
];

const MORE_FANTASY: [Book; 8] = [
    book("Small Dragon, Large Library", 950034, "novel", "JP"),
    book("Spellbound Post Road", 950035, "manga", "JP"),
    done(book("The Witch of Salt Bay", 950036, "manga", "JP"), 64),
    book("Golem Apprentice Diaries", 900301, "webtoon", "KR"),
    done(book("Hollow Grove Wardens", 900302, "novel", "JP"), 12),
    book("Second-Hand Mage", 900303, "manga", "JP"),
    book("Tower of Ninety Stairs", 900304, "webtoon", "KR"),
    book("The Spare Familiar", 900305, "manga", "JP"),
];

const FINISHED_MANGA: [Book; 8] = [
    done(book("Hearthstone Detectives", 900306, "manga", "JP"), 112),
    done(book("Summer Relay Diaries", 900307, "manga", "JP"), 64),
    done(book("An Oboe for the Choir", 900308, "manga", "JP"), 38),
    done(book("Paper Kite Academy", 900309, "manga", "JP"), 150),
    done(book("Haru's Umbrella Repair", 900310, "manga", "JP"), 27),
    done(book("Last Train to Hoshimura", 900311, "manga", "JP"), 9),
    done(book("Atlas of Small Islands", 900312, "manga", "JP"), 76),
    done(book("The Lighthouse Cook", 900313, "manga", "JP"), 41),
];

const BECAUSE: [Book; 7] = [
    book("The Ferryman's Almanac", 950005, "manga", "JP"),
    book("Lamps along the Canal", 950012, "manga", "JP"),
    done(book("Harbour Ghost Stories", 950021, "manga", "JP"), 33),
    book("Riverlight Couriers", 950027, "webtoon", "KR"),
    book("Tide Clock Mysteries", 950031, "manga", "JP"),
    book("Lanterns for the Drowned Town", 950003, "novel", "JP"),
    book("Night Boat Postmaster", 950016, "manga", "JP"),
];

fn row(
    id: &str,
    kind: ReadRowKind,
    format: Option<ReadRowFormat>,
    title: Msg,
    side: Option<Msg>,
    books: &[Book],
) -> ReadHomeRow {
    let view_all = (kind != ReadRowKind::Because).then(|| ReadSearchQuery {
        formats: format.map(ReadRowFormat::search_formats).unwrap_or_default(),
        sort: Some("trending".into()),
        ..ReadSearchQuery::default()
    });
    ReadHomeRow {
        id: id.into(),
        kind,
        format,
        title,
        side,
        tiles: books.iter().map(tile).collect(),
        view_all,
        pending: false,
    }
}

fn reading(msg: ReadingMsg) -> Msg {
    Msg::Reading(msg)
}

fn first_run_rows() -> Vec<ReadHomeRow> {
    use ReadRowFormat::{Manga, Novels, Webtoons};
    vec![
        row(
            "trending-manga",
            ReadRowKind::Trending,
            Some(Manga),
            reading(ReadingMsg::ReadHomeTrendingManga),
            None,
            &TRENDING_MANGA,
        ),
        row(
            "trending-webtoons",
            ReadRowKind::Trending,
            Some(Webtoons),
            reading(ReadingMsg::ReadHomeTrendingWebtoons),
            None,
            &TRENDING_WEBTOONS,
        ),
        row(
            "trending-novels",
            ReadRowKind::Trending,
            Some(Novels),
            reading(ReadingMsg::ReadHomeTrendingNovels),
            None,
            &TRENDING_NOVELS,
        ),
        row(
            "new-manga",
            ReadRowKind::New,
            Some(Manga),
            reading(ReadingMsg::ReadHomeNewManga),
            Some(reading(ReadingMsg::ReadHomeNewSide)),
            &NEW_MANGA,
        ),
    ]
}

fn spotlight_of(
    b: &Book,
    case: ReadSpotlightCase,
    label: Msg,
    line: Option<Msg>,
) -> ReadSpotlight {
    ReadSpotlight {
        case,
        label,
        facts: tile(b).facts,
        synopsis: Some(
            "A courier on the night canals carries letters no one else will take, and starts \
             reading them."
                .into(),
        ),
        line,
        action: None,
        planned: false,
        banner: art(b.art),
    }
}

fn first_run() -> ReadingHomeView {
    let top = &TRENDING_MANGA[6];
    ReadingHomeView {
        spotlight: Some(tile(top)),
        spot: Some(spotlight_of(
            top,
            ReadSpotlightCase::Popular,
            reading(ReadingMsg::ReadingSpotlightPopular),
            None,
        )),
        rows: first_run_rows(),
        end: Some(ReadHomeEnd::Everything),
        ..ReadingHomeView::default()
    }
}

fn with_history() -> ReadingHomeView {
    use ReadRowFormat::{Manga, Novels};
    let mut continuing: Vec<WorkTile> =
        [&TRENDING_MANGA[1], &TRENDING_MANGA[3], &TRENDING_NOVELS[2]]
            .into_iter()
            .map(tile)
            .collect();
    for (n, t) in continuing.iter_mut().enumerate() {
        t.progress = Some([0.35, 0.7, 0.15][n]);
        t.next = Some(reading(ReadingMsg::ReadingChapterPage {
            chapter: ["12", "31", "4"][n].into(),
            page: [9, 3, 1][n],
        }));
    }
    continuing[0].new_chapter = true;
    let up_next: Vec<WorkTile> = [&TRENDING_WEBTOONS[1], &TRENDING_NOVELS[5], &NEW_MANGA[2]]
        .into_iter()
        .map(tile)
        .collect();
    let mut trending = row(
        "trending-manga",
        ReadRowKind::Trending,
        Some(Manga),
        reading(ReadingMsg::ReadHomeTrendingManga),
        None,
        &TRENDING_MANGA[4..],
    );
    // A series opened since Home drew, which none of the person's sources has.
    trending.tiles[1].not_in_sources = true;
    let rows = vec![
        trending,
        row(
            "trending-novels",
            ReadRowKind::Trending,
            Some(Novels),
            reading(ReadingMsg::ReadHomeTrendingNovels),
            None,
            &TRENDING_NOVELS[..5],
        ),
        row(
            "because",
            ReadRowKind::Because,
            None,
            Msg::HomeBecause { title: "The Lantern Ferry".into() },
            None,
            &BECAUSE,
        ),
        row(
            "new-manga",
            ReadRowKind::New,
            Some(Manga),
            reading(ReadingMsg::ReadHomeNewManga),
            Some(reading(ReadingMsg::ReadHomeNewSide)),
            &NEW_MANGA,
        ),
        row(
            "genre",
            ReadRowKind::MoreGenre,
            None,
            Msg::HomeMoreGenre { genre: "Fantasy".into() },
            None,
            &MORE_FANTASY,
        ),
        row(
            "finished-manga",
            ReadRowKind::Finished,
            Some(Manga),
            reading(ReadingMsg::ReadHomeFinishedManga),
            Some(reading(ReadingMsg::ReadHomeFinishedSide)),
            &FINISHED_MANGA,
        ),
    ];
    let first = &TRENDING_MANGA[1];
    ReadingHomeView {
        spotlight: Some(continuing[0].clone()),
        spot: Some(spotlight_of(
            first,
            ReadSpotlightCase::NewChapter,
            reading(ReadingMsg::ReadingSpotlightNewChapter),
            Some(reading(ReadingMsg::ReadingOutToday { chapter: "13".into() })),
        )),
        continue_reading: continuing,
        up_next,
        rows,
        end: Some(ReadHomeEnd::Everything),
        ..ReadingHomeView::default()
    }
}

/// Every row planned and none landed yet: the placeholders under their headings.
fn loading() -> ReadingHomeView {
    let mut view = first_run();
    view.spotlight = None;
    view.spot = None;
    for row in &mut view.rows {
        row.tiles.clear();
        row.pending = true;
    }
    view.end = None;
    view
}

// Search.

const LANTERN: [Book; 4] = [
    Book { volumes: Some(6), ..book("Lantern Street Notes", 950008, "novel", "JP") },
    book("The Lantern Ferry", 950003, "manga", "JP"),
    book("Three Lanterns of Kiln Row", 950019, "webtoon", "KR"),
    done(book("Paper Lantern Club", 950023, "manga", "JP"), 42),
];

const NIGHT: [Book; 6] = [
    book("Night Ferry Club", 950003, "manga", "JP"),
    book("Ashgrove Night School", 950025, "novel", "JP"),
    book("Yorugawa Courier", 950007, "manga", "JP"),
    book("Night Shift at the Observatory", 950013, "webtoon", "KR"),
    book("Owls of the Night Market", 950021, "manga", "JP"),
    book("Nightbird Postal Service", 950029, "manga", "JP"),
];

fn group(
    id: &str,
    name: &str,
    state: ReadSourceState,
    tiles: Vec<WorkTile>,
) -> ReadSourceGroup {
    ReadSourceGroup { source_id: id.into(), source_name: name.into(), state, tiles, joined: 0 }
}

/// How far a shot's search has got.
#[derive(Clone, Copy, PartialEq)]
enum Moment {
    /// Northwind is still asking.
    Mid,
    /// Northwind gave no answer in 15 s.
    Failed,
}

fn home_library_hits() -> Vec<WorkTile> {
    vec![
        hit("hl-1", "Lantern Keeper's Logbook", Some(950031), Medium::Manga, 2),
        hit("hl-2", "Lanterns at Low Tide", Some(950017), Medium::Manga, 4),
    ]
}

fn comics_hits() -> Vec<WorkTile> {
    vec![
        hit("cf-1", "Lantern Alley Sketches", Some(950014), Medium::Manga, 1),
        hit("cf-2", "The Lantern Ferry: Side Stories", Some(950022), Medium::Manga, 3),
        hit("cf-3", "Kiln Row Lantern Festival", Some(950034), Medium::Manga, 1),
    ]
}

fn northwind_hits() -> Vec<WorkTile> {
    vec![
        hit("nw-1", "Notes from the Lantern Ferry", None, Medium::Novel, 3),
        hit("nw-2", "A Lantern for the Ferry Cat", Some(950035), Medium::Manga, 2),
        hit("nw-3", "The Lanternmaker's Rival", Some(950011), Medium::Manga, 5),
        hit("nw-4", "Lanterns in the Rain", Some(950026), Medium::Manga, 1),
        hit("nw-5", "Lantern Watch", Some(950032), Medium::Manga, 2),
    ]
}

fn sources_part(query: &ReadSearchQuery, moment: Moment) -> ReadSourceSearch {
    let northwind = match moment {
        Moment::Mid => ReadSourceState::Asking,
        Moment::Failed => ReadSourceState::TimedOut { seconds: 15 },
    };
    let mut home =
        group(HOME_LIBRARY, "Home library", ReadSourceState::Hits, home_library_hits());
    home.joined = 1;
    ReadSourceSearch {
        query: query.clone(),
        anilist: ReadAnilistState::Answered,
        groups: vec![
            home,
            group(COMICS_FOLDER, "Comics folder", ReadSourceState::Hits, comics_hits()),
            group(NORTHWIND, "Northwind", northwind, Vec::new()),
            group(DEMO, "Demo", ReadSourceState::Nothing, Vec::new()),
        ],
        joins: vec![ReadSourceJoin {
            work: WorkId(format!("a:{}", LANTERN[0].art)),
            source_ids: vec![HOME_LIBRARY.into()],
            source_names: vec!["Home library".into()],
        }],
        done: moment == Moment::Failed,
    }
}

/// Only the sources asked: AniList is skipped and the volumes it joined show in their row.
fn sources_only_part(query: &ReadSearchQuery) -> ReadSourceSearch {
    let mut home = home_library_hits();
    home.insert(0, hit("hl-0", "Lantern Street Notes", Some(950008), Medium::Novel, 6));
    ReadSourceSearch {
        query: query.clone(),
        anilist: ReadAnilistState::Skipped,
        groups: vec![
            group(HOME_LIBRARY, "Home library", ReadSourceState::Hits, home),
            group(COMICS_FOLDER, "Comics folder", ReadSourceState::Hits, comics_hits()),
            group(NORTHWIND, "Northwind", ReadSourceState::Hits, northwind_hits()),
            group(DEMO, "Demo", ReadSourceState::Nothing, Vec::new()),
        ],
        joins: Vec::new(),
        done: true,
    }
}

fn nothing_part(query: &ReadSearchQuery) -> ReadSourceSearch {
    ReadSourceSearch {
        query: query.clone(),
        anilist: ReadAnilistState::Answered,
        groups: [
            (HOME_LIBRARY, "Home library"),
            (COMICS_FOLDER, "Comics folder"),
            (NORTHWIND, "Northwind"),
            (DEMO, "Demo"),
        ]
        .into_iter()
        .map(|(id, name)| group(id, name, ReadSourceState::Nothing, Vec::new()))
        .collect(),
        joins: Vec::new(),
        done: true,
    }
}

/// The long list: thirty of AniList's results on its first page, one source with hits.
fn night_part(query: &ReadSearchQuery) -> ReadSourceSearch {
    ReadSourceSearch {
        query: query.clone(),
        anilist: ReadAnilistState::Answered,
        groups: vec![
            group(
                HOME_LIBRARY,
                "Home library",
                ReadSourceState::Hits,
                vec![
                    hit("hl-n1", "Night Market Sketches", Some(950036), Medium::Manga, 1),
                    hit("hl-n2", "Notes from the Night Ferry", None, Medium::Novel, 3),
                    hit(
                        "hl-n3",
                        "The Night Porter of Hoshimura",
                        Some(950013),
                        Medium::Manga,
                        12,
                    ),
                ],
            ),
            group(COMICS_FOLDER, "Comics folder", ReadSourceState::Nothing, Vec::new()),
            group(NORTHWIND, "Northwind", ReadSourceState::Nothing, Vec::new()),
            group(DEMO, "Demo", ReadSourceState::Nothing, Vec::new()),
        ],
        joins: Vec::new(),
        done: true,
    }
}

/// AniList's page of a search. `more` is the whole count when more pages follow, as AniList gives
/// it with the first page.
fn search_event(
    query: &ReadSearchQuery,
    page: u32,
    tiles: Vec<WorkTile>,
    more: Option<u32>,
) -> Event {
    let total = more.or_else(|| u32::try_from(tiles.len()).ok());
    Event::Reading(ReadingEvent::Search {
        query: query.clone(),
        page,
        tiles,
        has_more: more.is_some(),
        other_scope: None,
        total,
    })
}

fn source_event(part: ReadSourceSearch) -> Event {
    Event::Reading(ReadingEvent::SourceSearch(Box::new(part)))
}

/// Thirty results for the long list, the six night titles first and the rest from the shelves.
fn thirty() -> Vec<WorkTile> {
    NIGHT
        .iter()
        .chain(TRENDING_MANGA.iter())
        .chain(TRENDING_WEBTOONS.iter())
        .chain(NEW_MANGA.iter())
        .take(30)
        .map(tile)
        .collect()
}

/// What each shot answers reading's commands with.
fn answer_for(shot: &str) -> Option<ReadAnswer> {
    let home = |view: fn() -> ReadingHomeView| -> ReadAnswer {
        Rc::new(move |command| match command {
            ReadingCommand::LoadHome { .. } => {
                Some(vec![(0, Event::Reading(ReadingEvent::Home(Box::new(view()))))])
            }
            _ => None,
        })
    };
    Some(match shot {
        "read-disc-home-first" | "read-disc-home-first-end" => home(first_run),
        "read-disc-home-history" | "read-disc-home-history-end" => home(with_history),
        "read-disc-home-loading" => home(loading),
        "read-disc-search-mid" | "read-disc-search-mid-end" | "read-disc-search-failed" => {
            let failed = shot == "read-disc-search-failed";
            Rc::new(move |command| match command {
                ReadingCommand::Search { query } if !query.text.is_empty() => {
                    let mut asking = sources_part(query, Moment::Mid);
                    asking.anilist = ReadAnilistState::Asking;
                    for g in &mut asking.groups {
                        g.state = ReadSourceState::Asking;
                        g.tiles.clear();
                    }
                    asking.joins.clear();
                    let tiles: Vec<WorkTile> = LANTERN.iter().map(tile).collect();
                    let mut out = vec![
                        (0, source_event(asking)),
                        (250, search_event(query, 0, tiles, None)),
                        (300, source_event(sources_part(query, Moment::Mid))),
                    ];
                    if failed {
                        out.push((900, source_event(sources_part(query, Moment::Failed))));
                    }
                    Some(out)
                }
                ReadingCommand::Search { .. } => Some(Vec::new()),
                ReadingCommand::RetrySourceSearch { query, .. } => {
                    Some(vec![(0, source_event(sources_part(query, Moment::Mid)))])
                }
                _ => None,
            })
        }
        "read-disc-search-mine" | "read-disc-search-mine-end" => {
            Rc::new(|command| match command {
                ReadingCommand::Search { query } if query.sources_only => {
                    Some(vec![(200, source_event(sources_only_part(query)))])
                }
                ReadingCommand::Search { query } if !query.text.is_empty() => Some(vec![
                    (100, search_event(query, 0, LANTERN.iter().map(tile).collect(), None)),
                    (150, source_event(sources_part(query, Moment::Failed))),
                ]),
                ReadingCommand::Search { .. } => Some(Vec::new()),
                _ => None,
            })
        }
        "read-disc-search-nothing" => Rc::new(|command| match command {
            ReadingCommand::Search { query } if !query.text.is_empty() => Some(vec![
                (100, search_event(query, 0, Vec::new(), None)),
                (200, source_event(nothing_part(query))),
            ]),
            ReadingCommand::Search { .. } => Some(Vec::new()),
            _ => None,
        }),
        "read-disc-search-end" => Rc::new(|command| match command {
            ReadingCommand::Search { query } if !query.text.is_empty() => Some(vec![
                (100, search_event(query, 0, thirty(), Some(212))),
                (200, source_event(night_part(query))),
            ]),
            ReadingCommand::Search { .. } => Some(Vec::new()),
            _ => None,
        }),
        _ => return None,
    })
}

/// Type `query` into the search field on screen once the page is in, and with `mine`, turn on
/// Your sources only a moment after.
fn type_query(shell: &Rc<Shell>, query: &'static str, mine: bool) {
    let shell = Rc::clone(shell);
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
        if let Some(entry) = find::<gtk::SearchEntry>(shell.ctx.window.upcast_ref(), "") {
            entry.set_text(query);
        }
        if mine {
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(1200),
                move || {
                    if let Some(toggle) = find::<gtk::ToggleButton>(
                        shell.ctx.window.upcast_ref(),
                        "read-sources-only",
                    ) {
                        toggle.set_active(true);
                    }
                },
            );
        }
    });
}

/// The first mapped `T` under `widget`, with `class` when one is named.
fn find<T: IsA<gtk::Widget>>(widget: &gtk::Widget, class: &str) -> Option<T> {
    if widget.is_mapped()
        && (class.is_empty() || widget.has_css_class(class))
        && let Some(found) = widget.downcast_ref::<T>()
    {
        return Some(found.clone());
    }
    let mut child = widget.first_child();
    while let Some(next) = child {
        if let Some(found) = find(&next, class) {
            return Some(found);
        }
        child = next.next_sibling();
    }
    None
}

/// Scroll every page's scroll to its end, `ms` after the stage.
fn scroll_to_end_later(shell: &Rc<Shell>, ms: u64) {
    fn to_end(widget: &gtk::Widget) {
        if let Some(scrolled) = widget.downcast_ref::<gtk::ScrolledWindow>()
            && scrolled.is_mapped()
        {
            let adjustment = scrolled.vadjustment();
            adjustment.set_value(adjustment.upper() - adjustment.page_size());
        }
        let mut child = widget.first_child();
        while let Some(c) = child {
            to_end(&c);
            child = c.next_sibling();
        }
    }
    let window = shell.ctx.window.clone();
    gtk::glib::timeout_add_local_once(std::time::Duration::from_millis(ms), move || {
        to_end(window.upcast_ref());
    });
}

/// Put the window in the state `shot` shows, and say how long to let it settle, when it is one.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    let answer = answer_for(shot)?;
    let hub = Rc::clone(&shell.ctx.hub);
    *hub.read_answer.borrow_mut() = Some(Rc::clone(&answer));
    if shot.starts_with("read-disc-home") {
        let events: Vec<Event> = answer(&ReadingCommand::LoadHome { refresh: false })
            .unwrap_or_default()
            .into_iter()
            .map(|(_, e)| e)
            .collect();
        if shot == "read-disc-home-loading" {
            // The engine's own rows leave first, so every row starts out on its way.
            let bare = ReadingHomeView { rows: Vec::new(), ..loading() };
            hub.show_instead(vec![Event::Reading(ReadingEvent::Home(Box::new(bare)))]);
            let later = Rc::clone(&hub);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(1200),
                move || {
                    later.show_instead(events);
                },
            );
        } else {
            hub.show_instead(events);
        }
        shell.go(Dest::Read);
        if shot.ends_with("-end") {
            scroll_to_end_later(shell, 2500);
        }
        return Some(if shot.ends_with("-end") { 4.0 } else { 3.5 });
    }
    shell.go(Dest::ReadSearch);
    if shot.ends_with("-end") || shot == "read-disc-search-failed" {
        scroll_to_end_later(
            shell,
            if shot.starts_with("read-disc-search-mine") { 4200 } else { 3500 },
        );
    }
    Some(match shot {
        "read-disc-search-mine" | "read-disc-search-mine-end" => {
            type_query(shell, "lantern", true);
            5.0
        }
        "read-disc-search-nothing" => {
            type_query(shell, "lantren", false);
            4.0
        }
        "read-disc-search-end" => {
            type_query(shell, "night", false);
            4.5
        }
        _ => {
            type_query(shell, "lantern", false);
            4.5
        }
    })
}

/// Give reading back to the engine after a discovery shot.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !NAMES.contains(&shot) {
        return;
    }
    let hub = Rc::clone(&shell.ctx.hub);
    hub.read_answer.replace(None);
    hub.release(&["read-home", "read-search", "read-source-search"]);
    if let Some(entry) = find::<gtk::SearchEntry>(shell.ctx.window.upcast_ref(), "") {
        entry.set_text("");
    }
    crate::reading::send(&hub, ReadingCommand::LoadHome { refresh: false });
    shell.go(Dest::Read);
}

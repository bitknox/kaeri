//! How many AniList requests each journey costs, against the fake, in the world the Fixtures
//! apps show. Each journey has an upper bound ([`BOUNDS`]): a change that costs more fails here,
//! and one that costs less lowers the bound. Run with `--nocapture` to read the table, and with
//! `KAERI_LAUNCH_TIMELINE` set as well for when each cold-launch request went.

use std::collections::BTreeMap;

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    catalog::LibrarySegment,
    command::{Command, LicensedCommand},
    event::Event,
    home::SectionKind,
};
use kaeri_store::Store;
use kaeri_testkit::{
    fixtures,
    home_world::{HomeClock, HomeWorld},
};

use super::WITH_ANILIST;
use crate::harness::{Harness, Options};

/// A schedule as busy as AniList's: 130 invented shows airing weekly, from thirteen weeks back
/// to four ahead, about 130 broadcasts a week, at the fixtures' clock.
fn busy_schedule(h: &Harness) {
    const DAY: i64 = 86_400;
    let server = h.server();
    let titles: Vec<_> = (0..130u32)
        .map(|n| fixtures::invented(960_000 + n, "RELEASING", 24, (2030, 1, 1), &[], &[]))
        .collect();
    server.add_titles(titles);
    let mut rows = Vec::new();
    for n in 0..130u32 {
        let slot = i64::from(n) * 7 * DAY / 130;
        for week in -13..4i64 {
            let at = fixtures::FIXTURE_NOW + week * 7 * DAY + slot;
            rows.push((AnilistId::new(960_000 + n), (week + 14) as u32, at));
        }
    }
    server.add_broadcasts(rows);
}

fn install(h: &Harness, world: &HomeWorld) {
    busy_schedule(h);
    let server = h.server();
    server.add_titles(fixtures::long_franchise());
    server.add_titles(fixtures::franchise());
    let (season, rest): (Vec<_>, Vec<_>) = world
        .titles
        .iter()
        .cloned()
        .partition(|t| world.this_season.iter().any(|id| t["id"] == id.get()));
    server.add_season(season);
    server.add_catalogue(rest);
    server.add_broadcasts(world.broadcasts.iter().copied());
    server.add_entries(world.entries.iter().cloned());
}

async fn launch(
    now: i64,
    store: Option<Store>,
    world: &HomeWorld,
    digest: bool,
    ahead: i64,
) -> Harness {
    let clock = kaeri_engine::clock::Clock::default();
    clock.advance(ahead);
    let h = Harness::with(Options {
        now,
        config: Some(WITH_ANILIST.to_owned()),
        signed_in: vec!["anilist"],
        store,
        digest,
        clock: Some(clock),
        ..Options::default()
    })
    .await;
    install(&h, world);
    h
}

/// The most requests a journey may cost, by the start of its label.
const BOUNDS: &[(&str, usize)] = &[
    ("cold launch: launch pull", 2),
    ("cold launch: LoadHome", 9),
    ("cold launch: WidgetSnapshot", 4),
    ("cold launch: Continue stills", 0),
    ("Home again within 30 min", 0),
    ("Home after the player closes", 5),
    ("Home again after 31 min", 8),
    ("open a title (5-season", 2),
    ("switch season", 1),
    ("open a title (12-season", 4),
    ("Library", 0),
    ("Calendar", 6),
    ("Search start", 1),
    ("hover 10 covers on the Mac", 1),
    ("hover 10 covers", 1),
    ("sync tick", 0),
    ("warm launch: airing digest", 2),
    ("warm launch: launch pull", 1),
    ("warm launch: LoadHome", 1),
    ("warm launch: WidgetSnapshot", 3),
    ("warm launch: Continue stills", 0),
    ("warm launch: Calendar", 2),
    ("warm launch: Search start", 0),
    ("warm launch: open the same title", 2),
    ("new-episode check", 0),
    ("warm: Home after its half hour", 8),
];

#[derive(Default)]
struct Tally {
    rows: Vec<(String, usize, BTreeMap<String, usize>)>,
    seen: usize,
}

impl Tally {
    fn take(&mut self, h: &Harness, label: &str) {
        let all: Vec<_> =
            h.server().requests().into_iter().filter(|r| r.path == "/graphql").collect();
        let new = &all[self.seen.min(all.len())..];
        let mut by: BTreeMap<String, usize> = BTreeMap::new();
        for r in new {
            *by.entry(format!("{:?}", r.operation)).or_default() += 1;
        }
        self.rows.push((label.to_owned(), new.len(), by));
        self.seen = all.len();
    }

    fn reset(&mut self) {
        self.seen = 0;
    }

    /// Every journey stays within its bound.
    fn check(&self) {
        for (label, n, _) in &self.rows {
            let bound = BOUNDS.iter().find(|(prefix, _)| label.starts_with(prefix));
            let Some((_, max)) = bound else { panic!("no bound for the journey {label:?}") };
            assert!(n <= max, "{label} took {n} AniList requests, the bound is {max}");
        }
    }

    fn print(&self) {
        println!("\n| journey | requests | by operation |\n|---|---:|---|");
        for (label, n, by) in &self.rows {
            let by: Vec<String> = by.iter().map(|(k, v)| format!("{k} {v}")).collect();
            println!("| {label} | {n} | {} |", by.join(", "));
        }
    }
}

async fn run(h: &mut Harness, commands: Vec<Command>) -> Vec<Event> {
    for c in commands {
        h.send(c);
    }
    h.settle().await
}

fn home_items(updates: &[Event]) -> Vec<(AnilistId, bool, SectionKind)> {
    let Some(home) = updates
        .iter()
        .filter_map(|u| match u {
            Event::Home(home) => Some(home.as_ref().clone()),
            _ => None,
        })
        .next_back()
    else {
        return Vec::new();
    };
    home.sections
        .iter()
        .flat_map(|s| {
            s.items.iter().map(move |i| (i.entry.id, i.entry.banner_url.is_some(), s.kind))
        })
        .collect()
}

fn open_title(id: AnilistId) -> Vec<Command> {
    vec![
        Command::LoadDetail(id),
        Command::Licensed(LicensedCommand::LoadAvailability { id, plays_here: true }),
        Command::LoadEpisodes(id),
        Command::LoadDub(id),
    ]
}

#[tokio::test]
async fn anilist_budget_per_journey() {
    let now = kaeri_store::now();
    let world = HomeWorld::at(HomeClock::in_zone(now, &jiff::tz::TimeZone::UTC));
    let mut tally = Tally::default();

    let mut h = launch(now, None, &world, false, 0).await;
    world.seed_history(h.store(), now);
    let store = h.store().clone();

    // Cold launch: the launch pull, Home as the Mac's Home tab asks for it, then what the
    // Continue and sync answers set off (the widgets' record) and the stills of Continue cards
    // without a banner.
    run(&mut h, vec![Command::SyncNow]).await;
    tally.take(&h, "cold launch: launch pull (SyncNow)");
    let updates =
        run(&mut h, vec![Command::LoadHome { refresh: false }, Command::LoadContinue]).await;
    tally.take(&h, "cold launch: LoadHome + LoadContinue");
    for r in h.server().requests().iter().filter(|r| r.path == "/graphql") {
        let ids = r.variables.get("ids").map(|v| v.to_string()).unwrap_or_default();
        println!("  {:?} ids={ids}", r.operation);
    }
    let items = home_items(&updates);
    run(&mut h, vec![Command::WidgetSnapshot, Command::UpcomingNotices]).await;
    tally.take(&h, "cold launch: WidgetSnapshot + UpcomingNotices");
    let stills: Vec<Command> = items
        .iter()
        .filter(|(_, banner, kind)| !banner && *kind == SectionKind::Continue)
        .map(|(id, ..)| Command::LoadEpisodes(*id))
        .collect();
    let n = stills.len();
    run(&mut h, stills).await;
    tally.take(&h, &format!("cold launch: Continue stills ({n} cards without a banner)"));

    // Home again inside half an hour, and after the player closes (a forced refresh).
    run(&mut h, vec![Command::LoadHome { refresh: false }, Command::LoadContinue]).await;
    tally.take(&h, "Home again within 30 min");
    // The player closes after an episode of the last title in Continue: Continue and Up next
    // are made again from the history, and no part of Home is forced.
    let moved = items
        .iter()
        .rfind(|(_, _, kind)| *kind == SectionKind::Continue)
        .map(|(id, ..)| *id)
        .expect("Continue holds a title");
    kaeri_testkit::seed::watched(&store, moved, 1, now + 60);
    let updates =
        run(&mut h, vec![Command::LoadHome { refresh: false }, Command::LoadContinue]).await;
    tally.take(&h, "Home after the player closes (Continue and Up next only)");
    let after = home_items(&updates);
    let first = after.iter().find(|(_, _, kind)| *kind == SectionKind::Continue);
    assert_eq!(first.map(|(id, ..)| *id), Some(moved), "Continue moves on after an episode");
    h.advance_engine_clock(31 * 60);
    run(&mut h, vec![Command::LoadHome { refresh: false }, Command::LoadContinue]).await;
    tally.take(&h, "Home again after 31 min (engine clock moved)");

    // A title page, then two more seasons of the same franchise.
    let [_, _, third, fourth, first_last] = fixtures::FRANCHISE_SEASONS;
    run(&mut h, open_title(third)).await;
    tally.take(&h, "open a title (5-season franchise)");
    run(&mut h, open_title(fourth)).await;
    tally.take(&h, "switch season (next)");
    run(&mut h, open_title(first_last)).await;
    tally.take(&h, "switch season (last)");
    let long = fixtures::LONG_FRANCHISE_SEASONS[5];
    run(&mut h, open_title(long)).await;
    tally.take(&h, "open a title (12-season franchise)");
    run(&mut h, open_title(fixtures::LONG_FRANCHISE_SEASONS[6])).await;
    tally.take(&h, "switch season (12-season franchise)");

    // Library: every segment, as the screen asks on appearing.
    run(&mut h, LibrarySegment::ALL.iter().map(|s| Command::LoadLibrary(*s)).collect()).await;
    tally.take(&h, "Library (5 segments)");

    run(&mut h, vec![Command::LoadCalendar]).await;
    tally.take(&h, "Calendar");

    run(&mut h, vec![Command::LoadBrowse, Command::LoadSearchFacets]).await;
    tally.take(&h, "Search start (rails + facets)");

    // Hover ten covers: the hover card's data comes from what the lists brought, in one command
    // for the covers on screen. The page's Detail waits for the page to open.
    let hovered: Vec<AnilistId> = world
        .titles
        .iter()
        .filter_map(|t| t["id"].as_u64().map(|n| AnilistId::new(n as u32)))
        .filter(|id| *id != third && *id != fourth)
        .take(10)
        .collect();
    let updates = run(&mut h, vec![Command::LoadCards { ids: hovered.clone() }]).await;
    let carded: usize = updates
        .iter()
        .map(|u| if let Event::Cards(entries) = u { entries.len() } else { 0 })
        .sum();
    assert!(carded >= 1, "the cards of the covers on screen are answered");
    tally.take(&h, "hover 10 covers");
    // The Mac's hover card also reads the episode table for its Play line and stills. The
    // screen asks what the tables need with its covers' cards, in one request, and the hovers'
    // tables are then built from that: counted on its own so the card's own cost above stays
    // honest.
    run(&mut h, vec![Command::PrepareCards { ids: hovered.clone() }]).await;
    let updates =
        run(&mut h, hovered.iter().map(|id| Command::LoadStills(*id)).collect()).await;
    let tabled = hovered
        .iter()
        .filter(|id| {
            updates
                .iter()
                .any(|u| matches!(u, Event::Episodes { id: of, failed: None, .. } if of == *id))
        })
        .count();
    assert_eq!(tabled, hovered.len(), "every hovered title's table is built");
    tally.take(
        &h,
        "hover 10 covers on the Mac: cards and tables for the screen, then the hovers",
    );

    // A sync tick as the 15-minute loop and Sync now run it, and the widgets' record a sync
    // state sets off. Within five minutes of the last, so the schedule is still kept.
    run(&mut h, vec![Command::SyncNow, Command::WidgetSnapshot]).await;
    tally.take(&h, "sync tick + widgets (schedule still kept)");

    let _ = h.quit().await;

    // Warm launch: a new engine on the same database, last launched two days ago, with the
    // launch's airing digest on.
    store.set_meta_i64("airing_digest_seen", now - 2 * 86_400).unwrap();
    let mut h = launch(now, Some(store.clone()), &world, true, 31 * 60).await;
    tally.reset();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while h
        .server()
        .requests()
        .iter()
        .all(|r| r.operation != kaeri_testkit::Operation::Schedule)
        && std::time::Instant::now() < deadline
    {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    tally.take(&h, "warm launch: airing digest (last launch 2 days ago)");
    run(&mut h, vec![Command::SyncNow]).await;
    tally.take(&h, "warm launch: launch pull");
    let updates =
        run(&mut h, vec![Command::LoadHome { refresh: false }, Command::LoadContinue]).await;
    tally.take(&h, "warm launch: LoadHome + LoadContinue");
    let items = home_items(&updates);
    run(&mut h, vec![Command::WidgetSnapshot, Command::UpcomingNotices]).await;
    tally.take(&h, "warm launch: WidgetSnapshot + UpcomingNotices (schedule cold)");
    let stills: Vec<Command> = items
        .iter()
        .filter(|(_, banner, kind)| !banner && *kind == SectionKind::Continue)
        .map(|(id, ..)| Command::LoadEpisodes(*id))
        .collect();
    let n = stills.len();
    run(&mut h, stills).await;
    tally.take(&h, &format!("warm launch: Continue stills ({n})"));
    run(&mut h, vec![Command::LoadCalendar]).await;
    tally.take(&h, "warm launch: Calendar");
    run(&mut h, vec![Command::LoadBrowse, Command::LoadSearchFacets]).await;
    tally.take(&h, "warm launch: Search start");
    run(&mut h, open_title(third)).await;
    tally.take(&h, "warm launch: open the same title");
    run(&mut h, vec![Command::CheckNewEpisodes]).await;
    tally.take(&h, "new-episode check");
    h.advance_engine_clock(31 * 60);
    run(&mut h, vec![Command::LoadHome { refresh: false }, Command::LoadContinue]).await;
    tally.take(&h, "warm: Home after its half hour");
    println!("continue rows: {}", store.continue_list(15).unwrap_or_default().len());

    tally.print();
    tally.check();
    let _ = h.quit().await;
}

#[tokio::test]
async fn hover_cards_for_unlisted_covers_cost_one_batched_lookup() {
    let now = kaeri_store::now();
    let world = HomeWorld::at(HomeClock::in_zone(now, &jiff::tz::TimeZone::UTC));
    let mut h = launch(now, None, &world, false, 0).await;
    let ids: Vec<AnilistId> = world
        .titles
        .iter()
        .filter_map(|t| t["id"].as_u64().map(|n| AnilistId::new(n as u32)))
        .take(6)
        .collect();
    let mut tally = Tally::default();
    tally.take(&h, "before");
    let updates = run(&mut h, vec![Command::LoadCards { ids: ids.clone() }]).await;
    tally.take(&h, "cards for six covers no list brought");
    let carded: usize = updates
        .iter()
        .map(|u| if let Event::Cards(entries) = u { entries.len() } else { 0 })
        .sum();
    assert_eq!(carded, ids.len(), "every card is answered");
    assert_eq!(
        tally.rows[1].2.get("Many"),
        Some(&1),
        "by one batched lookup: {:?}",
        tally.rows[1].2
    );
    assert_eq!(tally.rows[1].2.get("Detail"), None, "and no page for any of them");
    let again = run(&mut h, vec![Command::LoadCards { ids }]).await;
    tally.take(&h, "the same again");
    assert!(again.iter().any(|u| matches!(u, Event::Cards(_))));
    assert_eq!(tally.rows[2].1, 0, "and the second time none");
}

/// The Mac's cold launch, as the app makes it, against an AniList that answers 30 a minute and
/// says so in its headers: the launch pull, Home and Continue as the Home tab asks, the week's
/// notices and the widgets' record, then once Home has drawn, the spotlight's page and the stills
/// of the Continue cards with no banner. Every title of the world without a banner on every
/// other id, as many real ones are, so the stills are counted. Run with `--nocapture` for the
/// table; [`COLD_LAUNCH`] bounds it.
/// What a cold launch on the Mac set off, against the fake: see [`cold_launch_on_the_mac`].
struct Launch {
    asked: Vec<kaeri_testkit::Request>,
    first_ten: usize,
    refused: usize,
    /// Alerts raised, and Home sends that said a part failed.
    alerts: usize,
    failed: usize,
    stills: usize,
    /// The kinds of the last Home's sections, and whether it had a spotlight.
    shelves: Vec<SectionKind>,
    spotlight: bool,
}

/// Launch the Mac app cold, signed in, with AniList answering `limit`: the launch pull, Home
/// and Continue as the Home tab asks for them, the notices and the widgets' record, then after
/// the first paint what Home's spotlight and its visible Continue cards ask.
async fn cold_launch_on_the_mac(limit: impl FnOnce(&kaeri_testkit::FakeAniList)) -> Launch {
    let now = kaeri_store::now();
    let world = HomeWorld::at(HomeClock::in_zone(now, &jiff::tz::TimeZone::UTC));
    let mut h = launch(now, None, &world, true, 0).await;
    limit(h.server());
    // Eight more shows being watched, finished and without a banner, so the Continue row is as
    // full as a viewer's and its cards ask for stills.
    h.server().add_titles(
        (0..8u32)
            .map(|n| fixtures::invented(970_000 + n, "FINISHED", 12, (2029, 4, 1), &[], &[])),
    );
    h.server().add_entries((0..8u32).map(|n| {
        serde_json::json!({
            "id": 87_000 + n,
            "mediaId": 970_000 + n,
            "status": "CURRENT",
            "progress": 3,
            "score": 0,
            "updatedAt": now - i64::from(n + 1) * 3_600,
        })
    }));
    world.seed_history(h.store(), now);
    let started = std::time::Instant::now();

    let mut updates = run(
        &mut h,
        vec![
            Command::SyncNow,
            Command::LoadHome { refresh: false },
            Command::LoadContinue,
            Command::UpcomingNotices,
            Command::WidgetSnapshot,
        ],
    )
    .await;
    let spotlight = updates.iter().rev().find_map(|u| match u {
        Event::Home(home) => home.spotlight.as_ref().map(|s| s.entry.id),
        _ => None,
    });
    let mut after: Vec<Command> = spotlight
        .into_iter()
        .flat_map(|id| [Command::ProbeTitle { id }, Command::LoadDetail(id)])
        .collect();
    // The Continue cards a Mac window shows at once.
    after.extend(
        home_items(&updates)
            .into_iter()
            .filter(|(_, banner, kind)| !banner && *kind == SectionKind::Continue)
            .take(6)
            .map(|(id, ..)| Command::LoadStills(id)),
    );
    let stills = after.iter().filter(|c| matches!(c, Command::LoadStills(_))).count();
    updates.extend(run(&mut h, after).await);

    let asked: Vec<_> =
        h.server().requests().into_iter().filter(|r| r.path == "/graphql").collect();
    let last = updates.iter().rev().find_map(|u| match u {
        Event::Home(home) => Some(home.as_ref().clone()),
        _ => None,
    });
    let launch = Launch {
        first_ten: asked.iter().filter(|r| r.at.duration_since(started).as_secs() < 10).count(),
        refused: asked.iter().filter(|r| r.refused).count(),
        alerts: updates
            .iter()
            .filter(|u| {
                matches!(u, Event::Toast(n) if n.kind == kaeri_model::notice::NoticeKind::Alert)
            })
            .count(),
        failed: updates.iter().filter(|u| matches!(u, Event::Home(home) if home.failed)).count(),
        stills,
        shelves: last.as_ref().map(|l| l.sections.iter().map(|s| s.kind).collect()).unwrap_or_default(),
        spotlight: last.is_some_and(|l| l.spotlight.is_some()),
        asked,
    };
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for r in &launch.asked {
        *by.entry(format!("{:?}", r.operation)).or_default() += 1;
    }
    let by: Vec<String> = by.iter().map(|(k, v)| format!("{k} {v}")).collect();
    println!(
        "\ncold launch on the Mac: {} requests, {} in the first 10 s, {} refused (429), {} alerts, \
         {} failed Home sends, {} stills asked, over {:.1} s\n  {}",
        launch.asked.len(),
        launch.first_ten,
        launch.refused,
        launch.alerts,
        launch.failed,
        launch.stills,
        started.elapsed().as_secs_f64(),
        by.join(", ")
    );
    if std::env::var_os("KAERI_LAUNCH_TIMELINE").is_some() {
        for r in &launch.asked {
            let ms = r.at.duration_since(started).as_millis();
            println!(
                "  {ms:>6} ms {:?}{}",
                r.operation,
                if r.refused { " refused" } else { "" }
            );
        }
    }
    let _ = h.quit().await;
    launch
}

#[tokio::test]
async fn anilist_cold_launch_on_the_mac() {
    let launch = cold_launch_on_the_mac(|server| server.limit_per_minute(30)).await;
    assert_eq!(launch.refused, 0, "a cold launch stays inside AniList's 30 a minute");
    assert_eq!((launch.alerts, launch.failed), (0, 0), "nothing at launch alerts");
    assert_eq!(launch.stills, 6, "the Continue cards without a banner ask for their stills");
    assert!(
        launch.asked.len() <= COLD_LAUNCH,
        "the cold launch took {}, the bound is {COLD_LAUNCH}",
        launch.asked.len()
    );
}

/// A launch over a limit far tighter than AniList's, as a second launch inside the minute of
/// the first meets: each refusal is a wait as the server says and another go, so Home comes
/// whole and nothing alerts.
#[tokio::test]
async fn anilist_cold_launch_over_a_tight_limit_waits_and_says_nothing() {
    let roomy = cold_launch_on_the_mac(|server| server.limit_per_minute(30)).await;
    let tight = cold_launch_on_the_mac(|server| {
        server.limit_per(6, std::time::Duration::from_secs(2));
    })
    .await;
    assert!(tight.refused > 0, "the tight limit refused some");
    assert_eq!((tight.alerts, tight.failed), (0, 0), "a refusal is a wait, never an alert");
    assert_eq!(tight.shelves, roomy.shelves, "Home comes whole");
    assert!(tight.spotlight);
}

/// The most AniList requests the Mac's cold launch may make: [`anilist_cold_launch_on_the_mac`].
/// It makes 20 when Home's, Continue's, Up next's and the widgets' title lookups meet in one
/// batch window and up to 23 when they each miss the others' (four to seven `Many`), as the
/// tasks happen to be scheduled.
const COLD_LAUNCH: usize = 23;

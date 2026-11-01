//! Your media through the engine: folders, the scan, matching, Keep unmatched and removal.
//! Folders are temporary directories of empty files, and AniList is the testkit's fake.

use std::path::Path;

use kaeri_core::ids::AnilistId;
use kaeri_model::{
    catalog::{AvailableAt, ContentList, Entry},
    command::{Command, LicensedCommand},
    event::Event,
    local::{
        FolderScan, GroupState, LocalCommand, LocalEvent, LocalView, MatchProgress,
        NotMatchedReason,
    },
    search::{Available, SearchQuery, SearchSort},
};
use kaeri_testkit::fixtures::invented;

use crate::harness::Harness;

const KNOWN: AnilistId = AnilistId::new(960_001);

async fn harness() -> Harness {
    let h = Harness::new().await;
    h.server().add_catalogue(vec![invented(
        KNOWN.get(),
        "FINISHED",
        12,
        (2030, 4, 6),
        &[],
        &[],
    )]);
    h
}

fn touch(root: &Path, rel: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"").unwrap();
}

fn view(updates: &[Event]) -> LocalView {
    updates
        .iter()
        .rev()
        .find_map(|u| match u {
            Event::Local(LocalEvent::View(v)) => Some(v.clone()),
            _ => None,
        })
        .expect("the view")
}

async fn scanned(h: &mut Harness, command: LocalCommand) -> LocalView {
    h.send(Command::Local(command));
    let updates =
        h.settle_until(|e| matches!(e, Event::Local(LocalEvent::View(v)) if !v.scanning)).await;
    view(&updates)
}

async fn settled(h: &mut Harness, command: LocalCommand) -> LocalView {
    h.send(Command::Local(command));
    view(&h.settle().await)
}

fn media() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "Invented 960001 [Release] (1080p)/Invented 960001 - 02.mkv");
    touch(dir.path(), "Invented 960001 [Release] (1080p)/Invented 960001 - 01.mkv");
    touch(dir.path(), "Quiet Orchard Days - 03.mp4");
    dir
}

fn add(dir: &tempfile::TempDir) -> LocalCommand {
    LocalCommand::AddFolder { path: dir.path().to_string_lossy().into_owned() }
}

#[tokio::test]
async fn a_folder_is_scanned_and_its_series_matched_by_name() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    assert_eq!(v.folders.len(), 1);
    assert!(v.folders[0].available);
    assert_eq!(v.folders[0].files, 3);
    let known = v.groups.iter().find(|g| g.name == "Invented 960001").expect("the series");
    assert_eq!(known.state, GroupState::Matched);
    assert_eq!(known.anilist, Some(KNOWN));
    assert_eq!(known.files, 2);
    let unknown = v.groups.iter().find(|g| g.name == "Quiet Orchard Days").expect("loose file");
    assert_eq!(unknown.state, GroupState::NotMatched);
    assert_eq!(unknown.reason, Some(NotMatchedReason::NoResult));
}

#[tokio::test]
async fn keeping_unmatched_makes_a_title_of_its_own_that_a_rescan_leaves_alone() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let group = v.groups.iter().find(|g| g.state == GroupState::NotMatched).unwrap().id.clone();
    let v = settled(&mut h, LocalCommand::Keep { group: group.clone() }).await;
    let kept = v.groups.iter().find(|g| g.id == group).unwrap();
    assert_eq!(kept.state, GroupState::Kept);
    assert!(kept.anilist.unwrap().is_own(), "never an AniList id");
    let v = scanned(&mut h, LocalCommand::Scan).await;
    let kept = v.groups.iter().find(|g| g.id == group).unwrap();
    assert_eq!(kept.state, GroupState::Kept);
}

#[tokio::test]
async fn choosing_a_match_confirms_it() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let group = v.groups.iter().find(|g| g.state == GroupState::NotMatched).unwrap().id.clone();
    let v =
        settled(&mut h, LocalCommand::Choose { group: group.clone(), anilist: KNOWN }).await;
    let chosen = v.groups.iter().find(|g| g.id == group).unwrap();
    assert_eq!(chosen.state, GroupState::Matched);
    assert_eq!(chosen.anilist, Some(KNOWN));
}

#[tokio::test]
async fn fix_this_match_lists_titles_and_says_which_do_not_fit_the_files() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let group = v.groups.iter().find(|g| g.state == GroupState::NotMatched).unwrap().id.clone();
    let query = "Invented 960001".to_owned();
    h.send(Command::Local(LocalCommand::FindMatch {
        group: group.clone(),
        query: query.clone(),
    }));
    let updates =
        h.settle_until(|e| matches!(e, Event::Local(LocalEvent::Matches { .. }))).await;
    let Some(Event::Local(LocalEvent::Matches {
        group: asked,
        query: echoed,
        choices,
        offline,
    })) = updates.iter().rev().find(|e| matches!(e, Event::Local(LocalEvent::Matches { .. })))
    else {
        panic!("the matches");
    };
    assert_eq!((asked, echoed, *offline), (&group, &query, false));
    let found = choices.iter().find(|c| c.anilist == KNOWN).expect("the title searched for");
    assert_eq!(found.episodes, Some(12));
    assert!(!found.fits, "12 episodes against one file");
    assert!(found.confidence > 0);
}

/// The results of a browse kept to `available`.
async fn browse(h: &mut Harness, available: Option<Available>) -> Vec<Entry> {
    let query = SearchQuery { available, ..SearchQuery::sorted(SearchSort::Popularity) };
    h.send(Command::Search { query });
    match super::content(&h.settle().await) {
        ContentList::Entries(entries) => entries.clone(),
        ContentList::Empty => Vec::new(),
        other => panic!("not a list of titles: {other:?}"),
    }
}

#[tokio::test]
async fn available_keeps_to_your_own_sources_or_your_services_and_says_where() {
    const SERVICE: &str = "Lantern Stream";
    let carried = kaeri_testkit::fixtures::FINISHED;
    let dir = media();
    let mut h = harness().await;
    let asked = h.server().requests_for(kaeri_testkit::Operation::Search).len();
    assert!(
        browse(&mut h, Some(Available::PlaysHere)).await.is_empty(),
        "nothing of yours yet"
    );
    assert!(browse(&mut h, Some(Available::OnYourServices)).await.is_empty(), "none chosen");
    let now = h.server().requests_for(kaeri_testkit::Operation::Search).len();
    assert_eq!(now, asked, "neither asks AniList");

    scanned(&mut h, add(&dir)).await;
    let here = browse(&mut h, Some(Available::PlaysHere)).await;
    assert_eq!(here.iter().map(|e| e.id).collect::<Vec<_>>(), [KNOWN]);
    assert_eq!(here[0].available_at, Some(AvailableAt::InLibrary));

    h.send(Command::Licensed(LicensedCommand::SetServices { services: vec![SERVICE.into()] }));
    h.settle().await;
    let on = browse(&mut h, Some(Available::OnYourServices)).await;
    assert!(
        on.iter().any(|e| e.id == carried),
        "{:?}",
        on.iter().map(|e| e.id).collect::<Vec<_>>()
    );
    let line = Some(AvailableAt::OnService { service: SERVICE.into() });
    assert!(on.iter().all(|e| e.available_at == line), "every one ends On {SERVICE}");

    let all = browse(&mut h, None).await;
    let at = |id| all.iter().find(|e| e.id == id).and_then(|e| e.available_at.clone());
    assert_eq!(at(KNOWN), Some(AvailableAt::InLibrary));
    assert_eq!(at(carried), line);
}

#[tokio::test]
async fn a_folder_that_is_not_there_is_unavailable_and_keeps_what_was_found() {
    let dir = media();
    let mut h = harness().await;
    scanned(&mut h, add(&dir)).await;
    let path = dir.path().to_path_buf();
    std::fs::rename(&path, path.with_extension("away")).unwrap();
    let v = scanned(&mut h, LocalCommand::Scan).await;
    assert!(!v.folders[0].available);
    assert_eq!(v.groups.len(), 2, "its series are remembered while the drive is out");
    std::fs::rename(path.with_extension("away"), &path).unwrap();
}

#[tokio::test]
async fn removing_a_folder_forgets_it_and_leaves_its_files() {
    let dir = media();
    let mut h = harness().await;
    scanned(&mut h, add(&dir)).await;
    let v = settled(
        &mut h,
        LocalCommand::RemoveFolder { path: dir.path().to_string_lossy().into_owned() },
    )
    .await;
    assert!(v.folders.is_empty() && v.groups.is_empty());
    assert!(dir.path().join("Quiet Orchard Days - 03.mp4").exists());
}

#[tokio::test]
async fn the_row_counts_the_matching_and_then_sums_up_the_folder() {
    let dir = media();
    let mut h = harness().await;
    h.send(Command::Local(add(&dir)));
    let updates =
        h.settle_until(|e| matches!(e, Event::Local(LocalEvent::View(v)) if !v.scanning)).await;
    let lines: Vec<FolderScan> = updates
        .iter()
        .filter_map(|u| match u {
            Event::Local(LocalEvent::View(v)) => v.folders.first().and_then(|f| f.scan.clone()),
            _ => None,
        })
        .collect();
    assert!(lines.iter().any(|l| matches!(l, FolderScan::Scanning { .. })), "{lines:?}");
    assert!(
        lines.iter().any(|l| matches!(
            l,
            FolderScan::Matching { progress: MatchProgress { done: 1, total: 2, .. } }
        )),
        "{lines:?}"
    );
    let done = view(&updates);
    assert_eq!(done.folders[0].scan, None, "the line is the summary once done");
    assert_eq!((done.folders[0].series, done.folders[0].episodes), (1, 2));
    assert!(done.groups.iter().all(|g| g.folder == dir.path().to_string_lossy()));
}

#[tokio::test]
async fn a_file_with_no_number_waits_for_its_episode_and_keeps_it_after_a_rescan() {
    let dir = media();
    touch(dir.path(), "Invented 960001 [Release] (1080p)/extras/Making of.mkv");
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let [file] = &v.unnumbered[..] else { panic!("one file waits: {:?}", v.unnumbered) };
    assert_eq!((file.name.as_str(), file.place.as_str()), ("Making of.mkv", "extras"));
    let v = settled(
        &mut h,
        LocalCommand::SetEpisode {
            group: file.group.clone(),
            path: file.path.clone(),
            season: 0,
            episode: 1,
        },
    )
    .await;
    assert!(v.unnumbered.is_empty());
    let v = scanned(&mut h, LocalCommand::Scan).await;
    assert!(v.unnumbered.is_empty(), "the rescan kept what was set");
}

#[tokio::test]
async fn undo_brings_a_removed_folder_back_with_what_was_decided() {
    let dir = media();
    let mut h = harness().await;
    let v = scanned(&mut h, add(&dir)).await;
    let group = v.groups.iter().find(|g| g.state == GroupState::NotMatched).unwrap().id.clone();
    settled(&mut h, LocalCommand::Keep { group: group.clone() }).await;
    let path = dir.path().to_string_lossy().into_owned();
    let v = settled(&mut h, LocalCommand::RemoveFolder { path: path.clone() }).await;
    assert!(v.folders.is_empty());
    let v = settled(&mut h, LocalCommand::RestoreFolder { path }).await;
    assert_eq!(v.folders.len(), 1);
    assert_eq!(v.groups.iter().find(|g| g.id == group).unwrap().state, GroupState::Kept);
}

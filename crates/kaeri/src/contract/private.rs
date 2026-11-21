//! Private watching and reading: what the engine says of the mode, and what it keeps.
//!
//! The mode is said at launch and on every change, a search made while it is on is not kept and
//! is left out of the log, and a title kept always private is kept on this device only, out of
//! the sync log.

use kaeri_engine::private::redact;
use kaeri_model::{
    command::Command,
    event::Event,
    private::{PrivateMode, PrivateTitle},
    progress::TitleProgress,
    search::SearchQuery,
};
use kaeri_testkit::{Operation, fixtures::FINISHED};
use kaeri_tui::keymap::Action;

use super::progress::{opened, seed_partway};
use crate::harness::{Harness, names};

fn private_said(updates: &[Event]) -> Vec<bool> {
    updates
        .iter()
        .filter_map(|u| match u {
            Event::Private(on) => Some(*on),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn the_mode_is_said_at_once_both_ways() {
    let mut h = Harness::new().await;
    h.send(Command::SetPrivate { on: true });
    assert_eq!(private_said(&h.settle().await), [true]);
    h.send(Command::SetPrivate { on: false });
    assert_eq!(private_said(&h.settle().await), [false]);
}

#[tokio::test]
async fn a_search_while_private_is_not_remembered() {
    let mut h = Harness::new().await;
    h.send(Command::SetPrivate { on: true });
    h.send(Command::RememberSearch(SearchQuery::text("lantern ferry")));
    let updates = h.settle().await;
    assert!(!names(&updates).contains(&"RecentSearches"), "{:?}", names(&updates));
    assert!(kaeri_engine::catalog::recents::recent(h.store()).is_empty());

    h.send(Command::SetPrivate { on: false });
    h.send(Command::RememberSearch(SearchQuery::text("paper moon")));
    h.settle().await;
    assert_eq!(kaeri_engine::catalog::recents::recent(h.store()).len(), 1, "kept again");
}

#[tokio::test]
async fn a_live_search_while_private_leaves_out_what_it_searched_for_not_the_typing() {
    const WORDS: &str = "quill harbour";
    let mut h = Harness::new().await;
    h.send(Command::SetPrivate { on: true });
    let typed: Vec<&str> = (1..=WORDS.len()).map(|n| &WORDS[..n]).collect();
    for text in &typed {
        h.send(Command::SearchLive { query: SearchQuery::text(*text) });
    }
    h.settle().await;
    let searched: Vec<String> = h
        .server()
        .requests_for(Operation::Search)
        .iter()
        .filter_map(|asked| asked.variables["r0_search"].as_str().map(str::to_owned))
        .collect();
    assert!(searched.iter().any(|text| text == WORDS), "searched {searched:?}");
    // The words searched for are left out of the log and the rest of the typing is not,
    // however the waits between the keystrokes fell.
    for text in typed.iter().map(|text| text.trim()).filter(|text| text.len() >= 3) {
        let left_out = redact::line(text) == redact::HIDDEN;
        assert_eq!(
            left_out,
            searched.iter().any(|s| s == text),
            "{text:?}, searched {searched:?}"
        );
    }
}

#[tokio::test]
async fn a_hand_mark_goes_through_while_private() {
    let mut h = Harness::new().await;
    h.send(Command::SetPrivate { on: true });
    h.send(Command::SetWatched { id: FINISHED, episodes: vec!["1".into()], watched: true });
    let updates = h.settle().await;
    assert_eq!(h.store().completed_episodes(FINISHED).unwrap(), ["1"], "asked for by hand");
    let progress = updates.iter().find_map(|u| match u {
        Event::Progress { progress, private, .. } => Some((*progress, *private)),
        _ => None,
    });
    assert_eq!(
        progress,
        Some((Some(TitleProgress { done: 1, next: 2 }), Some(PrivateMode::Session))),
        "the standing says the mode"
    );
}

#[tokio::test]
async fn always_private_is_kept_on_this_device_and_never_synced() {
    let mut h = Harness::new().await;
    let title = PrivateTitle::Show { id: FINISHED };
    h.settle().await;
    let before = h.store().sync_op_count().unwrap();
    h.send(Command::SetAlwaysPrivate { title: title.clone(), on: true });
    let updates = h.settle().await;
    assert!(
        updates.iter().any(|u| matches!(u, Event::AlwaysPrivate { on: true, .. })),
        "{:?}",
        names(&updates)
    );
    assert_eq!(h.store().private_titles().unwrap(), [title.key()]);
    assert_eq!(h.store().sync_op_count().unwrap(), before, "nothing for the sync log");
}

#[tokio::test]
async fn ctrl_p_puts_the_chip_in_the_header_once_the_engine_says_so() {
    let mut h = Harness::new().await;
    h.settle().await;
    assert!(!h.app().private_on(), "a launch starts with the mode off");
    h.press(Action::TogglePrivate);
    assert_eq!(private_said(&h.settle().await), [true]);
    assert!(h.app().private_on());
    let screen = h.screen();
    let header = screen.lines().next().unwrap_or_default();
    assert!(header.contains("◌ private"), "{header}");
    assert!(screen.contains("Private mode is on."), "the notice:\n{screen}");

    h.press(Action::TogglePrivate);
    assert_eq!(private_said(&h.settle().await), [false]);
    assert!(!h.screen().lines().next().unwrap_or_default().contains("private"));
}

#[tokio::test]
async fn always_private_from_the_title_page_says_so_under_its_main_action() {
    let mut h = Harness::new().await;
    seed_partway(&h);
    opened(&mut h).await;
    assert_eq!(h.app().private_line(FINISHED), None, "nothing private yet");

    h.press(Action::ToggleAlwaysPrivate);
    let updates = h.settle().await;
    assert!(
        updates.iter().any(|u| matches!(u, Event::AlwaysPrivate { on: true, .. })),
        "{:?}",
        names(&updates)
    );
    assert!(h.app().always_private(FINISHED));
    let screen = h.screen();
    assert!(screen.contains("◌ Always private · Not saved or synced"), "{screen}");

    h.press(Action::ToggleAlwaysPrivate);
    h.settle().await;
    assert!(!h.app().always_private(FINISHED));
    assert!(!h.screen().contains("Always private"));
}

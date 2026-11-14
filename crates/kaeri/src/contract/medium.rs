//! What Kaeri is for: asked of the engine as the terminal starts, as the apps ask it, so the
//! rail has Read and the question the engine holds is asked once over the screen.

use kaeri_model::medium::MediumAsk;
use kaeri_tui::{
    keymap::Action,
    nav::{Overlay, Section},
};

use crate::harness::{Harness, Options};

/// The terminal started on `config`, or on none for a new install, with what launch asked
/// answered.
async fn launched(config: Option<&str>, store: Option<kaeri_store::Store>) -> Harness {
    let mut h = Harness::with(Options {
        config: config.map(str::to_owned),
        store,
        reading: true,
        ..Options::default()
    })
    .await;
    h.settle().await;
    h
}

#[tokio::test]
async fn a_computer_set_to_shows_and_reading_has_read_in_its_rail() {
    let h = launched(Some("[reading]\nkaeri_is_for = \"both\"\n"), None).await;
    assert!(h.app().both());
    assert!(h.app().in_rail(Section::Read));
    assert_eq!(h.app().nav.overlay(), None, "an answered question is not asked again");
}

#[tokio::test]
async fn a_new_install_asks_on_its_first_run_and_will_not_be_put_off() {
    let mut h = launched(None, None).await;
    assert_eq!(h.app().medium_question(), Some(&MediumAsk::FirstRun));
    assert_eq!(h.app().nav.overlay(), Some(&Overlay::Medium));
    h.press(Action::Back);
    assert_eq!(h.app().nav.overlay(), Some(&Overlay::Medium));
}

#[tokio::test]
async fn a_config_from_before_reading_is_asked_once_over_home() {
    let earlier = "[theme]\nmode = \"immersive\"\n";
    let mut h = launched(Some(earlier), None).await;
    assert_eq!(h.app().medium_question(), Some(&MediumAsk::Updated));
    assert_eq!(h.app().nav.overlay(), Some(&Overlay::Medium));
    assert!(!h.app().in_rail(Section::Read), "shows alone until answered");

    // Shows and reading, the third choice.
    h.press(Action::Down);
    h.press(Action::Down);
    h.press(Action::Open);
    h.settle().await;
    assert_eq!(h.app().nav.overlay(), None);
    assert!(h.app().both() && h.app().in_rail(Section::Read));
    let written = std::fs::read_to_string(&h.paths().config_file).unwrap();
    assert!(written.contains("kaeri_is_for = \"both\""), "{written}");

    // Started again on what it wrote: never asked again.
    let store = h.store().clone();
    let _ = h.quit().await;
    let h = launched(Some(&written), Some(store)).await;
    assert_eq!(h.app().medium_question(), None);
    assert_eq!(h.app().nav.overlay(), None);
    assert!(h.app().both());
}

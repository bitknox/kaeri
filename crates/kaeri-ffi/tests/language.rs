//! The language an app starts the engine in is the one the engine's and the model's words come
//! in: the headings, count lines and search words the app asks for once it has started.
//!
//! The language is the process's, so this file holds one test: a second beside it, in the same
//! process, would race it for the language.
#![cfg(feature = "testing")]

use std::sync::Arc;

use kaeri_ffi::{FfiDirs, FfiEngine, FfiHost, FfiStart};
use kaeri_model::browsing::TrendPeriod;

/// A host that opens nothing, shows nothing and keeps no secrets.
struct Quiet;

impl FfiHost for Quiet {
    fn open_url(&self, _url: String) -> bool {
        false
    }

    fn notify(&self, _title: String, _body: String) {}

    fn secret_get(&self, _id: String) -> Option<String> {
        None
    }

    fn secret_set(&self, _id: String, _value: String) -> bool {
        false
    }

    fn secret_delete(&self, _id: String) -> bool {
        true
    }
}

/// An engine started for a test in `root`, in the language `start` names.
fn started(root: &std::path::Path, start: FfiStart) -> Arc<FfiEngine> {
    let dir = |name: &str| root.join(name).display().to_string();
    let dirs = FfiDirs { config: dir("config"), data: dir("data"), cache: dir("cache") };
    FfiEngine::start_testing(dirs, Arc::new(Quiet), start).expect("the engine starts")
}

#[test]
fn the_words_come_in_the_language_the_app_starts_in() {
    let first = tempfile::tempdir().expect("a temporary directory");
    let english = started(first.path(), FfiStart::default());
    assert_eq!(TrendPeriod::Today.label(), "Trending today");
    english.shutdown();

    let second = tempfile::tempdir().expect("a temporary directory");
    let german =
        started(second.path(), FfiStart { language: Some("de".into()), ..FfiStart::default() });
    assert_eq!(TrendPeriod::Today.label(), "Heute im Trend");
    assert_eq!(TrendPeriod::Week.choice(), "Diese Woche");

    // A start that names no language leaves the one set before it.
    german.shutdown();
    let third = tempfile::tempdir().expect("a temporary directory");
    let unsaid = started(third.path(), FfiStart::default());
    assert_eq!(TrendPeriod::Today.label(), "Heute im Trend");
    unsaid.shutdown();
}

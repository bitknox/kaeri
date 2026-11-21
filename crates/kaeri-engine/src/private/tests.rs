use std::sync::Arc;

use kaeri_core::{books::WorkId, ids::AnilistId};
use kaeri_model::{
    event::Event,
    private::{PrivateMode, PrivateTitle},
    progress::TitleProgress,
};
use kaeri_store::{Store, WatchEvent, reading::ReadEvent};

use super::{Privacy, redact::Redactor};

const SHOW: AnilistId = AnilistId::new(900_417);
const OTHER: AnilistId = AnilistId::new(900_418);

fn standing(id: AnilistId) -> Event {
    Event::Progress {
        id,
        progress: Some(TitleProgress { done: 4, next: 5 }),
        resume: None,
        episodes: Vec::new(),
        private: None,
        kept: false,
    }
}

#[test]
fn the_mode_starts_off_and_switches_both_ways() {
    let privacy = Privacy::default();
    assert!(!privacy.is_on(), "a launch starts with it off");
    assert!(privacy.set(true));
    assert!(!privacy.set(true), "already on");
    assert!(privacy.holds_show(SHOW) && privacy.holds_series("a:30001"));
    assert_eq!(privacy.show_mode(SHOW), Some(PrivateMode::Session));
    assert!(privacy.set(false));
    assert!(!privacy.holds_show(SHOW));
    assert_eq!(privacy.show_mode(SHOW), None);
}

#[test]
fn an_always_private_title_is_held_with_the_mode_off_and_survives_a_relaunch() {
    let store = Store::open_in_memory().unwrap();
    let privacy = Privacy::default();
    privacy.set_always(&store, &PrivateTitle::Show { id: SHOW }, true).unwrap();
    let work = PrivateTitle::Series { work: WorkId::new("a:30001") };
    privacy.set_always(&store, &work, true).unwrap();
    assert!(privacy.holds_show(SHOW) && !privacy.holds_show(OTHER));
    assert_eq!(privacy.show_mode(SHOW), Some(PrivateMode::Always));
    assert_eq!(privacy.series_mode("a:30001"), Some(PrivateMode::Always));

    let relaunched = Privacy::default();
    relaunched.load(&store);
    assert!(relaunched.show_always(SHOW) && relaunched.series_always("a:30001"));
    relaunched.set_always(&store, &PrivateTitle::Show { id: SHOW }, false).unwrap();
    let again = Privacy::default();
    again.load(&store);
    assert!(!again.show_always(SHOW) && again.series_always("a:30001"));
}

#[test]
fn a_place_is_kept_in_memory_and_gone_with_a_new_launch() {
    let privacy = Arc::new(Privacy::default());
    privacy.set(true);
    let gate = privacy.gate(SHOW);
    let mut watch = WatchEvent::new(SHOW, "6", 724.0);
    watch.duration_secs = Some(1440.0);
    assert!(gate.keep(&watch));
    assert_eq!(privacy.resume_kept(SHOW, "6"), Some(724.0));
    assert_eq!(privacy.resume_kept(SHOW, "7"), None, "only the episode it was in");

    // Turned off: the place stays in memory, and the gate writes again.
    privacy.set(false);
    assert!(!gate.holds());
    assert!(!gate.keep(&WatchEvent::new(SHOW, "6", 900.0)));
    assert_eq!(privacy.resume_kept(SHOW, "6"), Some(724.0));

    assert_eq!(Privacy::default().resume_kept(SHOW, "6"), None, "a new launch has none");
}

#[test]
fn a_finished_episode_resumes_nothing_and_moves_the_title_on() {
    let privacy = Privacy::default();
    let mut watch = WatchEvent::new(SHOW, "6", 1400.0);
    watch.completed = true;
    privacy.keep_watch(watch);
    assert_eq!(privacy.resume_kept(SHOW, "6"), None);
    let mut event = standing(SHOW);
    privacy.overlay(&mut event);
    let Event::Progress { progress, resume, kept, .. } = event else { unreachable!() };
    assert_eq!(progress, Some(TitleProgress { done: 4, next: 7 }));
    assert_eq!(resume, None);
    assert!(kept);
}

#[test]
fn the_overlay_puts_the_kept_place_and_the_mode_on_a_titles_standing() {
    let privacy = Privacy::default();
    privacy.set(true);
    let mut watch = WatchEvent::new(SHOW, "6", 720.0);
    watch.duration_secs = Some(1440.0);
    privacy.keep_watch(watch);
    let mut event = standing(SHOW);
    privacy.overlay(&mut event);
    let Event::Progress { progress, resume, private, kept, .. } = event else { unreachable!() };
    assert_eq!(progress, Some(TitleProgress { done: 4, next: 6 }));
    let resume = resume.expect("resumes from memory");
    assert_eq!((resume.position, resume.fraction), (720.0, Some(0.5)));
    assert_eq!(private, Some(PrivateMode::Session));
    assert!(kept);

    // Another title: the mode, and nothing kept.
    let mut event = standing(OTHER);
    privacy.overlay(&mut event);
    let Event::Progress { progress, private, kept, .. } = event else { unreachable!() };
    assert_eq!(progress, Some(TitleProgress { done: 4, next: 5 }));
    assert_eq!((private, kept), (Some(PrivateMode::Session), false));
}

#[test]
fn a_read_place_is_kept_by_series() {
    let privacy = Privacy::default();
    let mut read = ReadEvent::new("a:30001", "12", 1);
    read.page = Some(8);
    privacy.keep_read(read.clone());
    assert_eq!(privacy.read_kept("a:30001"), Some(read));
    assert_eq!(privacy.read_kept("a:30002"), None);
}

#[test]
fn notices_and_widgets_leave_out_always_private_titles() {
    let store = Store::open_in_memory().unwrap();
    let privacy = Privacy::default();
    privacy.set_always(&store, &PrivateTitle::Show { id: SHOW }, true).unwrap();
    let mut event = Event::UpcomingNotices(kaeri_model::notifications::UpcomingNotices {
        airings: vec![airing(SHOW), airing(OTHER)],
        ..Default::default()
    });
    privacy.overlay(&mut event);
    let Event::UpcomingNotices(notices) = event else { unreachable!() };
    assert_eq!(notices.airings.iter().map(|a| a.id).collect::<Vec<_>>(), [OTHER]);
}

fn airing(id: AnilistId) -> kaeri_model::notifications::UpcomingNotice {
    kaeri_model::notifications::UpcomingNotice {
        id,
        title: "Kite Weather".into(),
        episode: 7,
        airs_at: 1_800_000_000,
        reason: kaeri_model::notifications::NoticeReason::Watching,
        next_to_watch: 7,
        cover_url: None,
        banner_url: None,
        notifies: true,
        translation: None,
    }
}

#[test]
fn a_download_asked_for_while_private_stays_private() {
    let store = Store::open_in_memory().unwrap();
    let privacy = Privacy::default();
    privacy.note_download(2);
    assert!(!privacy.download_private(1, SHOW));
    assert!(privacy.download_private(2, SHOW), "turning it off does not name it");
    privacy.set_always(&store, &PrivateTitle::Show { id: OTHER }, true).unwrap();
    assert!(privacy.download_private(3, OTHER), "an always-private show's downloads");
}

#[test]
fn the_log_leaves_out_learnt_words_and_ids() {
    let log = Redactor::new();
    log.word("Kite Weather");
    log.word("ok");
    log.show_id(SHOW);
    log.work_id("a:30001");
    let line =
        log.line("play started title=\"KITE weather\" id=900417 other=9004170 work=a:30001 ok");
    assert_eq!(
        line,
        "play started title=\"[private]\" id=[private] other=9004170 work=[private] ok"
    );
}

#[test]
fn a_learnt_word_is_left_out_where_it_stands_alone_not_inside_another() {
    let log = Redactor::new();
    // A search for "the" and one for "202", made while private, and a private title.
    log.word("the");
    log.word("202");
    // A shorter word learnt first still leaves the longer one to be taken out whole.
    log.word("Kite");
    log.word("Kite Weather");
    assert_eq!(
        log.line("2026-11-21T10:12:03Z INFO the other source failed, then authenticated"),
        "2026-11-21T10:12:03Z INFO [private] other source failed, then authenticated"
    );
    assert_eq!(
        log.line("title=\"KITE WEATHER\" next: Kite Weather's 2nd (202)"),
        "title=\"[private]\" next: [private]'s 2nd ([private])"
    );
}

#[test]
fn a_learnt_word_is_left_out_in_any_case_where_lowercasing_changes_lengths() {
    let log = Redactor::new();
    log.word("\u{130}lara Lantern");
    assert_eq!(log.line("staging \u{130}LARA LANTERN 03"), "staging [private] 03");
    // Lowercased, the dotted capital I takes a byte more and the ohm sign a byte less: the
    // line keeps its length while its letters move.
    log.word("ab\u{130}");
    assert_eq!(log.line("ab\u{130}\u{2126} done"), "[private]\u{2126} done");
}

#[test]
fn the_keystrokes_of_a_live_search_while_private_are_not_left_out() {
    use kaeri_model::{command::Command, search::SearchQuery};
    // The process's log: words are only added, so this does not disturb other tests.
    let privacy = Privacy::default();
    privacy.set(true);
    for text in ["qui", "quill", "quill harb", "quill harbour"] {
        super::note_command(&privacy, &Command::SearchLive { query: SearchQuery::text(text) });
    }
    // The live search notes the words it searches for once it runs; the keystrokes note none.
    assert_eq!(super::redact::line("quill pen, quill harb"), "quill pen, quill harb");
    // Return searches at once, and its words are left out from then on.
    super::note_command(
        &privacy,
        &Command::Search { query: SearchQuery::text("quill harbour") },
    );
    privacy.set(false);
    assert_eq!(super::redact::line("q=Quill Harbour, quill pen"), "q=[private], quill pen");
}

#[test]
fn the_log_leaves_out_addresses_only_while_private() {
    let log = Redactor::new();
    let text = "fetch https://media.example/a/b.m3u8?t=1 failed, magnet:?xt=urn:btih:abc too";
    assert_eq!(log.line(text), text, "nothing private: the log is as it was");
    log.session(true);
    assert_eq!(log.line(text), "fetch [private] failed, [private] too");
    log.session(false);
    {
        let _open = log.open();
        assert!(log.active(), "an always-private play hides addresses too");
        assert_eq!(log.line("GET http://cdn.example/x 404"), "GET [private] 404");
    }
    assert!(!log.active());
}

#[test]
fn a_search_while_private_is_left_out_of_the_log() {
    // The process's log: words are only added, so this does not disturb other tests.
    let privacy = Privacy::default();
    let command = |text: &str| kaeri_model::command::Command::ManualSearch {
        id: SHOW,
        query: text.to_owned(),
        provider_id: None,
    };
    super::note_command(&privacy, &command("Lantern Ferry Unique Words"));
    assert!(super::redact::line("q=Lantern Ferry Unique Words").contains("Lantern"));
    privacy.set(true);
    super::note_command(&privacy, &command("Paper Moon Unique Words"));
    privacy.set(false);
    assert_eq!(super::redact::line("q=paper moon unique words"), "q=[private]");
}

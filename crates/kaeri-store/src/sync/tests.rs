use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use kaeri_core::ids::AnilistId;

use super::*;
use crate::{Store, WatchEvent, history::is_complete};

const SHOW: AnilistId = AnilistId::new(4_001);
const OTHER: AnilistId = AnilistId::new(4_002);

/// A store with sync on and a clock of its own, in milliseconds.
struct Dev {
    store: Store,
    clock: Arc<AtomicI64>,
    id: String,
}

fn dev(id: &str, start_secs: i64) -> Dev {
    let store = Store::open_in_memory().unwrap();
    let clock = Arc::new(AtomicI64::new(start_secs * 1000));
    let c = clock.clone();
    store.set_sync_clock(move || c.load(Ordering::SeqCst));
    store.sync_enable(id).unwrap();
    Dev { store, clock, id: id.to_owned() }
}

impl Dev {
    fn advance(&self, secs: i64) {
        self.clock.fetch_add(secs * 1000, Ordering::SeqCst);
    }

    fn secs(&self) -> i64 {
        self.clock.load(Ordering::SeqCst) / 1000
    }

    fn watch(&self, id: AnilistId, ep: &str, position: f64, duration: f64) {
        self.advance(1);
        let mut e = WatchEvent::new(id, ep, position);
        e.duration_secs = Some(duration);
        e.at = self.secs();
        e.completed = is_complete(position, Some(duration), 0.85);
        self.store.record_event(&e).unwrap();
    }

    fn finish(&self, id: AnilistId, ep: &str) {
        self.watch(id, ep, 1400.0, 1440.0);
    }
}

/// Everything `from` holds that `to` lacks, applied on `to`.
fn send(from: &Dev, to: &Dev) -> Applied {
    let ops = from.store.ops_after(&to.store.version_vector().unwrap(), usize::MAX).unwrap();
    to.store.apply_ops(ops).unwrap()
}

fn exchange(a: &Dev, b: &Dev) {
    send(a, b);
    send(b, a);
}

/// The merged state, for comparing two devices.
fn dump(s: &Store) -> String {
    s.with_personal(|c| {
        let mut out = String::new();
        let mut table = |name: &str, sql: &str| -> rusqlite::Result<()> {
            out.push_str(&format!("[{name}]\n"));
            let mut stmt = c.prepare(sql)?;
            let n = stmt.column_count();
            let rows = stmt.query_map([], |r| {
                (0..n).map(|i| r.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}"))).collect::<rusqlite::Result<Vec<_>>>()
            })?;
            for row in rows {
                out.push_str(&row?.join("|"));
                out.push('\n');
            }
            Ok(())
        };
        table("events", "SELECT uid, anilist_id, episode, position_secs, completed, stamp FROM watch_event ORDER BY uid")?;
        table("progress", "SELECT anilist_id, last_episode, last_position, episodes_done, hidden FROM watch_progress ORDER BY anilist_id")?;
        table("regs", "SELECT kind, key, stamp, device, value FROM sync_reg ORDER BY kind, key")?;
        table("tomb", "SELECT uid FROM sync_tomb ORDER BY uid")?;
        table("show", "SELECT anilist_id, setting, value FROM show_setting ORDER BY 1, 2")?;
        table("pin", "SELECT anilist_id, provider_id, release_id FROM provider_preference ORDER BY 1")?;
        table("marks", "SELECT tracker, anilist_id, episode FROM sync_mark ORDER BY 1, 2")?;
        table("peers", "SELECT device, removed FROM sync_peer ORDER BY 1")?;
        Ok(out)
    })
    .unwrap()
}

#[test]
fn sync_off_writes_nothing_and_changes_nothing() {
    let s = Store::open_in_memory().unwrap();
    let mut e = WatchEvent::new(SHOW, "1", 100.0);
    e.duration_secs = Some(1440.0);
    s.record_event(&e).unwrap();
    s.hide_from_continue(SHOW).unwrap();
    s.set_show_setting(SHOW, "skip", Some("on"), 5).unwrap();
    assert_eq!(s.sync_op_count().unwrap(), 0);
    assert_eq!(s.sync_device().unwrap(), None);
    assert!(s.progress(SHOW).unwrap().unwrap().last_position > 99.0);
}

#[test]
fn enabling_turns_todays_history_into_the_first_ops() {
    let s = Store::open_in_memory().unwrap();
    for (ep, at) in [("1", 1_000), ("2", 2_000), ("3", 3_000)] {
        let mut e = WatchEvent::new(SHOW, ep, 1400.0);
        e.duration_secs = Some(1440.0);
        e.completed = true;
        e.at = at;
        s.record_event(&e).unwrap();
    }
    s.set_show_setting(SHOW, "skip", Some("on"), 2_500).unwrap();
    s.hide_from_continue(SHOW).unwrap();
    s.sync_enable("aaaa").unwrap();
    // Three events, one setting, one hide.
    assert_eq!(s.sync_op_count().unwrap(), 5);
    assert_eq!(s.version_vector().unwrap()["aaaa"], 5);
    let hidden = s.continue_list(10).unwrap();
    assert!(hidden.is_empty(), "still hidden after the backfill");
    // Again, even after turning it off: the log and its numbering stay.
    s.sync_enable("aaaa").unwrap();
    s.sync_disable().unwrap();
    s.set_show_setting(SHOW, "skip", Some("off"), 9).unwrap();
    assert_eq!(s.sync_op_count().unwrap(), 5, "off writes nothing");
    s.sync_enable("aaaa").unwrap();
    s.set_show_setting(SHOW, "skip", Some("on"), 10).unwrap();
    assert_eq!(s.sync_op_count().unwrap(), 6);
    assert!(s.sync_enable("bbbb").is_err(), "a log belongs to one device");
}

#[test]
fn a_new_device_receives_the_whole_history_of_another() {
    let a = dev("aaaa", 10_000);
    a.finish(SHOW, "1");
    a.finish(SHOW, "2");
    a.watch(SHOW, "3", 300.0, 1440.0);
    let b = dev("bbbb", 20_000);
    let applied = send(&a, &b);
    assert_eq!(applied.ops, 3);
    assert_eq!(dump(&a.store).split("[regs]").next(), dump(&b.store).split("[regs]").next());
    let p = b.store.progress(SHOW).unwrap().unwrap();
    assert_eq!((p.last_episode.as_str(), p.episodes_done), ("3", 2));
    assert_eq!(b.store.last_event_origin(SHOW.get()).unwrap().unwrap().0, "aaaa");
    // Nothing is sent twice, and nothing received is sent on as new.
    assert_eq!(send(&a, &b).ops, 0);
    assert_eq!(b.store.version_vector().unwrap()["aaaa"], 3);
}

#[test]
fn the_newest_position_wins_and_both_events_stay() {
    let phone = dev("aaaa", 10_000);
    let tv = dev("bbbb", 10_000);
    phone.watch(SHOW, "7", 724.0, 1440.0);
    phone.advance(3_600);
    tv.advance(7_200);
    tv.watch(SHOW, "8", 190.0, 1440.0);
    exchange(&phone, &tv);
    for d in [&phone, &tv] {
        let p = d.store.progress(SHOW).unwrap().unwrap();
        assert_eq!((p.last_episode.as_str(), p.last_position), ("8", 190.0));
        assert_eq!(d.store.events_for(SHOW, 10).unwrap().len(), 2);
    }
    assert_eq!(dump(&phone.store), dump(&tv.store));
}

#[test]
fn a_clock_an_hour_fast_cannot_hide_later_changes() {
    let fast = dev("aaaa", 10_000 + 3_600);
    let normal = dev("bbbb", 10_000);
    fast.store.set_show_setting(SHOW, "skip", Some("on"), 1).unwrap();
    send(&fast, &normal);
    // The normal device changes it after seeing the fast one's: its clock has moved past.
    normal.advance(5);
    normal.store.set_show_setting(SHOW, "skip", Some("off"), 2).unwrap();
    exchange(&fast, &normal);
    for d in [&fast, &normal] {
        assert_eq!(d.store.show_settings(SHOW).unwrap(), vec![("skip".into(), "off".into())]);
    }
}

#[test]
fn skip_filler_off_wins_on_every_device_when_it_is_later() {
    let ipad = dev("aaaa", 10_000);
    let mac = dev("bbbb", 10_000);
    ipad.store.set_show_setting(SHOW, "filler", Some("skip"), 1).unwrap();
    mac.advance(3_600);
    mac.store.set_show_setting(SHOW, "filler", Some("keep"), 2).unwrap();
    exchange(&ipad, &mac);
    for d in [&ipad, &mac] {
        assert_eq!(
            d.store.show_settings(SHOW).unwrap(),
            vec![("filler".into(), "keep".into())]
        );
    }
    // Ties break by device id, the same on both sides.
    let (x, y) = (dev("aaaa", 50_000), dev("bbbb", 50_000));
    x.store.sync_register("setting", "playback.skip", Some("1"), Some(70_000_000)).unwrap();
    y.store.sync_register("setting", "playback.skip", Some("0"), Some(70_000_000)).unwrap();
    exchange(&x, &y);
    assert_eq!(dump(&x.store), dump(&y.store));
    assert_eq!(
        x.store.sync_reg_value("setting", "playback.skip").unwrap().as_deref(),
        Some("0")
    );
}

#[test]
fn an_unwatch_never_removes_a_watch_it_did_not_see() {
    let mac = dev("aaaa", 10_000);
    let phone = dev("bbbb", 10_000);
    mac.finish(SHOW, "5");
    exchange(&mac, &phone);
    // Offline: the Mac takes episode 5 back while the phone plays it to the end again.
    mac.advance(60);
    mac.store.unmark_episode(SHOW, "5").unwrap();
    phone.advance(120);
    phone.finish(SHOW, "5");
    let to_mac = send(&phone, &mac);
    let to_phone = send(&mac, &phone);
    for d in [&mac, &phone] {
        assert_eq!(
            d.store.completed_episodes(SHOW).unwrap(),
            vec!["5".to_string()],
            "{}",
            d.id
        );
    }
    assert_eq!(dump(&mac.store), dump(&phone.store));
    // The surprise is kept on each device once, with the action that reverses it on the one that
    // took the episode back.
    assert_eq!((to_mac.notes.len(), to_phone.notes.len()), (1, 1));
    for n in to_mac.notes.iter().chain(&to_phone.notes) {
        assert_eq!((n.unwatch_device.as_str(), n.watch_device.as_str()), ("aaaa", "bbbb"));
    }
    assert_eq!(mac.store.sync_notes(0).unwrap().len(), 1);
    assert_eq!(phone.store.sync_notes(0).unwrap().len(), 1);
    // And again changes nothing.
    exchange(&mac, &phone);
    assert_eq!(mac.store.sync_notes(0).unwrap().len(), 1);
}

#[test]
fn an_unwatch_removes_what_it_saw_on_every_device() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    a.finish(SHOW, "5");
    exchange(&a, &b);
    b.advance(10);
    b.store.forget_episode(SHOW, "5").unwrap();
    exchange(&a, &b);
    for d in [&a, &b] {
        assert!(d.store.completed_episodes(SHOW).unwrap().is_empty());
        assert!(d.store.progress(SHOW).unwrap().is_none());
    }
    assert!(a.store.sync_notes(0).unwrap().is_empty());
}

#[test]
fn an_unwatch_arriving_before_the_watch_still_wins_through_its_tombstone() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    let c = dev("cccc", 10_000);
    a.finish(SHOW, "5");
    send(&a, &b);
    b.advance(10);
    b.store.unmark_episode(SHOW, "5").unwrap();
    // C hears from B first, then from A.
    let from_b = b.store.ops_after(&Default::default(), usize::MAX).unwrap();
    let (b_ops, a_ops): (Vec<_>, Vec<_>) = from_b.into_iter().partition(|o| o.device == "bbbb");
    c.store.apply_ops(b_ops).unwrap();
    c.store.apply_ops(a_ops).unwrap();
    assert!(c.store.completed_episodes(SHOW).unwrap().is_empty());
    assert_eq!(c.store.events_for(SHOW, 10).unwrap().len(), 0);
}

#[test]
fn hidden_from_continue_comes_back_only_by_a_later_watch() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    a.watch(SHOW, "1", 300.0, 1440.0);
    exchange(&a, &b);
    b.advance(10);
    b.store.hide_from_continue(SHOW).unwrap();
    exchange(&a, &b);
    assert!(a.store.continue_list(10).unwrap().is_empty());
    assert!(b.store.continue_list(10).unwrap().is_empty());
    a.advance(100);
    a.watch(SHOW, "1", 400.0, 1440.0);
    exchange(&a, &b);
    assert_eq!(a.store.continue_list(10).unwrap().len(), 1);
    assert_eq!(b.store.continue_list(10).unwrap().len(), 1);
}

#[test]
fn a_month_offline_is_one_exchange() {
    let home = dev("aaaa", 10_000);
    let away = dev("bbbb", 10_000);
    exchange(&home, &away);
    for day in 0..30 {
        away.advance(86_400);
        away.finish(OTHER, &(day + 1).to_string());
        away.store.set_show_setting(SHOW, "n", Some(&day.to_string()), 1).unwrap();
    }
    home.advance(86_400 * 10);
    home.watch(SHOW, "1", 10.0, 1440.0);
    let sent = send(&away, &home);
    assert_eq!(sent.ops, 60);
    send(&home, &away);
    assert_eq!(dump(&home.store), dump(&away.store));
    assert_eq!(home.store.progress(OTHER).unwrap().unwrap().episodes_done, 30);
    assert_eq!(home.store.show_settings(SHOW).unwrap(), vec![("n".into(), "29".into())]);
}

#[test]
fn ops_apply_once_and_wait_for_the_one_before() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    a.finish(SHOW, "1");
    a.finish(SHOW, "2");
    a.finish(SHOW, "3");
    let ops = a.store.ops_after(&Default::default(), 10).unwrap();
    // Seq 2 and 3 first: they wait.
    let held = b.store.apply_ops(vec![ops[1].clone(), ops[2].clone()]).unwrap();
    assert_eq!((held.ops, held.gaps), (0, 2));
    let all = b.store.apply_ops(ops.clone()).unwrap();
    assert_eq!(all.ops, 3);
    assert_eq!(b.store.apply_ops(ops).unwrap().duplicates, 3);
    assert_eq!(b.store.completed_episodes(SHOW).unwrap().len(), 3);
}

#[test]
fn any_order_of_the_same_ops_gives_the_same_state() {
    // Three devices make random changes offline; four other stores take every device's log in a
    // different interleaving, and all must end the same.
    let mut seed = 0x2545_f491_4f6c_dd1d_u64;
    let mut rnd = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    for round in 0..12 {
        let devs = [dev("aaaa", 10_000), dev("bbbb", 10_000 + round), dev("cccc", 9_000)];
        for _ in 0..40 {
            let d = &devs[rnd(3) as usize];
            d.advance(rnd(50) as i64);
            let id = [SHOW, OTHER][rnd(2) as usize];
            let ep = (1 + rnd(4)).to_string();
            match rnd(9) {
                0..=2 => d.finish(id, &ep),
                3 => d.watch(id, &ep, 100.0 + rnd(900) as f64, 1440.0),
                4 => d.store.unmark_episode(id, &ep).unwrap().map(drop).unwrap_or(()),
                5 => d.store.forget_episode(id, &ep).unwrap(),
                6 => d.store.hide_from_continue(id).unwrap(),
                7 => d.store.set_show_setting(id, "s", Some(&rnd(5).to_string()), 1).unwrap(),
                _ => {
                    d.store.adopt_progress(id, rnd(6) as u32, d.secs()).unwrap();
                }
            }
            // Now and then two devices meet, which is what makes ops name each other's events.
            if rnd(6) == 0 {
                let (x, y) = (rnd(3) as usize, rnd(3) as usize);
                if x != y {
                    exchange(&devs[x], &devs[y]);
                }
            }
        }
        let logs: Vec<Vec<Op>> = devs
            .iter()
            .map(|d| d.store.ops_after(&Default::default(), usize::MAX).unwrap())
            .collect();
        let mut dumps = Vec::new();
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0], [1, 0, 2]] {
            let fresh = dev("zzzz", 10_000);
            // Apply in chunks, interleaved, with a different split each time.
            let mut cursors = [0usize; 3];
            let step = 1 + rnd(7) as usize;
            while cursors.iter().zip(&logs).any(|(c, l)| *c < l.len()) {
                for &i in &order {
                    let end = (cursors[i] + step).min(logs[i].len());
                    fresh.store.apply_ops(logs[i][cursors[i]..end].to_vec()).unwrap();
                    cursors[i] = end;
                }
            }
            dumps.push(dump(&fresh.store));
        }
        assert!(
            dumps.windows(2).all(|w| w[0] == w[1]),
            "round {round} diverged:\n{}\n---\n{}",
            dumps[0],
            dumps[1]
        );
    }
}

#[test]
fn per_show_choices_travel_and_a_reset_clears_them_everywhere() {
    use kaeri_core::{ids::ProviderKey, media::Translation, track_memory::TrackMemory};
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    a.store.set_show_setting(SHOW, "skip", Some("on"), 1).unwrap();
    a.store.set_provider_preference(SHOW, "src-one", 1).unwrap();
    a.store.remember_tracks(SHOW, Translation::Sub, &TrackMemory::default()).unwrap();
    a.store
        .set_override_titled(
            SHOW,
            "src-one",
            &ProviderKey::new("entry-9"),
            Some("Made Up Title"),
            1,
        )
        .unwrap();
    send(&a, &b);
    assert_eq!(b.store.show_settings(SHOW).unwrap().len(), 1);
    assert_eq!(b.store.provider_preference(SHOW).unwrap().as_deref(), Some("src-one"));
    assert!(b.store.track_memory(SHOW, Translation::Sub).unwrap().is_some());
    assert_eq!(dump(&a.store), dump(&b.store));
    b.advance(10);
    b.store.reset_show(SHOW).unwrap();
    b.store.clear_title_match(SHOW).unwrap();
    send(&b, &a);
    assert!(a.store.show_settings(SHOW).unwrap().is_empty());
    assert_eq!(a.store.provider_preference(SHOW).unwrap(), None);
    assert!(a.store.track_memory(SHOW, Translation::Sub).unwrap().is_none());
}

#[test]
fn a_trackers_count_syncs_and_leaves_the_resume_point_alone() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    a.watch(SHOW, "2", 300.0, 1440.0);
    a.advance(5);
    a.store.adopt_progress(SHOW, 6, a.secs()).unwrap();
    send(&a, &b);
    let p = b.store.progress(SHOW).unwrap().unwrap();
    assert_eq!((p.last_episode.as_str(), p.last_position, p.episodes_done), ("2", 300.0, 6));
    assert_eq!(dump(&a.store), dump(&b.store));
    // Taking one of its episodes back on the other device reaches the first.
    b.advance(5);
    b.store.unmark_episode(SHOW, "4").unwrap();
    send(&b, &a);
    assert_eq!(a.store.completed_episode_count(SHOW).unwrap(), 5);
}

#[test]
fn downloads_say_where_files_are_and_let_go_travels() {
    let mac = dev("aaaa", 10_000);
    let phone = dev("bbbb", 10_000);
    let d = mac
        .store
        .enqueue_download(SHOW, "3", "Made Up Show", "magnet:?xt=urn:btih:00")
        .unwrap();
    mac.store.finish_download(d.id, Some("/dl/x")).unwrap();
    send(&mac, &phone);
    assert_eq!(
        phone.store.remote_downloads().unwrap(),
        vec![("aaaa".into(), SHOW.get(), "3".into())]
    );
    assert!(phone.store.downloads().unwrap().is_empty(), "files never move");
    mac.advance(5);
    mac.store.remove_download(d.id).unwrap();
    send(&mac, &phone);
    assert!(phone.store.remote_downloads().unwrap().is_empty());
    assert!(
        phone.store.download_dismissed(SHOW, "3").unwrap(),
        "keep next will not refetch it"
    );
}

#[test]
fn devices_join_and_a_removal_wins_over_any_add() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    a.store.sync_device_add("cccc", "Hall phone", "ab").unwrap();
    a.advance(5);
    a.store.sync_device_remove("cccc").unwrap();
    // A later add of the same key, from elsewhere, changes nothing.
    b.store.sync_device_add("cccc", "x", "ab").unwrap();
    exchange(&a, &b);
    for d in [&a, &b] {
        let removed: i64 = d
            .store
            .with_personal(|c| {
                Ok(c.query_row("SELECT removed FROM sync_peer WHERE device='cccc'", [], |r| {
                    r.get(0)
                })?)
            })
            .unwrap();
        assert_eq!(removed, 1);
    }
}

#[test]
fn compaction_keeps_numbering_and_state() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    for i in 0..10 {
        a.advance(1);
        a.store.set_show_setting(SHOW, "n", Some(&i.to_string()), 1).unwrap();
    }
    a.finish(SHOW, "1");
    send(&a, &b);
    let acked = b.store.version_vector().unwrap();
    let before = dump(&a.store);
    assert_eq!(a.store.compact(&acked).unwrap(), 9);
    assert_eq!(dump(&a.store), before);
    // A device that joins later still gets contiguous numbers, and the same state.
    let c = dev("cccc", 10_000);
    send(&a, &c);
    assert_eq!(c.store.version_vector().unwrap()["aaaa"], 11);
    assert_eq!(c.store.show_settings(SHOW).unwrap(), vec![("n".into(), "9".into())]);
    assert_eq!(a.store.compact(&acked).unwrap(), 0, "nothing left to do");
}

#[test]
fn the_database_of_the_previous_release_migrates_and_syncs() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("kaeri.db");
    {
        let conn = rusqlite::Connection::open(&path).unwrap();
        crate::schema::migrate_until(&conn, Some("0021_sync_log")).unwrap();
        conn.execute(
            "INSERT INTO watch_event (anilist_id, episode, position_secs, duration_secs, watched_secs,
                                      completed, at) VALUES (4001, '1', 1400, 1440, 1400, 1, 1000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO watch_progress (anilist_id, last_episode, last_position, last_duration,
                                         episodes_done, updated_at) VALUES (4001, '1', 1400, 1440, 1, 1000)",
            [],
        )
        .unwrap();
    }
    let store = Store::open(&path).unwrap();
    assert_eq!(store.sync_device().unwrap(), None, "opening turns nothing on");
    assert_eq!(store.progress(SHOW).unwrap().unwrap().episodes_done, 1);
    store.sync_enable("aaaa").unwrap();
    let other = dev("bbbb", 10_000);
    other.store.apply_ops(store.ops_after(&Default::default(), 10).unwrap()).unwrap();
    assert_eq!(other.store.completed_episodes(SHOW).unwrap(), vec!["1".to_string()]);
}

#[test]
fn a_received_watch_is_filled_by_the_lead_for_a_tracker_its_origin_lacked() {
    // The phone watches with AniList queued; the Mac and the iPad are both signed in to
    // AniList and to Trakt, which the phone lacks.
    let phone = dev("cccc", 10_000);
    let mac = dev("aaaa", 10_000);
    let ipad = dev("bbbb", 10_000);
    let trackers = |t: &[&str]| t.iter().map(|s| (*s).to_owned()).collect::<Vec<_>>();
    for (d, t) in [
        (&phone, &["anilist"][..]),
        (&mac, &["anilist", "trakt"]),
        (&ipad, &["anilist", "trakt"]),
    ] {
        d.store.sync_set_my_trackers(&trackers(t)).unwrap();
    }
    phone.advance(1);
    let mut e = WatchEvent::new(SHOW, "3", 1400.0);
    e.duration_secs = Some(1440.0);
    e.at = phone.secs();
    e.completed = true;
    phone.store.record_event_queued(&e, trackers(&["anilist"])).unwrap();

    for d in [&mac, &ipad] {
        send(&phone, d);
        send(&mac, d);
        send(&ipad, d);
        d.store.sync_touch_peer("aaaa", d.secs()).unwrap();
        d.store.sync_touch_peer("bbbb", d.secs()).unwrap();
        d.store.sync_touch_peer("cccc", d.secs()).unwrap();
    }
    let received = |d: &Dev| {
        let ops = phone.store.ops_after(&VersionVector::new(), usize::MAX).unwrap();
        // Applying again is a no-op; the report of the first application is what matters, so
        // build the same list from the op.
        let _ = d;
        ops.into_iter()
            .filter_map(|op| match op.body {
                OpBody::Watch { anilist_id, episode, queued_for, .. } => {
                    Some(ReceivedWatch { device: op.device, anilist_id, episode, queued_for })
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let now = mac.secs();
    // AniList was queued by the phone: nobody fills it. Trakt: the lowest id, the Mac, does.
    let mac_gaps = mac
        .store
        .sync_tracker_gaps(&trackers(&["anilist", "trakt"]), &received(&mac), now)
        .unwrap();
    let ipad_gaps = ipad
        .store
        .sync_tracker_gaps(&trackers(&["anilist", "trakt"]), &received(&ipad), now)
        .unwrap();
    assert_eq!(mac_gaps, vec![("trakt".to_owned(), SHOW.get())]);
    assert!(ipad_gaps.is_empty(), "only one device sends it");

    // Once any device has pushed it, no one else does.
    mac.store.sync_tracker_mark("trakt", SHOW.get(), 3).unwrap();
    send(&mac, &ipad);
    send(&mac, &phone);
    assert!(
        mac.store
            .sync_tracker_gaps(&trackers(&["trakt"]), &received(&mac), now)
            .unwrap()
            .is_empty()
    );
    assert_eq!(ipad.store.sync_tracker_mark_for("trakt", SHOW.get()).unwrap(), Some(3));
}

#[test]
fn a_devices_cloud_travels_as_a_register_and_its_id_is_kept_while_sync_is_off() {
    let (a, b) = (dev("00000000000000a1", 1_000), dev("00000000000000b2", 1_000));
    a.store.sync_set_my_cloud(r#"{"folder":"google_drive"}"#).unwrap();
    let applied = send(&a, &b);
    assert!(applied.effects.is_empty(), "{:?}", applied.effects);
    assert_eq!(
        b.store.sync_reg_value(kinds::DEVICE_CLOUD, &a.id).unwrap().as_deref(),
        Some(r#"{"folder":"google_drive"}"#)
    );
    a.store.sync_disable().unwrap();
    assert_eq!(a.store.sync_device().unwrap(), None);
    assert_eq!(a.store.sync_device_kept().unwrap().as_deref(), Some(a.id.as_str()));
    a.store.sync_enable(&a.id).unwrap();

    // Bookkeeping clears by prefix, and only that prefix: `_` and `%` are literal.
    for k in ["folder..segno", "folder..cur.b2", "folder_x", "handoff.b2"] {
        a.store.sync_meta_set(k, "1").unwrap();
    }
    assert_eq!(a.store.sync_meta_clear("folder.").unwrap(), 2);
    assert_eq!(a.store.sync_meta_get("folder_x").unwrap().as_deref(), Some("1"));
    a.store.sync_meta_remove("handoff.b2").unwrap();
    assert_eq!(a.store.sync_meta_get("handoff.b2").unwrap(), None);
}

#[test]
fn continue_names_the_other_device_that_played_a_title_last_for_a_day() {
    let mac = dev("aaaa", 100_000);
    let phone = dev("bbbb", 100_000);
    for d in [&mac, &phone] {
        d.store.sync_device_add("aaaa", "Study Mac", "aa").unwrap();
        d.store.sync_device_add("bbbb", "Kim's phone", "bb").unwrap();
    }
    mac.watch(SHOW, "3", 300.0, 1440.0);
    mac.watch(OTHER, "1", 300.0, 1440.0);
    exchange(&mac, &phone);
    phone.advance(60);
    phone.watch(SHOW, "3", 724.0, 1440.0);
    exchange(&mac, &phone);

    let since = mac.secs() - 86_400;
    let ids = [SHOW.get(), OTHER.get()];
    // On the Mac, the phone played the show last; the other title the Mac played itself.
    let on_mac = mac.store.sync_last_devices(&ids, since).unwrap();
    assert_eq!(
        on_mac.into_iter().collect::<Vec<_>>(),
        [(SHOW.get(), "Kim's phone".to_owned())]
    );
    // On the phone, the Mac played the other title last.
    let on_phone = phone.store.sync_last_devices(&ids, since).unwrap();
    assert_eq!(
        on_phone.into_iter().collect::<Vec<_>>(),
        [(OTHER.get(), "Study Mac".to_owned())]
    );
    // Older than a day, it is the usual line again.
    let later = phone.secs() + 86_400 + 1;
    assert!(mac.store.sync_last_devices(&ids, later - 86_400).unwrap().is_empty());
    // With sync off nothing is named.
    mac.store.sync_disable().unwrap();
    assert!(mac.store.sync_last_devices(&ids, since).unwrap().is_empty());
}

/// What a newer version would write: an op of a kind this version does not know, numbered after
/// `seq - 1` of `device`'s.
fn future_op(device: &str, seq: u64, stamp: Stamp) -> serde_json::Value {
    serde_json::json!({
        "d": device,
        "s": seq,
        "t": stamp,
        "b": { "k": "chapter_read", "work": "w-1", "chapter": "10.5", "pages": [1, 2] },
    })
}

#[test]
fn a_batch_with_an_unknown_kind_applies_the_rest_and_relays_it() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    let c = dev("cccc", 10_000);
    a.finish(SHOW, "1");
    let [watch] =
        <[Op; 1]>::try_from(a.store.ops_after(&Default::default(), 10).unwrap()).unwrap();
    // The batch as a newer `aaaa` sends it: a known op, one it does not know, and a known one
    // numbered after it. Read from JSON, as both roads read it.
    let mut later = watch.clone();
    later.seq = 3;
    later.stamp += 2;
    let OpBody::Watch { episode, .. } = &mut later.body else { unreachable!() };
    *episode = "2".into();
    let unknown = future_op("aaaa", 2, watch.stamp + 1);
    let wire = serde_json::json!([watch, unknown, later]).to_string();
    let ops: Vec<Op> = serde_json::from_str(&wire).expect("an unknown kind fails nothing");
    assert!(!ops[1].body.is_known());

    let applied = b.store.apply_ops(ops).unwrap();
    assert_eq!((applied.ops, applied.unknown, applied.gaps), (2, 1, 0));
    assert_eq!(b.store.completed_episodes(SHOW).unwrap(), vec!["1".to_string(), "2".into()]);
    assert_eq!(b.store.version_vector().unwrap()["aaaa"], 3, "its number is held");

    // Relayed to a third device as it arrived.
    let relayed = b.store.ops_after(&c.store.version_vector().unwrap(), usize::MAX).unwrap();
    let held = relayed.iter().find(|op| op.device == "aaaa" && op.seq == 2).unwrap();
    assert_eq!(serde_json::to_value(held).unwrap(), unknown);
    let applied = c.store.apply_ops(relayed).unwrap();
    assert_eq!((applied.unknown, applied.gaps), (1, 0));
    assert_eq!(c.store.completed_episodes(SHOW).unwrap().len(), 2);
    assert_eq!(dump(&b.store), dump(&c.store));
}

#[test]
fn a_held_op_applies_once_this_version_knows_its_kind() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    let unknown: Op = serde_json::from_value(future_op("aaaa", 1, 1 << 40)).unwrap();
    assert_eq!(b.store.apply_ops(vec![unknown]).unwrap().unknown, 1);
    assert!(b.store.completed_episodes(SHOW).unwrap().is_empty());

    // An update teaches the device the kind: stood in for here by giving the held op a body
    // this version reads.
    a.finish(SHOW, "1");
    let watch = a.store.ops_after(&Default::default(), 1).unwrap().remove(0);
    let body = serde_json::to_string(&watch.body).unwrap();
    b.store
        .with_personal(|c| {
            c.execute(
                "UPDATE sync_op SET body = ?1 WHERE device = 'aaaa' AND seq = 1",
                [&body],
            )?;
            Ok(())
        })
        .unwrap();

    // The next batch, empty or not, applies it, once.
    assert_eq!(b.store.apply_ops(Vec::new()).unwrap().ops, 1);
    assert_eq!(b.store.completed_episodes(SHOW).unwrap(), vec!["1".to_string()]);
    assert_eq!(b.store.apply_ops(Vec::new()).unwrap().ops, 0);
    assert_eq!(b.store.sync_meta_get("held_unknown").unwrap(), None);
}

fn read_on(d: &Dev, work: &str, chapter: &str, page: u32, completed: bool) {
    d.advance(1);
    let mut e = crate::reading::ReadEvent::new(work, chapter, d.secs());
    e.page = Some(page);
    e.of_pages = Some(20);
    e.completed = completed;
    d.store.record_read(&e, Vec::new()).unwrap();
}

#[test]
fn reading_travels_and_read_again_with_its_undo_reaches_every_device() {
    let a = dev("a", 1_000);
    let b = dev("b", 1_000);
    let work = "a:900201";
    read_on(&a, work, "1", 20, true);
    read_on(&a, work, "2", 9, false);
    a.advance(1);
    a.store.adopt_read(work, "1", None, a.secs()).unwrap();
    let applied = send(&a, &b);
    assert_eq!(applied.works, [work]);
    let chapters = b.store.read_chapters(work).unwrap();
    assert_eq!(chapters.len(), 2);
    assert_eq!(chapters[1].last_page, Some(9));
    assert_eq!(chapters[1].device.as_deref(), Some("a"));

    b.advance(5);
    let reset = b.store.reset_read(work, b.secs()).unwrap();
    send(&b, &a);
    assert!(!a.store.chapter_is_read(work, "1").unwrap());
    a.advance(10);
    a.store.undo_read_reset(work, &reset, a.secs()).unwrap();
    send(&a, &b);
    assert!(b.store.chapter_is_read(work, "1").unwrap());

    b.advance(1);
    b.store.unread(work, &["2".into()]).unwrap();
    exchange(&a, &b);
    assert_eq!(a.store.read_chapters(work).unwrap().len(), 1);
    assert_eq!(dump(&a.store), dump(&b.store));
}

#[test]
fn an_unread_never_takes_back_a_read_it_did_not_see_and_each_device_keeps_a_note() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    let work = "a:900204";
    read_on(&a, work, "3", 20, true);
    exchange(&a, &b);
    // Offline: the tablet takes chapter 3 back while the phone reads it to the end again.
    a.advance(60);
    a.store.unread(work, &["3".into()]).unwrap();
    b.advance(120);
    read_on(&b, work, "3", 20, true);
    let to_a = send(&b, &a);
    let to_b = send(&a, &b);
    for d in [&a, &b] {
        assert!(d.store.chapter_is_read(work, "3").unwrap(), "{}", d.id);
    }
    assert_eq!((to_a.read_notes.len(), to_b.read_notes.len()), (1, 1));
    for n in to_a.read_notes.iter().chain(&to_b.read_notes) {
        assert_eq!((n.unread_device.as_str(), n.read_device.as_str()), ("aaaa", "bbbb"));
        assert_eq!((n.work.as_str(), n.chapter.as_str()), (work, "3"));
    }
    exchange(&a, &b);
    assert_eq!(a.store.read_notes(0).unwrap().len(), 1);
    let id = b.store.read_notes(0).unwrap()[0].id;
    b.store.dismiss_read_note(id).unwrap();
    assert!(b.store.read_notes(0).unwrap().is_empty());

    // A read made after seeing the unread is no surprise.
    a.advance(10);
    a.store.unread(work, &["3".into()]).unwrap();
    exchange(&a, &b);
    assert!(!b.store.chapter_is_read(work, "3").unwrap());
    b.advance(10);
    read_on(&b, work, "3", 20, true);
    let to_a = send(&b, &a);
    assert!(to_a.read_notes.is_empty());
}

#[test]
fn an_unread_under_a_watermark_travels_and_a_newer_watermark_counts_the_chapter_again() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    let work = "a:900205";
    a.store.adopt_read(work, "10", None, a.secs()).unwrap();
    exchange(&a, &b);
    b.advance(5);
    b.store.unread(work, &["4".into()]).unwrap();
    assert!(!b.store.chapter_is_read(work, "4").unwrap());
    exchange(&a, &b);
    let marks = a.store.read_marks(work).unwrap();
    assert!(!marks.is_read("4"));
    assert!(marks.is_read("5"));
    a.advance(5);
    a.store.adopt_read(work, "8", None, a.secs()).unwrap();
    exchange(&a, &b);
    assert!(b.store.chapter_is_read(work, "4").unwrap());
}

#[test]
fn reading_choices_travel_as_registers_and_local_works_stay_home() {
    let a = dev("a", 1_000);
    let b = dev("b", 1_000);
    let work = "a:900202";
    let local = "l:00112233445566778899aabbccddeeff";
    a.store.set_series_setting(work, "direction", Some("rtl"), true, 10).unwrap();
    a.store.set_series_setting(work, "zoom", Some("fit"), true, 10).unwrap();
    a.store.set_read_status(work, Some("reading"), 10).unwrap();
    a.store.set_bookmark(work, "v1/3", true, 10).unwrap();
    a.store.set_track_map(work, "mal", None, "hand", 10).unwrap();
    a.store.set_read_hidden(work, true).unwrap();
    let before = a.store.sync_op_count().unwrap();
    read_on(&a, local, "1", 2, false);
    a.store.set_bookmark(local, "1", true, 11).unwrap();
    assert_eq!(a.store.sync_op_count().unwrap(), before);

    send(&a, &b);
    let settings = b.store.series_settings(work).unwrap();
    assert_eq!(settings.len(), 1);
    assert_eq!(
        (settings[0].setting.as_str(), settings[0].value.as_str()),
        ("direction", "rtl")
    );
    assert_eq!(b.store.read_status(work).unwrap().0.as_deref(), Some("reading"));
    assert_eq!(b.store.bookmarks(work).unwrap(), ["v1/3"]);
    let map = b.store.track_map(work).unwrap();
    assert_eq!((map[0].tracker_id.as_deref(), map[0].origin.as_str()), (None, "hand"));
    assert!(b.store.read_chapters(local).unwrap().is_empty());
}

#[test]
fn reading_done_before_sync_becomes_the_first_ops() {
    let store = Store::open_in_memory().unwrap();
    let work = "a:900203";
    let mut e = crate::reading::ReadEvent::new(work, "4", 100);
    e.completed = true;
    store.record_read(&e, Vec::new()).unwrap();
    store.adopt_read(work, "3", None, 101).unwrap();
    store.set_series_setting(work, "mode", Some("strip"), true, 102).unwrap();
    store.record_read(&crate::reading::ReadEvent::new("l:ff", "1", 103), Vec::new()).unwrap();
    store.sync_enable("a").unwrap();
    assert_eq!(store.sync_op_count().unwrap(), 3);
    let other = dev("b", 1_000);
    let ops = store.ops_after(&other.store.version_vector().unwrap(), usize::MAX).unwrap();
    other.store.apply_ops(ops).unwrap();
    assert!(other.store.chapter_is_read(work, "2").unwrap());
    assert!(other.store.chapter_is_read(work, "4").unwrap());
    assert_eq!(other.store.series_settings(work).unwrap()[0].value, "strip");
}

#[test]
fn a_removed_device_starts_over_under_a_new_id_and_the_group_takes_it() {
    let a = dev("aaaa", 10_000);
    let b = dev("bbbb", 10_000);
    for d in [&a, &b] {
        d.store.sync_device_add(&d.id, "x", "00").unwrap();
    }
    a.finish(SHOW, "1");
    exchange(&a, &b);
    a.store.sync_device_remove("bbbb").unwrap();
    send(&a, &b);
    let gone = |s: &Store, id: &str| s.sync_peers().unwrap().iter().any(|p| p.0 == id && p.3);
    assert!(gone(&b.store, "bbbb"), "b read its own removal");
    // Joining again under the same id would stay removed for good: it starts over.
    b.store.sync_device_add("bbbb", "x", "00").unwrap();
    assert!(gone(&b.store, "bbbb") && gone(&a.store, "bbbb"));
    b.store.sync_new_device("bbbc").unwrap();
    assert_eq!(b.store.sync_device().unwrap(), None, "off until it joins");
    b.store.sync_enable("bbbc").unwrap();
    b.store.sync_device_add("bbbc", "x", "00").unwrap();
    b.store.set_show_setting(OTHER, "skip", Some("on"), b.secs() * 1000).unwrap();
    b.finish(SHOW, "2");
    send(&b, &a);
    assert!(!gone(&a.store, "bbbc"), "the new id is in the group");
    assert_eq!(a.store.events_for(SHOW, 10).unwrap().len(), 2, "its new watch arrived");
    assert_eq!(
        a.store.events_for(SHOW, 10).unwrap().iter().filter(|e| e.episode == "1").count(),
        1
    );
    // What it held stays.
    assert_eq!(b.store.events_for(SHOW, 10).unwrap().len(), 2);
}

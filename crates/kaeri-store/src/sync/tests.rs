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
    s.with_conn(|c| {
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
            .with_conn(|c| {
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

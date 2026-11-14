//! Reading's sync between devices, at random: every device, and a fresh one taking the logs
//! in other orders, ends with the same state.

use super::*;

/// Reading's merged state: what two devices that saw the same ops must agree on.
fn read_dump(s: &Store) -> String {
    s.with_personal(|c| {
        let mut out = String::new();
        let mut table = |name: &str, sql: &str| -> rusqlite::Result<()> {
            out.push_str(&format!("[{name}]\n"));
            let mut stmt = c.prepare(sql)?;
            let n = stmt.column_count();
            let rows = stmt.query_map([], |r| {
                (0..n)
                    .map(|i| r.get::<_, rusqlite::types::Value>(i).map(|v| format!("{v:?}")))
                    .collect::<rusqlite::Result<Vec<_>>>()
            })?;
            for row in rows {
                out.push_str(&row?.join("|"));
                out.push('\n');
            }
            Ok(())
        };
        table("event", "SELECT uid, stamp, work, chapter, page, completed, at FROM read_event ORDER BY uid")?;
        table("adopt", "SELECT uid, work, upto_chapter, upto_milli, stamp FROM read_adopt ORDER BY uid")?;
        table("reset", "SELECT uid, work, stamp, undone FROM read_reset ORDER BY uid")?;
        table("chapter", "SELECT work, chapter, read, read_at, last_page, stamp, device FROM read_chapter ORDER BY 1, 2")?;
        table("work", "SELECT work, last_chapter, stamp, reset_stamp, adopt_milli, hidden FROM read_work ORDER BY 1")?;
        table("status", "SELECT work, status, score FROM read_status ORDER BY 1")?;
        table("bookmark", "SELECT work, chapter FROM read_bookmark ORDER BY 1, 2")?;
        table("unread", "SELECT work, chapter, stamp, device FROM read_unread ORDER BY 1, 2")?;
        table("series", "SELECT work, setting, value, scope FROM read_series_setting ORDER BY 1, 2")?;
        table("regs", "SELECT kind, key, stamp, device, value FROM sync_reg WHERE kind LIKE 'read%' ORDER BY kind, key")?;
        Ok(out)
    })
    .unwrap()
}

/// What the apps show, per chapter 1..=6 of each work: read or not, and the work's Continue.
fn shown(s: &Store) -> String {
    let mut out = String::new();
    for work in ["a:900501", "a:900502"] {
        let marks = s.read_marks(work).unwrap();
        let read: Vec<bool> = (1..=6).map(|c| marks.is_read(&c.to_string())).collect();
        let w = s.read_work(work).unwrap().map(|w| (w.last_chapter, w.hidden));
        let status = s.read_status(work).unwrap().0;
        out.push_str(&format!(
            "{work} read={read:?} work={w:?} status={status:?} bm={:?}\n",
            s.bookmarks(work).unwrap()
        ));
    }
    out
}

/// Three devices read, mark, unmark, mark up to, read again and undo, set statuses and
/// bookmarks, offline and now and then meeting; then every device meets every other. All three
/// must end the same, and the same as a fresh device that took the logs in other orders.
#[test]
fn reading_ops_in_any_order_give_the_same_state() {
    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut rnd = move |n: u64| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed % n
    };
    let mut failures = Vec::new();
    for round in 0..40 {
        let devs = [dev("aaaa", 10_000), dev("bbbb", 10_000 + round), dev("cccc", 9_000)];
        let mut resets: Vec<(String, String)> = Vec::new();
        for _ in 0..50 {
            let i = rnd(3) as usize;
            let d = &devs[i];
            d.advance(rnd(30) as i64);
            let work = ["a:900501", "a:900502"][rnd(2) as usize];
            let ch = (1 + rnd(6)).to_string();
            match rnd(12) {
                0..=2 => read_on(d, work, &ch, 20, true),
                3 => read_on(d, work, &ch, rnd(19) as u32, false),
                4 => d.store.unread(work, std::slice::from_ref(&ch)).unwrap(),
                5 => d.store.adopt_read(work, &ch, None, d.secs()).unwrap(),
                6 => {
                    let r = d.store.reset_read(work, d.secs()).unwrap();
                    resets.push((work.to_owned(), r));
                }
                7 => {
                    if let Some((w, r)) = resets.get(rnd(resets.len().max(1) as u64) as usize) {
                        let _ = d.store.undo_read_reset(w, r, d.secs());
                    }
                }
                8 => {
                    let s = ["reading", "completed", "paused", "planning"][rnd(4) as usize];
                    d.store.set_read_status(work, Some(s), d.secs()).unwrap();
                }
                9 => d.store.set_bookmark(work, &ch, rnd(2) == 0, d.secs()).unwrap(),
                10 => d.store.set_read_hidden(work, rnd(2) == 0).unwrap(),
                _ => d
                    .store
                    .set_series_setting(
                        work,
                        "mode",
                        Some(["strip", "paged"][rnd(2) as usize]),
                        true,
                        d.secs(),
                    )
                    .unwrap(),
            }
            if rnd(5) == 0 {
                let (x, y) = (rnd(3) as usize, rnd(3) as usize);
                if x != y {
                    exchange(&devs[x], &devs[y]);
                }
            }
        }
        for _ in 0..2 {
            exchange(&devs[0], &devs[1]);
            exchange(&devs[1], &devs[2]);
            exchange(&devs[0], &devs[2]);
        }
        let dumps: Vec<String> = devs.iter().map(|d| read_dump(&d.store)).collect();
        let views: Vec<String> = devs.iter().map(|d| shown(&d.store)).collect();
        let logs: Vec<Vec<Op>> = devs
            .iter()
            .map(|d| d.store.ops_after(&Default::default(), usize::MAX).unwrap())
            .collect();
        let mut fresh_views = Vec::new();
        for order in [[0, 1, 2], [2, 1, 0], [1, 2, 0]] {
            let fresh = dev("zzzz", 10_000);
            let mut cursors = [0usize; 3];
            let step = 1 + rnd(7) as usize;
            while cursors.iter().zip(&logs).any(|(c, l)| *c < l.len()) {
                for &i in &order {
                    let end = (cursors[i] + step).min(logs[i].len());
                    fresh.store.apply_ops(logs[i][cursors[i]..end].to_vec()).unwrap();
                    cursors[i] = end;
                }
            }
            fresh_views.push(shown(&fresh.store));
        }
        let devs_agree =
            dumps.windows(2).all(|w| w[0] == w[1]) && views.windows(2).all(|w| w[0] == w[1]);
        let fresh_agree = fresh_views.iter().all(|v| *v == views[0]);
        if !devs_agree || !fresh_agree {
            let tables = |d: &str| -> Vec<(String, String)> {
                let mut out = Vec::new();
                for part in d.split('[').skip(1) {
                    let (name, rest) = part.split_once(']').unwrap_or((part, ""));
                    out.push((name.to_owned(), rest.to_owned()));
                }
                out
            };
            let (ta, tb, tc) = (tables(&dumps[0]), tables(&dumps[1]), tables(&dumps[2]));
            let differing: Vec<String> = ta
                .iter()
                .zip(tb.iter().zip(tc.iter()))
                .filter(|(a, (b, c))| a.1 != b.1 || a.1 != c.1)
                .map(|(a, _)| a.0.clone())
                .collect();
            let views_agree = views.windows(2).all(|w| w[0] == w[1]);
            failures.push(format!(
                "round {round}: devices agree={devs_agree} views agree={views_agree} fresh agree={fresh_agree} tables differ={differing:?}\n--- a\n{}\n--- b\n{}\n--- c\n{}\n--- fresh\n{}\n--- a dump\n{}\n--- b dump\n{}",
                views[0], views[1], views[2], fresh_views.join("\n"), dumps[0], dumps[1]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of 40 rounds diverged:\n{}",
        failures.len(),
        failures[..failures.len().min(2)].join("\n")
    );
}

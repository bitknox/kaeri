//! `kaeri --sync`: the outbox drained and the library pulled once, with what each
//! tracker made of it.

use anyhow::Result;
use kaeri_core::config::{Config, Paths};
use kaeri_engine::tracking;
use kaeri_net::HttpClient;
use kaeri_store::Store;

/// Drain and pull once, reporting what happened.
pub async fn sync_once(
    config: &Config,
    paths: &Paths,
    store: &Store,
    http: &HttpClient,
) -> Result<()> {
    let sync = tracking::Sync::build(config, paths, store, http);
    if sync.trackers.is_empty() {
        println!("no tracker enabled; set trackers.enabled in your config");
        return Ok(());
    }

    for tracker in &sync.trackers {
        let id = tracker.id();
        println!("── {id} ─────────────────────────────────────────────");
        println!("  authenticated  {}", tracker.is_authenticated());
        println!("  queued         {}", store.outbox_depth(Some(id)).unwrap_or(0));
        if !tracker.is_authenticated() {
            println!("  ▲ not signed in; run kaeri --login-url");
            continue;
        }

        let now = kaeri_store::now();
        match kaeri_track::drain(store, tracker.as_ref(), now).await {
            Ok(report) => println!(
                "  drain          sent {} · failed {} · remaining {}{}{}",
                report.sent,
                report.failed,
                report.remaining,
                match store.outbox_deferred(id).unwrap_or(0) {
                    0 => String::new(),
                    held => format!(" · {held} waiting for a match"),
                },
                if report.needs_reauth { " · NEEDS RE-AUTH" } else { "" }
            ),
            Err(e) => println!("  ✕ drain: {e}"),
        }

        let last = store.get_meta_i64(&format!("last_pull:{id}")).unwrap_or(None).unwrap_or(0);
        match kaeri_track::pull(store, tracker.as_ref(), now, last).await {
            Ok(report) => {
                let _ = store.set_meta_i64(&format!("last_pull:{id}"), now);
                println!(
                    "  pull           {} titles · queued {} · adopted {} · conflicts {}",
                    report.seen,
                    report.queued,
                    report.adopted,
                    report.conflicts.len()
                );
                for conflict in report.conflicts.iter().take(5) {
                    println!(
                        "    ▲ {} {}: mine {} / theirs {}",
                        store
                            .cached_title(conflict.anilist_id)
                            .unwrap_or(None)
                            .unwrap_or_else(|| conflict.anilist_id.get().to_string()),
                        conflict.field.label(),
                        conflict.local,
                        conflict.remote
                    );
                }
            }
            Err(e) => println!("  ✕ pull: {e}"),
        }
    }
    Ok(())
}

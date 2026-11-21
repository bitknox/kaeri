//! `kaeri --sync`: the outbox drained and the library pulled once, with what each
//! tracker made of it.

use anyhow::Result;
use kaeri_core::config::{Config, Paths};
use kaeri_engine::tracking;
use kaeri_net::HttpClient;
use kaeri_store::Store;

use super::login::login_command;

/// Drain and pull once, reporting what happened.
///
/// Every tracker has its turn, and then the run fails if any of them could not drain or pull,
/// or had its credential refused, so a scheduled `kaeri --sync` tells its scheduler. A tracker
/// waiting for a sign-in is not a failure: Kitsu and MangaUpdates are enabled out of the box
/// and queue nothing until they have credentials.
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

    let mut failed = Vec::new();
    for tracker in &sync.trackers {
        let id = tracker.id();
        println!("── {id} ─────────────────────────────────────────────");
        println!("  authenticated  {}", tracker.is_authenticated());
        println!("  queued         {}", store.outbox_depth(Some(id)).unwrap_or(0));
        if !tracker.is_authenticated() {
            match login_command(id) {
                Some(command) => println!("  ▲ not signed in; run {command}"),
                None => println!("  ▲ not signed in; the terminal has no sign-in for it"),
            }
            continue;
        }

        let mut synced = true;
        let now = kaeri_store::now();
        match kaeri_track::drain(store, tracker.as_ref(), now).await {
            Ok(report) => {
                println!(
                    "  drain          sent {} · failed {} · remaining {}{}{}",
                    report.sent,
                    report.failed,
                    report.remaining,
                    match store.outbox_deferred(id).unwrap_or(0) {
                        0 => String::new(),
                        held => format!(" · {held} waiting for a match"),
                    },
                    if report.needs_reauth { " · NEEDS RE-AUTH" } else { "" }
                );
                // The queue waits for a new sign-in, so nothing reached the tracker.
                synced &= !report.needs_reauth;
            }
            Err(e) => {
                println!("  ✕ drain: {e}");
                synced = false;
            }
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
            Err(e) => {
                println!("  ✕ pull: {e}");
                synced = false;
            }
        }
        if !synced {
            failed.push(id);
        }
    }
    if failed.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("sync failed for {}", failed.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_core::config::NetworkConfig;

    /// The default config with only `enabled` on and tokens kept in files, its paths in `dir`.
    fn setup(dir: &std::path::Path, enabled: &[&str]) -> (Config, Paths) {
        let paths = Paths {
            config_file: dir.join("config.toml"),
            data_dir: dir.join("data"),
            cache_dir: dir.join("cache"),
        };
        let mut config = Config::default();
        config.trackers.enabled = enabled.iter().map(|id| (*id).to_owned()).collect();
        config.trackers.token_storage = "file".into();
        (config, paths)
    }

    /// A month-long token for `tracker` in the file the trackers read theirs from.
    fn sign_in(config: &Config, paths: &Paths, tracker: &str) {
        let expires = kaeri_store::now() + 30 * 86_400;
        tracking::token_store(config, paths)
            .set_pair(tracker, "a-token", Some("a-refresh"), Some(expires))
            .expect("a token file");
    }

    /// A client that resolves no host name: a request for a tracker fails at once, before
    /// anything leaves the machine.
    fn offline(config: &Config) -> HttpClient {
        HttpClient::offline(&NetworkConfig { timeout_secs: 5, ..config.network.clone() })
            .expect("the offline client builds")
    }

    #[tokio::test]
    async fn a_tracker_that_cannot_pull_fails_the_sync() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let (config, paths) = setup(dir.path(), &["kitsu"]);
        sign_in(&config, &paths, "kitsu");
        let store = Store::open_in_memory().expect("a store");

        let ran = sync_once(&config, &paths, &store, &offline(&config)).await;
        let error = ran.expect_err("a pull that failed must fail the run");
        assert_eq!(error.to_string(), "sync failed for kitsu");
    }

    #[tokio::test]
    async fn a_refused_credential_fails_the_sync() {
        // MangaUpdates keeps no anime list, so its pull goes through without the network and
        // the refusal alone is what failed.
        let dir = tempfile::tempdir().expect("a temporary directory");
        let (config, paths) = setup(dir.path(), &["mangaupdates"]);
        sign_in(&config, &paths, "mangaupdates");
        let store = Store::open_in_memory().expect("a store");
        kaeri_track::sync::note_rejected(&store, "mangaupdates", kaeri_store::now())
            .expect("the hold is kept");

        let ran = sync_once(&config, &paths, &store, &offline(&config)).await;
        let error = ran.expect_err("a refused credential must fail the run");
        assert_eq!(error.to_string(), "sync failed for mangaupdates");
    }

    #[tokio::test]
    async fn a_sync_that_goes_through_or_waits_for_a_sign_in_succeeds() {
        let dir = tempfile::tempdir().expect("a temporary directory");
        let (config, paths) = setup(dir.path(), &["kitsu", "mangaupdates"]);
        sign_in(&config, &paths, "mangaupdates");
        let store = Store::open_in_memory().expect("a store");

        let ran = sync_once(&config, &paths, &store, &offline(&config)).await;
        assert!(ran.is_ok(), "{ran:?}");
    }
}

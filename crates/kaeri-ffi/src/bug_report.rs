//! "Report a bug" for the native apps: the same address the terminal builds.

use kaeri_model::bug_report::{
    BugReport, PlayerLogStats, issue_url_capped, issue_url_with, report_text as text,
};
use kaeri_model::crash::CrashRecord;

/// The address of a new GitHub issue with the bug-report form's diagnostics filled in, for the
/// app to open in the browser. Nothing in it names a source, an address or a file: see
/// [`kaeri_model::bug_report`]. At most 7,500 bytes.
#[uniffi::export]
pub fn bug_report_url(report: BugReport) -> String {
    let crashes = crashes_of(&report);
    issue_url_with(&report, &crashes)
}

/// [`bug_report_url`] cut to `max_bytes`: 900 for a code on a TV's screen.
#[uniffi::export]
pub fn bug_report_url_capped(report: BugReport, max_bytes: u32) -> String {
    let crashes = crashes_of(&report);
    issue_url_capped(&report, &crashes, max_bytes as usize)
}

/// The whole report as text, with the full crashes, for Copy report.
#[uniffi::export]
pub fn report_text(report: BugReport) -> String {
    let crashes = crashes_of(&report);
    text(&report, &crashes)
}

/// The player log as the file Save log writes: the report's summary, then the last playbacks,
/// newest first, redacted. For What is sent's Player log segment and for saving. Off the main
/// thread: it redacts up to a megabyte.
#[uniffi::export]
pub fn player_log(report: BugReport) -> String {
    kaeri_engine::player_log::text(&report)
}

/// What the Player log row says ("The last 3 playbacks · 412 KB", or nothing logged yet when
/// `playbacks` is 0) and the name Save log fills in, in local time. The size is worked out again
/// only after the log has changed.
#[uniffi::export]
pub fn player_log_stats() -> PlayerLogStats {
    kaeri_engine::player_log::stats()
}

/// Whether Player log starts ticked as Report opens: from a playback error (a [`BugError`] of
/// kind `playback`) or with a crash in the player among the report's crashes. Unticked from
/// anywhere else, Settings among them.
///
/// [`BugError`]: kaeri_model::bug_report::BugError
#[uniffi::export]
pub fn player_log_starts_ticked(report: BugReport) -> bool {
    let crashes = crashes_of(&report);
    kaeri_model::bug_report::player_log_ticked(&report, &crashes)
}

/// The saved crashes the report names; an id that is gone is left out.
fn crashes_of(report: &BugReport) -> Vec<CrashRecord> {
    report.crashes.iter().filter_map(|id| kaeri_engine::crash::armed_store()?.get(id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_apps_get_the_models_address() {
        let report = BugReport { app_version: "0.10.0".into(), ..BugReport::default() };
        assert_eq!(bug_report_url(report.clone()), kaeri_model::bug_report::issue_url(&report));
        assert!(
            bug_report_url(report).starts_with("https://github.com/bitknox/kaeri/issues/new?")
        );
    }
}

//! The second line of a chapter row in words, the same wherever chapters are listed: "Read · 4
//! days ago · Downloaded", "New · yesterday", "Page 8 of 19", "Not downloaded". Every row has
//! words there; the dot and the track beside them are the app's.

use jiff::tz::TimeZone;
use kaeri_model::{
    Msg, ReadingMsg,
    reading::{ChapterDownloadState, ChapterReadState, ChapterRow, VolumeGroup},
};

/// Where a row's chapter is read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    /// From a source, unless a download keeps it here.
    Source,
    /// A file on the device: a folder's or a finished download's.
    Device,
}

/// Every row's line, for rows read from `kept`.
pub fn fill(volumes: &mut [VolumeGroup], kept: Kept, now: i64, zone: &TimeZone) {
    for row in volumes.iter_mut().flat_map(|v| v.chapters.iter_mut()) {
        row.line = line(row, kept, now, zone);
    }
}

/// A row's line: its reading state, when that happened, then what this device keeps of it.
pub fn line(row: &ChapterRow, kept: Kept, now: i64, zone: &TimeZone) -> Vec<Msg> {
    let r = Msg::Reading;
    let mut parts = Vec::new();
    match row.state {
        ChapterReadState::Read => {
            parts.push(r(ReadingMsg::ReadingRowRead));
            parts.extend(row.read_at.map(|at| ago(at, now, zone)));
        }
        ChapterReadState::InProgress { page, of } => {
            // The state already counts from 1, as everything the engine sends.
            let page = i64::from(page);
            parts.push(r(match of.filter(|of| *of > 0) {
                Some(of) => ReadingMsg::ReadingRowPageOf { page, of: i64::from(of) },
                None => ReadingMsg::ReadingRowPage { page },
            }));
        }
        ChapterReadState::Unread if row.new_chapter => {
            parts.push(r(ReadingMsg::ReadingRowNew));
            parts.extend(row.published.map(|at| ago(at, now, zone)));
            if let Some(label) = &row.version_label {
                parts.push(Msg::Verbatim { text: label.clone() });
            }
            if row.versions > 1 {
                parts.push(r(ReadingMsg::ReadingRowVersions { n: i64::from(row.versions) }));
            }
        }
        ChapterReadState::Unread => {}
    }
    let unread_plain = parts.is_empty();
    match (&row.download, kept) {
        (Some(ChapterDownloadState::Done), _) => {
            parts.push(r(ReadingMsg::ReadingRowDownloaded));
        }
        (Some(ChapterDownloadState::Queued), _) => parts.push(r(ReadingMsg::ReadingRowQueued)),
        (Some(ChapterDownloadState::WaitsForWifi), _) => {
            parts.push(r(ReadingMsg::ReadingRowWaitsWifi));
        }
        (Some(ChapterDownloadState::Downloading { done, total }), _) => {
            parts.push(r(ReadingMsg::ReadingRowDownloading {
                done: i64::from(*done),
                total: i64::from(*total),
            }));
        }
        (Some(ChapterDownloadState::Failed { failure }), _) => {
            parts.push(r(ReadingMsg::ReadingRowFailed));
            parts.push(failure.msg());
        }
        // Read and part-read rows say nothing of a download that isn't there.
        (None, Kept::Source) if unread_plain => {
            parts.push(r(ReadingMsg::ReadingRowNotDownloaded));
        }
        (None, Kept::Device) if unread_plain => parts.push(match row.pages {
            Some(n) if n > 0 => r(ReadingMsg::ReadingRowPages { n: i64::from(n) }),
            _ => r(ReadingMsg::ReadingRowOnDevice),
        }),
        (None, _) => {}
    }
    if row.gone_from_source {
        parts.push(r(ReadingMsg::ReadingRowGoneFromSource));
    }
    parts
}

/// How long ago `at` was, by the person's calendar past the first hour: "12 min ago", "today",
/// "yesterday", "4 days ago".
pub fn ago(at: i64, now: i64, zone: &TimeZone) -> Msg {
    let seconds = now - at;
    let msg = if seconds < 60 {
        ReadingMsg::ReadingAgoNow
    } else if seconds < 3_600 {
        ReadingMsg::ReadingAgoMinutes { minutes: seconds / 60 }
    } else {
        match calendar_days(at, now, zone) {
            0 => ReadingMsg::ReadingAgoToday,
            1 => ReadingMsg::ReadingAgoYesterday,
            days @ 2..7 => ReadingMsg::ReadingAgoDays { days },
            days @ 7..60 => ReadingMsg::ReadingAgoWeeks { weeks: days / 7 },
            days @ 60..365 => ReadingMsg::ReadingAgoMonths { months: days / 30 },
            days => ReadingMsg::ReadingAgoYears { years: (days / 365).max(1) },
        }
    };
    Msg::Reading(msg)
}

/// Midnights between `at` and `now` where the person is.
fn calendar_days(at: i64, now: i64, zone: &TimeZone) -> i64 {
    let date =
        |t: i64| jiff::Timestamp::from_second(t).ok().map(|t| t.to_zoned(zone.clone()).date());
    match (date(at), date(now)) {
        (Some(at), Some(now)) => {
            now.since(at).map_or(0, |span| i64::from(span.get_days())).max(0)
        }
        _ => (now - at) / 86_400,
    }
}

#[cfg(test)]
mod tests {
    use kaeri_core::books::ChapterId;
    use kaeri_model::reading::ReadingFailure;

    use super::*;

    fn row(state: ChapterReadState) -> ChapterRow {
        ChapterRow {
            id: ChapterId::new("12"),
            label: "Chapter 12".into(),
            state,
            number: Some("12".into()),
            title: None,
            volume: None,
            published: None,
            pages: None,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
        }
    }

    fn ids(parts: &[Msg]) -> Vec<&'static str> {
        parts.iter().map(Msg::id).collect()
    }

    // Noon UTC on a fixed day, so no test depends on the machine's clock or zone.
    const NOW: i64 = 1_700_000_000 - 1_700_000_000 % 86_400 + 12 * 3_600;

    #[test]
    fn every_row_has_words_in_the_boards_order() {
        let utc = TimeZone::UTC;
        let words = |row: &ChapterRow, kept| ids(&line(row, kept, NOW, &utc));

        let plain = row(ChapterReadState::Unread);
        assert_eq!(words(&plain, Kept::Source), ["reading-row-not-downloaded"]);
        assert_eq!(words(&plain, Kept::Device), ["reading-row-on-device"]);
        let file = ChapterRow { pages: Some(19), ..plain.clone() };
        assert_eq!(words(&file, Kept::Device), ["reading-row-pages"]);

        let read = ChapterRow {
            read_at: Some(NOW - 4 * 86_400),
            download: Some(ChapterDownloadState::Done),
            ..row(ChapterReadState::Read)
        };
        assert_eq!(
            words(&read, Kept::Source),
            ["reading-row-read", "reading-ago-days", "reading-row-downloaded"]
        );
        let gone = ChapterRow { gone_from_source: true, ..read.clone() };
        assert_eq!(
            words(&gone, Kept::Source),
            [
                "reading-row-read",
                "reading-ago-days",
                "reading-row-downloaded",
                "reading-row-gone-from-source"
            ]
        );
        let read_here = ChapterRow { download: None, ..read };
        assert_eq!(words(&read_here, Kept::Source), ["reading-row-read", "reading-ago-days"]);

        let new = ChapterRow {
            new_chapter: true,
            published: Some(NOW - 86_400),
            version_label: Some("Group Example".into()),
            versions: 2,
            ..plain.clone()
        };
        assert_eq!(
            words(&new, Kept::Source),
            ["reading-row-new", "reading-ago-yesterday", "verbatim", "reading-row-versions"]
        );

        let page = row(ChapterReadState::InProgress { page: 8, of: Some(19) });
        assert_eq!(
            line(&page, Kept::Source, NOW, &utc),
            [Msg::Reading(ReadingMsg::ReadingRowPageOf { page: 8, of: 19 })],
            "the state's page, which counts from 1, as it is"
        );

        let wifi = ChapterRow { download: Some(ChapterDownloadState::WaitsForWifi), ..plain };
        assert_eq!(words(&wifi, Kept::Source), ["reading-row-waits-wifi"]);
        let failed = ChapterRow {
            download: Some(ChapterDownloadState::Failed { failure: ReadingFailure::Offline }),
            ..wifi
        };
        assert_eq!(
            words(&failed, Kept::Source),
            ["reading-row-failed", "reading-failed-offline"]
        );
    }

    #[test]
    fn ago_counts_minutes_then_the_calendar() {
        let utc = TimeZone::UTC;
        let since = |at| ago(at, NOW, &utc);
        let msg = Msg::Reading;
        assert_eq!(since(NOW + 30), msg(ReadingMsg::ReadingAgoNow), "a clock a little behind");
        assert_eq!(since(NOW - 120), msg(ReadingMsg::ReadingAgoMinutes { minutes: 2 }));
        assert_eq!(since(NOW - 3 * 3_600), msg(ReadingMsg::ReadingAgoToday));
        // Late last night is yesterday, though under a day ago.
        assert_eq!(since(NOW - 13 * 3_600), msg(ReadingMsg::ReadingAgoYesterday));
        assert_eq!(since(NOW - 6 * 86_400), msg(ReadingMsg::ReadingAgoDays { days: 6 }));
        assert_eq!(since(NOW - 15 * 86_400), msg(ReadingMsg::ReadingAgoWeeks { weeks: 2 }));
        assert_eq!(since(NOW - 90 * 86_400), msg(ReadingMsg::ReadingAgoMonths { months: 3 }));
        assert_eq!(since(NOW - 800 * 86_400), msg(ReadingMsg::ReadingAgoYears { years: 2 }));
        // Two hours ago is today in UTC, but before midnight where it is already 01:00 the
        // next day.
        let east = TimeZone::fixed(jiff::tz::offset(13));
        assert_eq!(since(NOW - 2 * 3_600), msg(ReadingMsg::ReadingAgoToday));
        assert_eq!(ago(NOW - 2 * 3_600, NOW, &east), msg(ReadingMsg::ReadingAgoYesterday));
    }
}

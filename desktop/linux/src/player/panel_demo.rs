//! For shots and the walkthrough: the board's made-up show, its sources and releases, and the
//! engine's side of a switch, scripted. The fixture engine has no releases, no stream health and
//! no switches to show.

use kaeri_core::traits::SourceCandidate;
use kaeri_model::{
    Msg,
    episodes::{EpisodeKind, EpisodeRow},
    hosted::{MenuEpisode, MenuSource},
    playback::{SourceSwitch, SwitchTo},
    sources::{SourceNote, SourceOption},
};

/// The show, as the board names it.
pub const SHOW: &str = "The Lamp Road"; // i18n-skip
/// The episode playing, and where in it.
pub const NOW: u32 = 7;
pub const AT: f64 = 724.0;
/// The source playing, as the panel names it.
pub const PLAYING: &str = "Example source";

const TITLES: [Option<&str>; 13] = [
    Some("First light"),
    Some("Ferry lights"),
    Some("The night market"),
    Some("Everything the river carried down from the mountain before the thaw"),
    Some("Paper wicks"),
    Some("Lanterns upstream"),
    Some("The lamp seller"),
    Some("A town without night"),
    Some("The long way home"),
    Some("Embers in the snow"),
    None,
    None,
    None,
];

const SYNOPSES: [&str; 10] = [
    "A lamp seller arrives in a valley town the night its last streetlight goes out, and offers to light the road for free.",
    "The ferry runs late. Mina counts the lamps on the far bank and finds one more than there should be.",
    "At the night market every stall sells light of a different colour. One sells darkness, by the hour.",
    "The thaw brings the river up to the doorsteps, and with it everything the mountain kept through the winter: boots, letters, a lamp still burning. Mina fishes the lamp out of the shallows and carries it home, and it will not go out. The lamp seller says it is not one of his, and then, after a long look, that it used to be.",
    "A festival day in town. The lamp seller teaches the children to fold paper wicks while the adults argue about the price of oil.",
    "Mina follows the paper lanterns upstream to see where they are launched from, and meets the one who launches them.",
    "The lamp seller refuses a sale for the first time. Mina wants to know why, and the answer takes them out past the last house on the road.",
    "The town tries a night without any lamps at all. Half of it is a party. The other half is something else.",
    "The way home is longer than the way out. Nobody on the road says why.",
    "Snow on the road, and the lamps begin to go out one by one from the far end.",
];

/// Unix seconds of `days` from today at 17:30 local time.
fn at_half_five(days: i32) -> Option<i64> {
    let now = gtk::glib::DateTime::now_local().ok()?;
    let day = now.add_days(days).ok()?;
    let at = gtk::glib::DateTime::from_local(
        day.year(),
        day.month(),
        day.day_of_month(),
        17,
        30,
        0.0,
    )
    .ok()?;
    Some(at.to_unix())
}

/// The board's 13 rows: 9 out, the 7th playing at 724 s, 8 dubbed. `stills` are the fixture's
/// own episode stills, given out in turn to the eight rows that have one.
pub fn table(stills: &[Option<String>]) -> Vec<EpisodeRow> {
    let stills: Vec<String> = stills.iter().flatten().cloned().collect();
    (1..=13u32)
        .map(|n| {
            let i = (n - 1) as usize;
            EpisodeRow {
                number: n.to_string(),
                title: TITLES[i].map(str::to_owned),
                thumbnail: (n <= 8 && !stills.is_empty())
                    .then(|| stills[i % stills.len()].clone()),
                description: SYNOPSES.get(i).map(|s| (*s).to_owned()),
                duration_secs: match n {
                    1..=8 => Some(1420),
                    9 => Some(1440),
                    _ => None,
                },
                watched: match n {
                    3 => 0.42,
                    7 => AT / 1420.0,
                    1 | 2 | 4 | 5 | 6 => 1.0,
                    _ => 0.0,
                },
                completed: matches!(n, 1 | 2 | 4 | 5 | 6),
                kind: match n {
                    5 => Some(EpisodeKind::Filler),
                    8 => Some(EpisodeKind::Mixed),
                    _ => None,
                },
                airs_at: match n {
                    10 => at_half_five(1),
                    11 => at_half_five(15),
                    12 => at_half_five(22),
                    13 => at_half_five(29),
                    _ => None,
                },
                delayed_from: if n == 11 { at_half_five(8) } else { None },
                source: (n == 9).then(|| "Home server".to_owned()), // i18n-skip
                past_progress: n > NOW,
                spoiler_ahead: n > NOW + 1,
                metadata_only: false,
                ..EpisodeRow::default()
            }
        })
        .collect()
}

/// The player's own episode menu for the table: the 7th playing.
pub fn menu(table: &[EpisodeRow]) -> Vec<MenuEpisode> {
    table
        .iter()
        .filter(|row| row.number.parse::<u32>().is_ok_and(|n| n <= 9))
        .map(|row| MenuEpisode {
            number: row.number.clone(),
            label: kaeri_i18n::t::episode_number(row.number.parse().unwrap_or(0)),
            title: row.title.clone(),
            watched: row.completed,
            filler: row.kind == Some(EpisodeKind::Filler),
            current: row.number == NOW.to_string(),
        })
        .collect()
}

/// The streams of the source playing, `current` the one on.
pub fn streams(current: &str) -> Vec<MenuSource> {
    [
        ("s2", "1080p · server 2", None), // i18n-skip
        ("s1", "1080p · server 1", Some("Failed the last 3 times · 12 min ago")), // i18n-skip
        ("s3", "720p · server 1", None),  // i18n-skip
    ]
    .into_iter()
    .map(|(id, label, health)| MenuSource {
        id: id.into(),
        label: label.into(),
        detail: None,
        current: id == current,
        health: health.map(str::to_owned),
    })
    .collect()
}

fn option(provider: &str, name: &str, covered: u32, note: Option<SourceNote>) -> SourceOption {
    SourceOption {
        provider_id: provider.into(),
        name: name.into(),
        covered,
        of: 9,
        pinned: false,
        note,
        releases: Vec::new(),
        pinned_release: None,
        first: (covered > 0).then_some(1),
        last: (covered > 0).then_some(covered),
        lacking: if covered == 8 { vec![9] } else { Vec::new() },
        lacking_since: None,
    }
}

/// The title's sources in the order they are tried.
pub fn options() -> Vec<SourceOption> {
    vec![
        option("example", PLAYING, 8, None),
        option("home", "Home server", 9, None), // i18n-skip
        option("index", "Example index", 9, None), // i18n-skip
        option("folder", "Media folder", 0, Some(SourceNote::Ambiguous)), // i18n-skip
        option("second", "Second example", 0, Some(SourceNote::Resting)), // i18n-skip
        option(
            "archive",
            "Archive example", // i18n-skip
            0,
            Some(SourceNote::Failed { reason: "HTTP 503 from api.archive.example".into() }), // i18n-skip
        ),
        option("old", "Old example", 0, Some(SourceNote::Off)), // i18n-skip
    ]
}

/// The episode's releases: one from Home server and five from Example index, in the index's
/// own order.
pub fn releases() -> Vec<SourceCandidate> {
    let release = |id: &str,
                   provider: &str,
                   name: &str,
                   title: &str,
                   quality: u32,
                   size: &str,
                   label: Option<&str>| SourceCandidate {
        id: id.into(),
        provider_id: provider.into(),
        provider_name: name.into(),
        title: title.into(),
        quality: Some(quality),
        label: label.map(str::to_owned),
        size: Some(size.into()),
        dual_audio: false,
        dubbed: false,
        auto_pick: false,
    };
    let index = "Example index"; // i18n-skip
    vec![
        release("h7", "home", "Home server", "The Lamp Road 07", 1080, "1.3 GiB", None), // i18n-skip
        SourceCandidate {
            auto_pick: true,
            ..release(
                "r1",
                "index",
                index,
                "The Lamp Road 07 1080p",
                1080,
                "1.4 GiB",
                Some("lantern-archive"),
            ) // i18n-skip
        },
        SourceCandidate {
            dual_audio: true,
            ..release(
                "r2",
                "index",
                index,
                "The Lamp Road 07 1080p multi",
                1080,
                "2.1 GiB",
                None,
            ) // i18n-skip
        },
        SourceCandidate {
            dubbed: true,
            ..release("r3", "index", index, "The Lamp Road 07 dub 1080p", 1080, "1.5 GiB", None) // i18n-skip
        },
        release(
            "r4",
            "index",
            index,
            "The Lamp Road 07 720p",
            720,
            "620 MiB",
            Some("night-index"),
        ), // i18n-skip
        release(
            "r5",
            "index",
            index,
            "The Lamp Road 01 to 13 1080p",
            1080,
            "18.2 GiB",
            Some("batch"),
        ), // i18n-skip
    ]
}

/// How a scripted switch goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ending {
    Lands,
    /// It fails while the new source is still being found, before anything loads.
    FailsFinding,
    /// It fails once the new stream was loading.
    FailsLoading,
}

/// How a pick goes on its `attempt`th try (from 1): server 1 and the multi-audio release fail
/// the first time and land on Try again; the 720p release fails before it loads, every time.
pub fn ending(key: &str, attempt: u32) -> Ending {
    match key {
        "stream/s1" | "release/index/r2" if attempt == 1 => Ending::FailsLoading,
        "release/index/r4" => Ending::FailsFinding,
        _ => Ending::Lands,
    }
}

/// The engine's states for a switch to `to`, from `at`, each after its delay in ms. The stream
/// switches skip Finding: the source playing has them already.
pub fn script(to: &SwitchTo, at: f64, ending: Ending) -> Vec<(u64, SourceSwitch)> {
    let finding = to.stream.is_none();
    let mut steps = Vec::new();
    let mut ms = 120;
    if finding {
        steps.push((ms, SourceSwitch::Finding { to: to.clone() }));
        ms += 1500;
    }
    let failed = || SourceSwitch::Failed {
        to: to.clone(),
        at,
        text: Msg::PlayerPanelSwitchFailed { source: to.name.clone(), current: PLAYING.into() },
        reason: None,
    };
    if ending == Ending::FailsFinding {
        steps.push((ms + 2600, failed()));
        return steps;
    }
    steps.push((ms, SourceSwitch::Loading { to: to.clone(), at }));
    ms += 2200;
    steps.push((
        ms,
        match ending {
            Ending::Lands => SourceSwitch::Landed { to: to.clone(), at },
            _ => failed(),
        },
    ));
    steps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_board_show_has_nine_out_and_the_seventh_playing() {
        let rows = table(&[Some("still.example/1.jpg".into())]);
        assert_eq!(rows.len(), 13);
        assert_eq!(rows.iter().filter(|r| r.airs_at.is_none()).count(), 9);
        let menu = menu(&rows);
        assert_eq!(menu.iter().find(|e| e.current).map(|e| e.number.as_str()), Some("7"));
        assert!(rows[9].thumbnail.is_none() && rows[7].thumbnail.is_some());
    }

    #[test]
    fn a_failure_before_loading_never_loads() {
        let to = SwitchTo {
            provider_id: "index".into(),
            name: "Example index".into(),
            release: Some("r4".into()),
            stream: None,
        };
        let steps = script(&to, AT, ending("release/index/r4", 1));
        assert!(steps.iter().all(|(_, s)| !matches!(s, SourceSwitch::Loading { .. })));
        assert!(matches!(steps.last(), Some((_, SourceSwitch::Failed { .. }))));
        let again = script(&to, AT, ending("release/index/r2", 2));
        assert!(matches!(again.last(), Some((_, SourceSwitch::Landed { .. }))));
    }
}

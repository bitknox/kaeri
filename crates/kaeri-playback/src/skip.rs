//! Opening, ending and recap skip times, from aniskip and from the file's own chapters.
//!
//! Verified during planning: `api.aniskip.com/v2` returns real intervals — MAL 52991 ep 1
//! gives OP `3.2–93.2s` and ED `1417–1507s`. It is keyed on **MAL id**, not AniList, which is
//! one of the two places the mapping layer earns its keep (the other being release numbering).
//!
//! A title with no MAL id simply has no skip data. The prompt is then absent rather than
//! broken — this is decoration, and an episode plays perfectly well without it.

use serde::Deserialize;

/// Which segment a skip interval covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkipKind {
    Opening,
    Ending,
    /// The "previously on" at the start of an episode. Skipped by itself only under its own
    /// setting, off by default.
    Recap,
}

impl SkipKind {
    /// Every kind, in the order a panel lists them.
    pub const ALL: [Self; 3] = [Self::Opening, Self::Ending, Self::Recap];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::Ending => "ending",
            Self::Recap => "recap",
        }
    }

    /// The type aniskip files these under. A submission is always one of these three: the
    /// mixed types (credits over story) are read, never sent.
    pub const fn api_type(self) -> &'static str {
        match self {
            Self::Opening => "op",
            Self::Ending => "ed",
            Self::Recap => "recap",
        }
    }

    /// The kind aniskip's `skipType` names, the mixed ones included.
    pub fn from_api_type(skip_type: &str) -> Option<Self> {
        match skip_type {
            "op" | "mixed-op" => Some(Self::Opening),
            "ed" | "mixed-ed" => Some(Self::Ending),
            "recap" => Some(Self::Recap),
            _ => None,
        }
    }
}

/// Where a skip interval came from. Per kind, the first of these that has one wins: see
/// [`merge`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SkipOrigin {
    /// The file's own chapters, cut against this exact encode.
    File,
    /// Marked by the person watching, on this device.
    Yours,
    /// AniSkip's shared list.
    AniSkip,
}

/// Skip intervals from the file's own chapter markers.
///
/// Release groups routinely author `OP`/`ED`/`Intro`/`Outro` chapters into the mkv, and
/// those were cut against *this exact encode* — where aniskip's community times were taken
/// against whichever rip the submitter had. When a file declares chapters, they outrank
/// the API. An interval runs to the next chapter, or for the last one to the end of the file,
/// which is how a release that ends on its ED is cut. While the length is not known yet, a
/// final chapter is left alone rather than guessed at.
pub fn from_chapters(chapters: &[(String, f64)], duration: Option<f64>) -> Vec<SkipInterval> {
    let classify = |title: &str| -> Option<SkipKind> {
        let t = title.trim().to_ascii_lowercase();
        // Terse, conventional names only. "Part A", "Avant" and friends stay unclassified
        // — mislabelling a cold open as an opening would auto-skip story.
        if t == "op" || t == "opening" || t == "intro" || t.starts_with("ncop") {
            Some(SkipKind::Opening)
        } else if t == "ed" || t == "ending" || t == "outro" || t.starts_with("nced") {
            Some(SkipKind::Ending)
        } else if t == "recap" || t == "previously" || t == "previously on" {
            Some(SkipKind::Recap)
        } else {
            None
        }
    };

    chapters
        .iter()
        .enumerate()
        .filter_map(|(i, (title, start))| {
            let kind = classify(title)?;
            let end = chapters.get(i + 1).map(|(_, t)| *t).or(duration)?;
            (end > *start).then_some(SkipInterval {
                kind,
                start: *start,
                end,
                origin: SkipOrigin::File,
            })
        })
        .collect()
}

/// Chapter markers describing a set of skip intervals — the inverse of [`from_chapters`].
///
/// A stream that carries no chapters of its own gives the viewer a featureless seek bar, even
/// when we know exactly where the opening and ending sit. These markers put that knowledge on
/// the bar: where the opening begins and ends, and where the ending does.
///
/// Chapters are boundaries rather than spans, so each interval contributes two — its start,
/// named for the segment, and its end, named for whatever the segment gives way to. A run that
/// does not begin on a marker gets one at zero, so the first segment is named rather than
/// nameless.
pub fn to_chapters(skips: &[SkipInterval]) -> Vec<(String, f64)> {
    let mut marks: Vec<(String, f64)> = Vec::new();
    for skip in skips.iter().filter(|s| s.duration() > 0.0) {
        let (opens, closes) = match skip.kind {
            // What follows an opening is the episode; what follows an ending is the next
            // episode's preview, which is the one thing that reliably sits there.
            SkipKind::Opening => ("Opening", "Episode"),
            SkipKind::Ending => ("Ending", "Preview"),
            SkipKind::Recap => ("Recap", "Episode"),
        };
        marks.push((opens.to_owned(), skip.start));
        marks.push((closes.to_owned(), skip.end));
    }

    marks.sort_by(|a, b| a.1.total_cmp(&b.1));
    // Two markers a fraction apart are one boundary as far as a seek bar is concerned, and mpv
    // would render them on top of each other.
    marks.dedup_by(|b, a| (b.1 - a.1).abs() < 1.0);

    if marks.first().is_some_and(|(_, at)| *at > 1.0) {
        marks.insert(0, ("Episode".to_owned(), 0.0));
    }
    marks
}

#[cfg(test)]
mod chapter_tests {
    use super::*;

    fn chapters(names: &[(&str, f64)]) -> Vec<(String, f64)> {
        names.iter().map(|(n, t)| (n.to_string(), *t)).collect()
    }

    #[test]
    fn conventional_chapter_names_become_skips() {
        let skips = from_chapters(
            &chapters(&[
                ("Intro", 0.0),
                ("Part A", 90.0),
                ("Part B", 700.0),
                ("Outro", 1320.0),
                ("Preview", 1410.0),
            ]),
            Some(1440.0),
        );
        assert_eq!(
            skips,
            vec![
                SkipInterval {
                    kind: SkipKind::Opening,
                    start: 0.0,
                    end: 90.0,
                    origin: SkipOrigin::File
                },
                SkipInterval {
                    kind: SkipKind::Ending,
                    start: 1320.0,
                    end: 1410.0,
                    origin: SkipOrigin::File
                },
            ]
        );
    }

    #[test]
    fn ambiguous_and_unbounded_chapters_are_left_alone() {
        // "Avant" is a cold open — skipping it would skip story. And a final chapter has
        // no bound until the file's length is known, so it is not guessed at.
        let skips =
            from_chapters(&chapters(&[("Avant", 0.0), ("Part A", 60.0), ("ED", 1300.0)]), None);
        assert!(skips.is_empty(), "got {skips:?}");
    }

    #[test]
    fn an_ending_that_closes_the_file_runs_to_its_end() {
        // Plenty of releases stop on the ED, with no preview chapter after it.
        let skips = from_chapters(&chapters(&[("Part A", 0.0), ("ED", 1300.0)]), Some(1420.0));
        assert_eq!(
            skips,
            vec![SkipInterval {
                kind: SkipKind::Ending,
                start: 1300.0,
                end: 1420.0,
                origin: SkipOrigin::File
            }]
        );
        // A chapter list that claims to start past the end is not believed.
        assert!(from_chapters(&chapters(&[("ED", 1500.0)]), Some(1420.0)).is_empty());
    }

    #[test]
    fn recap_chapters_are_recaps() {
        for name in ["Recap", "Previously", "previously on"] {
            let skips = from_chapters(&chapters(&[(name, 0.0), ("Part A", 45.0)]), None);
            assert_eq!(skips[0].kind, SkipKind::Recap, "{name}");
            assert_eq!(skips[0].origin, SkipOrigin::File);
        }
        // And they round trip as the others do.
        let recap = vec![SkipInterval {
            kind: SkipKind::Recap,
            start: 0.0,
            end: 45.0,
            origin: SkipOrigin::File,
        }];
        assert_eq!(from_chapters(&to_chapters(&recap), Some(1440.0)), recap);
    }

    #[test]
    fn ncop_variants_count_as_openings() {
        let skips = from_chapters(&chapters(&[("NCOP1", 10.0), ("Part A", 100.0)]), None);
        assert_eq!(skips[0].kind, SkipKind::Opening);
    }

    #[test]
    fn skips_become_markers_a_seek_bar_can_show() {
        let marks = to_chapters(&[
            SkipInterval {
                kind: SkipKind::Opening,
                start: 3.2,
                end: 93.2,
                origin: SkipOrigin::File,
            },
            SkipInterval {
                kind: SkipKind::Ending,
                start: 1417.0,
                end: 1507.0,
                origin: SkipOrigin::File,
            },
        ]);
        assert_eq!(
            marks,
            chapters(&[
                // A cold open runs before the opening, so it is named rather than left blank.
                ("Episode", 0.0),
                ("Opening", 3.2),
                ("Episode", 93.2),
                ("Ending", 1417.0),
                ("Preview", 1507.0),
            ])
        );
    }

    /// The names are the ones [`from_chapters`] reads, so what we write is what we would have
    /// understood had the release group written it.
    #[test]
    fn the_markers_round_trip_back_to_the_same_skips() {
        let skips = vec![
            SkipInterval {
                kind: SkipKind::Opening,
                start: 0.0,
                end: 90.0,
                origin: SkipOrigin::File,
            },
            SkipInterval {
                kind: SkipKind::Ending,
                start: 1320.0,
                end: 1410.0,
                origin: SkipOrigin::File,
            },
        ];
        assert_eq!(from_chapters(&to_chapters(&skips), Some(1440.0)), skips);
    }

    #[test]
    fn an_opening_at_the_very_start_gets_no_marker_before_it() {
        let marks = to_chapters(&[SkipInterval {
            kind: SkipKind::Opening,
            start: 0.0,
            end: 90.0,
            origin: SkipOrigin::File,
        }]);
        assert_eq!(marks, chapters(&[("Opening", 0.0), ("Episode", 90.0)]));
    }

    #[test]
    fn a_segment_with_no_length_is_not_a_boundary() {
        assert!(
            to_chapters(&[SkipInterval {
                kind: SkipKind::Opening,
                start: 90.0,
                end: 90.0,
                origin: SkipOrigin::File
            }])
            .is_empty()
        );
    }
}

/// A skippable segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkipInterval {
    pub kind: SkipKind,
    pub start: f64,
    pub end: f64,
    pub origin: SkipOrigin,
}

impl SkipInterval {
    /// Whether the playhead is inside this segment.
    ///
    /// The start is nudged forward slightly: offering to skip an opening in its first instant
    /// is jarring, and a viewer who just seeked there probably meant to be there.
    pub fn contains(&self, position: f64) -> bool {
        position >= self.start + 0.5 && position < self.end
    }

    pub fn duration(&self) -> f64 {
        (self.end - self.start).max(0.0)
    }
}

/// The skips that win, per kind, from each origin's: the file's chapters, then the person's own
/// marks, then aniskip's. A kind one origin names is taken from it whole, so an opening from
/// the file never sits beside aniskip's opening for another cut.
pub fn merge(
    file: &[SkipInterval],
    yours: &[SkipInterval],
    aniskip: &[SkipInterval],
) -> Vec<SkipInterval> {
    let mut merged: Vec<SkipInterval> = Vec::new();
    for kind in SkipKind::ALL {
        let named = |from: &[SkipInterval]| from.iter().any(|s| s.kind == kind);
        let from = [file, yours, aniskip].into_iter().find(|from| named(from)).unwrap_or(&[]);
        merged.extend(from.iter().filter(|s| s.kind == kind).copied());
    }
    merged.sort_by(|a, b| a.start.total_cmp(&b.start));
    merged
}

/// How far an episode's length may be from the length a mark was made on for the mark to be
/// used, in seconds: AniSkip's own tolerance for the same question.
pub const MARK_LENGTH_TOLERANCE: f64 = 20.0;

/// A segment the viewer marked, with the episode's length when they marked it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkedSkip {
    pub interval: SkipInterval,
    /// The episode's length when marked, in seconds.
    pub length: f64,
}

impl MarkedSkip {
    /// A mark of `kind` from `start` to `end` on an episode `length` seconds long.
    pub const fn new(kind: SkipKind, start: f64, end: f64, length: f64) -> Self {
        Self { interval: SkipInterval { kind, start, end, origin: SkipOrigin::Yours }, length }
    }

    /// Whether the mark is used on a file `duration` seconds long: one within
    /// [`MARK_LENGTH_TOLERANCE`] of the length it was made on. Not before the length is known,
    /// as a mark made on another cut would skip the wrong stretch.
    pub fn fits(&self, duration: Option<f64>) -> bool {
        duration.is_some_and(|d| (d - self.length).abs() <= MARK_LENGTH_TOLERANCE)
    }
}

/// The marks in `marks` that fit a file `duration` seconds long, as segments.
pub fn fitting(marks: &[MarkedSkip], duration: Option<f64>) -> Vec<SkipInterval> {
    marks.iter().filter(|m| m.fits(duration)).map(|m| m.interval).collect()
}

/// Query URL for one episode's skip times.
///
/// `episode_length` is required by the API and behaves like a filter: a real length
/// returns only submissions within a tight tolerance of it, while `0` disables the
/// filter and returns everything — which is why `0` is the right first ask. It has one
/// failure mode, measured against the live service: an interval that runs to the exact
/// end of the episode makes the server answer 500 under `episodeLength=0`, so callers
/// need a real length to fall back to.
pub fn query_url(mal_id: u32, episode: u32, episode_length: u32) -> String {
    query_url_on(API, mal_id, episode, episode_length)
}

/// Where aniskip answers.
pub const API: &str = "https://api.aniskip.com";

/// [`query_url`], asked of `origin` rather than aniskip: a test's server.
pub fn query_url_on(origin: &str, mal_id: u32, episode: u32, episode_length: u32) -> String {
    let types: String = READ_TYPES.iter().map(|t| format!("types={t}&")).collect();
    format!("{origin}/v2/skip-times/{mal_id}/{episode}?{types}episodeLength={episode_length}")
}

/// The types asked for: the three kinds, and the mixed openings and endings, which are read as
/// openings and endings.
const READ_TYPES: [&str; 5] = ["op", "ed", "mixed-op", "mixed-ed", "recap"];

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    found: bool,
    #[serde(default)]
    results: Vec<Result_>,
}

#[derive(Deserialize)]
struct Result_ {
    interval: Interval,
    #[serde(rename = "skipType")]
    skip_type: String,
    #[serde(rename = "skipId", default)]
    skip_id: String,
    #[serde(rename = "episodeLength", default)]
    episode_length: f64,
    #[serde(default)]
    votes: i64,
}

/// One of aniskip's times, as it gave it: the interval, and what a vote on it needs.
#[derive(Debug, Clone, PartialEq)]
pub struct AniSkipTime {
    pub interval: SkipInterval,
    /// The time's id, which a vote names. Empty when the answer had none.
    pub skip_id: String,
    /// The length of the episode the time was taken against, in seconds; `0` when not said.
    pub episode_length: f64,
    /// Its votes, where the answer says; aniskip already left out those voted down to -2.
    pub votes: i64,
}

#[derive(Deserialize)]
struct Interval {
    #[serde(rename = "startTime")]
    start_time: f64,
    #[serde(rename = "endTime")]
    end_time: f64,
}

/// Parse an aniskip response.
///
/// Anything unexpected yields no intervals rather than an error: skip data is optional, and a
/// changed API shape should cost the prompt, not the episode.
pub fn parse(payload: &str) -> Vec<SkipInterval> {
    parse_times(payload).into_iter().map(|time| time.interval).collect()
}

/// [`parse`], keeping each time's id, votes and the length it was taken against.
pub fn parse_times(payload: &str) -> Vec<AniSkipTime> {
    let Ok(response) = serde_json::from_str::<Response>(payload) else {
        return Vec::new();
    };
    if !response.found {
        return Vec::new();
    }

    response
        .results
        .into_iter()
        .filter_map(|result| {
            let kind = SkipKind::from_api_type(&result.skip_type)?;
            let interval = SkipInterval {
                kind,
                start: result.interval.start_time,
                end: result.interval.end_time,
                origin: SkipOrigin::AniSkip,
            };
            // A zero-length or backwards interval would produce a prompt that skips nowhere.
            (interval.duration() > 1.0).then_some(AniSkipTime {
                interval,
                skip_id: result.skip_id,
                episode_length: result.episode_length,
                votes: result.votes,
            })
        })
        .collect()
}

/// The segment the playhead is currently inside, if any.
pub fn active(intervals: &[SkipInterval], position: f64) -> Option<&SkipInterval> {
    intervals.iter().find(|i| i.contains(position))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shape captured from api.aniskip.com during planning (MAL 52991, ep 1).
    const PAYLOAD: &str = r#"{
      "found": true,
      "results": [
        {"interval":{"startTime":3.221,"endTime":93.221},
         "skipType":"op","skipId":"c2cacbe5","episodeLength":1559.949},
        {"interval":{"startTime":1417.135,"endTime":1507.135},
         "skipType":"ed","skipId":"92965d0e","episodeLength":1510.102}
      ],
      "message":"Successfully found skip times","statusCode":200
    }"#;

    #[test]
    fn the_query_is_keyed_on_mal_id_not_anilist() {
        // Getting this wrong returns skip times for an unrelated show.
        let url = query_url(52_991, 1, 0);
        assert!(url.contains("/skip-times/52991/1"));
        assert!(url.contains("types=op"));
        assert!(url.contains("types=ed"));
        assert!(url.contains("episodeLength=0"));
        // The fallback ask carries the real runtime, verbatim.
        assert!(query_url(52_991, 1, 1440).contains("episodeLength=1440"));
    }

    #[test]
    fn a_real_payload_yields_both_intervals() {
        let intervals = parse(PAYLOAD);
        assert_eq!(intervals.len(), 2);

        let opening = intervals.iter().find(|i| i.kind == SkipKind::Opening).unwrap();
        assert!((opening.start - 3.221).abs() < 1e-6);
        assert!((opening.end - 93.221).abs() < 1e-6);
        assert!((opening.duration() - 90.0).abs() < 0.01);

        let ending = intervals.iter().find(|i| i.kind == SkipKind::Ending).unwrap();
        assert!((ending.start - 1417.135).abs() < 1e-6);
    }

    #[test]
    fn a_not_found_response_yields_nothing() {
        // The common case — most titles have no submitted skip times.
        let payload =
            r#"{"found":false,"results":[],"message":"No skip times found","statusCode":404}"#;
        assert!(parse(payload).is_empty());
    }

    #[test]
    fn a_changed_or_broken_api_costs_the_prompt_not_the_episode() {
        for payload in ["", "not json", "{}", r#"{"found":true}"#, "null", "[]"] {
            assert!(parse(payload).is_empty(), "unexpected result from {payload:?}");
        }
    }

    #[test]
    fn degenerate_intervals_are_discarded() {
        // A zero-length interval would produce a prompt that skips nowhere.
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":10.0,"endTime":10.0},"skipType":"op"},
            {"interval":{"startTime":50.0,"endTime":20.0},"skipType":"ed"}
        ]}"#;
        assert!(parse(payload).is_empty());
    }

    #[test]
    fn unknown_skip_types_are_ignored() {
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":0.0,"endTime":90.0},"skipType":"preview"}
        ]}"#;
        assert!(parse(payload).is_empty());
    }

    #[test]
    fn recaps_are_read_with_their_id_and_the_length_they_were_taken_against() {
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":0.0,"endTime":42.5},"skipType":"recap",
             "skipId":"6f1c2a3e-0000-4000-8000-000000000001","episodeLength":1440.2},
            {"interval":{"startTime":42.5,"endTime":132.5},"skipType":"op",
             "skipId":"6f1c2a3e-0000-4000-8000-000000000002","episodeLength":1440.2,"votes":3}
        ]}"#;
        let times = parse_times(payload);
        assert_eq!(times.len(), 2);
        assert_eq!(times[0].interval.kind, SkipKind::Recap);
        assert_eq!(times[0].interval.origin, SkipOrigin::AniSkip);
        assert_eq!(times[0].skip_id, "6f1c2a3e-0000-4000-8000-000000000001");
        assert!((times[0].episode_length - 1440.2).abs() < 1e-9);
        assert_eq!((times[0].votes, times[1].votes), (0, 3), "votes default to none said");
    }

    #[test]
    fn every_kind_and_the_mixed_ones_are_asked_for() {
        let url = query_url(52_991, 1, 0);
        for t in
            ["types=op&", "types=ed&", "types=mixed-op&", "types=mixed-ed&", "types=recap&"]
        {
            assert!(url.contains(t), "{url} lacks {t}");
        }
        for kind in SkipKind::ALL {
            assert_eq!(SkipKind::from_api_type(kind.api_type()), Some(kind));
        }
        assert_eq!(SkipKind::from_api_type("mixed-ed"), Some(SkipKind::Ending));
    }

    fn at(kind: SkipKind, start: f64, end: f64, origin: SkipOrigin) -> SkipInterval {
        SkipInterval { kind, start, end, origin }
    }

    #[test]
    fn per_kind_the_file_wins_then_your_marks_then_aniskip() {
        use SkipKind::{Ending, Opening, Recap};
        use SkipOrigin::{AniSkip, File, Yours};
        let file = [at(Ending, 1300.0, 1390.0, File)];
        let yours = [at(Opening, 40.0, 130.0, Yours), at(Ending, 1310.0, 1400.0, Yours)];
        let aniskip = [
            at(Recap, 0.0, 40.0, AniSkip),
            at(Opening, 42.0, 132.0, AniSkip),
            at(Ending, 1312.0, 1402.0, AniSkip),
        ];
        assert_eq!(
            merge(&file, &yours, &aniskip),
            [
                at(Recap, 0.0, 40.0, AniSkip),
                at(Opening, 40.0, 130.0, Yours),
                at(Ending, 1300.0, 1390.0, File),
            ]
        );
        // With nothing of its own, each origin stands alone.
        assert_eq!(merge(&[], &[], &aniskip), aniskip);
        assert!(merge(&[], &[], &[]).is_empty());
    }

    #[test]
    fn a_mark_is_used_only_on_a_file_of_about_its_length() {
        let mark = MarkedSkip::new(SkipKind::Opening, 40.0, 130.0, 1420.0);
        assert!(!mark.fits(None), "not before the length is known");
        assert!(mark.fits(Some(1420.0)));
        assert!(mark.fits(Some(1440.0)));
        assert!(mark.fits(Some(1400.0)));
        assert!(!mark.fits(Some(1441.0)));
        assert!(!mark.fits(Some(1399.0)));
        assert_eq!(fitting(&[mark], Some(1425.0)), [mark.interval]);
        assert!(fitting(&[mark], Some(1500.0)).is_empty());
        assert_eq!(mark.interval.origin, SkipOrigin::Yours);
    }

    #[test]
    fn mixed_op_and_ed_types_are_accepted() {
        // aniskip uses these when the opening is blended into the episode.
        let payload = r#"{"found":true,"results":[
            {"interval":{"startTime":0.0,"endTime":90.0},"skipType":"mixed-op"}
        ]}"#;
        assert_eq!(parse(payload)[0].kind, SkipKind::Opening);
    }

    #[test]
    fn the_active_interval_tracks_the_playhead() {
        let intervals = parse(PAYLOAD);

        // Before the opening.
        assert!(active(&intervals, 1.0).is_none());
        // Inside it.
        assert_eq!(active(&intervals, 30.0).map(|i| i.kind), Some(SkipKind::Opening));
        // After it, before the ending.
        assert!(active(&intervals, 200.0).is_none());
        // Inside the ending.
        assert_eq!(active(&intervals, 1450.0).map(|i| i.kind), Some(SkipKind::Ending));
        // Past everything.
        assert!(active(&intervals, 1509.0).is_none());
    }

    #[test]
    fn the_prompt_does_not_appear_in_the_first_instant() {
        // Offering to skip an opening the moment it starts is jarring, and someone who just
        // seeked there probably meant to be there.
        let intervals = parse(PAYLOAD);
        assert!(active(&intervals, 3.3).is_none(), "too eager");
        assert!(active(&intervals, 4.0).is_some());
    }

    #[test]
    fn the_end_of_an_interval_is_exclusive() {
        // Otherwise the prompt lingers for a frame after the segment has passed.
        let intervals = parse(PAYLOAD);
        assert!(active(&intervals, 93.221).is_none());
        assert!(active(&intervals, 93.0).is_some());
    }
}

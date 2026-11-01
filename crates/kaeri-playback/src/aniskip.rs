//! What is sent to AniSkip, and what its answers mean: a skip time the viewer marked, and a
//! vote on one of AniSkip's. Pure, as the rest of this crate: the requests are built and the
//! answers read here, and sent by the engine.
//!
//! Nothing here is sent unless the viewer turned sharing on. What is sent is the MyAnimeList
//! id, the episode number, the kind, the two times, the episode's length, the app's name and a
//! random id for the install: no names, accounts, source or file.

use serde::{Deserialize, Serialize};

use crate::{
    marking::{self, Check},
    skip::SkipKind,
};

/// How many times a request is tried before it is let go.
pub const MOST_ATTEMPTS: u32 = 5;

/// How long a vote waits after AniSkip says too many, in seconds: it allows four an hour.
pub const VOTE_WAIT: i64 = 3_600;

/// How long a request waits after the network or AniSkip failed, in seconds, doubled for each
/// attempt after the first.
pub const RETRY_WAIT: i64 = 300;

/// A skip time to submit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Submission {
    pub mal_id: u32,
    pub episode: u32,
    /// AniSkip's name for the kind: `op`, `ed` or `recap`.
    pub kind: String,
    pub start: f64,
    pub end: f64,
    pub episode_length: f64,
}

impl Submission {
    pub fn new(
        mal_id: u32,
        episode: u32,
        kind: SkipKind,
        (start, end): (f64, f64),
        episode_length: f64,
    ) -> Self {
        Self { mal_id, episode, kind: kind.api_type().to_owned(), start, end, episode_length }
    }
}

/// A vote on one of AniSkip's times.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vote {
    pub skip_id: String,
    pub up: bool,
}

/// One request for AniSkip, as the outbox keeps it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Submit(Submission),
    Vote(Vote),
}

impl Request {
    /// What the outbox calls it.
    pub const fn op(&self) -> &'static str {
        match self {
            Self::Submit(_) => "submit",
            Self::Vote(_) => "vote",
        }
    }

    /// The address on `origin` it goes to.
    pub fn url_on(&self, origin: &str) -> String {
        match self {
            Self::Submit(s) => format!("{origin}/v2/skip-times/{}/{}", s.mal_id, s.episode),
            Self::Vote(v) => format!("{origin}/v2/skip-times/vote/{}", v.skip_id),
        }
    }

    /// The JSON body, with the app named `provider` and the install's `submitter` id.
    pub fn body(&self, provider: &str, submitter: &str) -> serde_json::Value {
        match self {
            Self::Submit(s) => serde_json::json!({
                "skipType": s.kind,
                "providerName": provider,
                "startTime": s.start,
                "endTime": s.end,
                "episodeLength": s.episode_length,
                "submitterId": submitter,
            }),
            Self::Vote(v) => {
                serde_json::json!({ "voteType": if v.up { "upvote" } else { "downvote" } })
            }
        }
    }
}

/// What to do with a request after AniSkip answered `status` (`None` for no answer: offline,
/// or the request failed on the way) at `now`, on attempt `attempt`, counting from 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Done. A submission's answer names the time it became, where it says.
    Sent,
    /// Try again at this moment.
    Later(i64),
    /// AniSkip refused it as it is, or it has been tried [`MOST_ATTEMPTS`] times: let it go.
    Dropped,
}

/// The next midnight UTC after `now`: when AniSkip's daily limit on submissions starts again.
pub const fn next_utc_day(now: i64) -> i64 {
    (now.div_euclid(86_400) + 1) * 86_400
}

/// Read an answer: see [`Outcome`].
pub fn outcome(request: &Request, status: Option<u16>, now: i64, attempt: u32) -> Outcome {
    match status {
        Some(200..=299) => Outcome::Sent,
        // Too many: a submission waits for the next day, a vote an hour.
        Some(429) => {
            if attempt >= MOST_ATTEMPTS {
                return Outcome::Dropped;
            }
            match request {
                Request::Submit(_) => Outcome::Later(next_utc_day(now)),
                Request::Vote(_) => Outcome::Later(now + VOTE_WAIT),
            }
        }
        // Refused as it stands, or a time that is gone: sending it again changes nothing.
        Some(400..=499) => Outcome::Dropped,
        // AniSkip down, or no answer: later, for longer each time.
        _ if attempt >= MOST_ATTEMPTS => Outcome::Dropped,
        _ => Outcome::Later(now + RETRY_WAIT * (1_i64 << attempt.saturating_sub(1).min(10))),
    }
}

/// What saving a mark of `kind` from `start` to `end` sends, with sharing on, for episode
/// `episode` of `mal_id`, `episode_length` long. `theirs` is AniSkip's time of that kind in
/// use, by its id and times.
///
/// A mark within [`marking::SAME_WITHIN`] of theirs at both edges confirms it: one upvote and
/// no duplicate. Otherwise the mark is submitted if AniSkip would take it, and theirs is
/// downvoted with it. A mark AniSkip would hide at once is kept on the device only, and sends
/// nothing.
pub fn for_save(
    mal_id: u32,
    episode: u32,
    kind: SkipKind,
    (start, end): (f64, f64),
    episode_length: f64,
    theirs: Option<(&str, (f64, f64))>,
) -> Vec<Request> {
    let theirs = theirs.filter(|(id, _)| !id.is_empty());
    if let Some((id, times)) = theirs
        && marking::same((start, end), times)
    {
        return vec![Request::Vote(Vote { skip_id: id.to_owned(), up: true })];
    }
    if marking::check(kind, start, end, episode_length) != Check::Fine {
        return Vec::new();
    }
    let mut requests = vec![Request::Submit(Submission::new(
        mal_id,
        episode,
        kind,
        (start, end),
        episode_length,
    ))];
    if let Some((id, _)) = theirs {
        requests.push(Request::Vote(Vote { skip_id: id.to_owned(), up: false }));
    }
    requests
}

/// The id of the time a submission became, from AniSkip's answer.
pub fn submitted_id(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Answer {
        #[serde(rename = "skipId")]
        skip_id: String,
    }
    serde_json::from_str::<Answer>(body).ok().map(|a| a.skip_id).filter(|id| !id.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submission() -> Request {
        Request::Submit(Submission::new(4_242, 3, SkipKind::Opening, (61.5, 151.5), 1_420.0))
    }

    #[test]
    fn a_submission_names_the_app_and_the_install_and_nothing_else() {
        let request = submission();
        assert_eq!(
            request.url_on("https://aniskip.example"),
            "https://aniskip.example/v2/skip-times/4242/3"
        );
        let body = request.body("kaeri", "0b6c2a4e-0000-4000-8000-000000000001");
        assert_eq!(
            body,
            serde_json::json!({
                "skipType": "op",
                "providerName": "kaeri",
                "startTime": 61.5,
                "endTime": 151.5,
                "episodeLength": 1420.0,
                "submitterId": "0b6c2a4e-0000-4000-8000-000000000001",
            })
        );
    }

    #[test]
    fn a_vote_goes_to_its_time() {
        let down = Request::Vote(Vote { skip_id: "time-7".into(), up: false });
        assert_eq!(
            down.url_on("http://x.example"),
            "http://x.example/v2/skip-times/vote/time-7"
        );
        assert_eq!(down.body("kaeri", "id"), serde_json::json!({ "voteType": "downvote" }));
        let up = Request::Vote(Vote { skip_id: "time-7".into(), up: true });
        assert_eq!(up.body("kaeri", "id")["voteType"], "upvote");
    }

    #[test]
    fn a_request_is_kept_as_json_and_read_back() {
        for request in [submission(), Request::Vote(Vote { skip_id: "t".into(), up: true })] {
            let kept = serde_json::to_string(&request).unwrap();
            assert_eq!(serde_json::from_str::<Request>(&kept).unwrap(), request);
        }
    }

    #[test]
    fn too_many_waits_for_the_next_day_or_an_hour() {
        let now = 1_800_000_123;
        // The next midnight UTC after it.
        assert_eq!(outcome(&submission(), Some(429), now, 1), Outcome::Later(1_800_057_600));
        let vote = Request::Vote(Vote { skip_id: "t".into(), up: true });
        assert_eq!(outcome(&vote, Some(429), now, 1), Outcome::Later(now + VOTE_WAIT));
        assert_eq!(next_utc_day(86_400 * 3 + 5), 86_400 * 4);
        assert_eq!(next_utc_day(86_400 * 3), 86_400 * 4);
    }

    #[test]
    fn a_refusal_is_let_go_and_a_failure_is_tried_again_five_times_at_most() {
        let now = 1_000;
        assert_eq!(outcome(&submission(), Some(201), now, 1), Outcome::Sent);
        assert_eq!(outcome(&submission(), Some(400), now, 1), Outcome::Dropped);
        assert_eq!(outcome(&submission(), Some(404), now, 1), Outcome::Dropped);
        assert_eq!(outcome(&submission(), None, now, 1), Outcome::Later(now + RETRY_WAIT));
        assert_eq!(
            outcome(&submission(), Some(500), now, 3),
            Outcome::Later(now + RETRY_WAIT * 4)
        );
        assert_eq!(outcome(&submission(), None, now, MOST_ATTEMPTS), Outcome::Dropped);
        assert_eq!(outcome(&submission(), Some(429), now, MOST_ATTEMPTS), Outcome::Dropped);
    }

    #[test]
    fn a_mark_like_theirs_confirms_it_and_is_not_sent_again() {
        let theirs = Some(("time-1", (60.0, 150.0)));
        let sent = for_save(4_242, 3, SkipKind::Opening, (61.5, 151.5), 1_420.0, theirs);
        assert_eq!(sent, [Request::Vote(Vote { skip_id: "time-1".into(), up: true })]);
    }

    #[test]
    fn a_mark_that_replaces_theirs_is_sent_and_theirs_reported() {
        let theirs = Some(("time-1", (40.0, 130.0)));
        let sent = for_save(4_242, 3, SkipKind::Opening, (61.5, 151.5), 1_420.0, theirs);
        assert_eq!(
            sent,
            [submission(), Request::Vote(Vote { skip_id: "time-1".into(), up: false })]
        );
        // With nothing of theirs, or theirs with no id, only the mark goes.
        assert_eq!(
            for_save(4_242, 3, SkipKind::Opening, (61.5, 151.5), 1_420.0, None),
            [submission()]
        );
        let no_id = Some(("", (40.0, 130.0)));
        assert_eq!(
            for_save(4_242, 3, SkipKind::Opening, (61.5, 151.5), 1_420.0, no_id),
            [submission()]
        );
    }

    #[test]
    fn a_mark_aniskip_would_hide_is_kept_here_only() {
        // Four minutes is longer than an opening in an episode under 35 minutes may be.
        let theirs = Some(("time-1", (40.0, 130.0)));
        assert!(
            for_save(4_242, 3, SkipKind::Opening, (60.0, 300.0), 1_420.0, theirs).is_empty()
        );
        assert!(for_save(4_242, 3, SkipKind::Opening, (60.0, 63.0), 1_420.0, None).is_empty());
    }

    #[test]
    fn the_answer_names_the_new_time() {
        assert_eq!(
            submitted_id(r#"{"message":"ok","skipId":"time-9"}"#).as_deref(),
            Some("time-9")
        );
        assert_eq!(submitted_id("{}"), None);
        assert_eq!(submitted_id("not json"), None);
    }
}

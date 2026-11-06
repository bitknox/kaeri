//! Reconciling local state with a tracker's.
//!
//! Pure functions over plain values, because this is the code that decides whether someone's
//! list gets overwritten. Every rule here is reachable from a test, and the interesting cases
//! are the ones where the two sides genuinely disagree.
//!
//! Three fields, three different rules, and conflating them is the usual bug:
//!
//! | Field | Rule | Why |
//! |---|---|---|
//! | progress | `max(local, remote)` | Monotonic. "Watched two on my phone" needs no ceremony. |
//! | reading progress | as progress, or the latest change when the person chose that | An unread here can take a tracker down. |
//! | status | last write wins, by timestamp | Not monotonic — `Dropped` → `Current` is a real edit. |
//! | score | last write wins, by timestamp | Same, and a score is deliberate rather than derived. |
//!
//! The asymmetry is the point. Progress can be merged without asking because moving it forward
//! is never destructive. Status and score can be genuinely contradictory, so when the two sides
//! disagree and we cannot tell which is newer, the divergence is *surfaced* rather than
//! resolved — silently overwriting a status the user set on the website is exactly the kind of
//! thing that makes people stop trusting a sync client.

use kaeri_core::{
    config::WhenTrackersDisagree,
    ids::AnilistId,
    traits::{TrackOp, TrackedEntry, TrackedReadEntry, WatchStatus},
};

/// Local state for one title, as the merge sees it.
///
/// Timestamps are `None` when local never set the field, which is the common case: progress
/// comes from watching, but status and score are only ever set deliberately.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocalState {
    pub progress: u32,
    pub status: Option<WatchStatus>,
    /// When the local status was last set.
    pub status_at: Option<i64>,
    pub score: Option<f32>,
    pub score_at: Option<i64>,
    /// The highest progress this tracker has already been sent for the title, if any.
    ///
    /// Not state about the title but about the conversation with one tracker, and it is what
    /// stops an echo: a tracker that answers a push with success and keeps reporting a lower
    /// figure would otherwise be sent the same value again after every pull.
    pub last_pushed: Option<u32>,
}

/// What reconciling one title produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Merged {
    /// Operations to push, so the remote catches up with local.
    pub push: Vec<TrackOp>,
    /// The value local should adopt, when the remote is ahead.
    pub adopt_progress: Option<u32>,
    /// Disagreements the user has to settle.
    pub conflicts: Vec<Conflict>,
}

impl Merged {
    pub fn is_empty(&self) -> bool {
        self.push.is_empty() && self.adopt_progress.is_none() && self.conflicts.is_empty()
    }
}

/// A disagreement that cannot be resolved without asking.
#[derive(Debug, Clone, PartialEq)]
pub struct Conflict {
    pub anilist_id: AnilistId,
    pub field: Field,
    pub local: String,
    pub remote: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Status,
    Score,
}

impl Field {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Score => "score",
        }
    }
}

/// Scores differing by less than this are the same score.
///
/// AniList stores scores as floats across several user-selectable scales (100-point, 10-point,
/// 5-star), so a round-trip can perturb the last bit. Treating that as a conflict would prompt
/// the user about nothing.
const SCORE_EPSILON: f32 = 0.01;

/// Reconcile one title.
///
/// `pending_progress` is the highest episode already queued in the outbox. Without it a push
/// that has not drained yet looks like the remote being ahead, and the merge would helpfully
/// walk local progress *backwards* to the stale remote value.
pub fn reconcile(
    local: &LocalState,
    remote: &TrackedEntry,
    pending_progress: Option<u32>,
) -> Merged {
    let mut merged = Merged::default();

    // Progress: monotonic, so the higher value simply wins and nobody needs to be asked.
    let local_effective = local.progress.max(pending_progress.unwrap_or(0));
    match local_effective.cmp(&remote.progress) {
        std::cmp::Ordering::Greater => {
            // Only queue if it is not already queued — the outbox coalesces, but skipping the
            // call keeps the drain quiet on a no-op resync — and not if this very value has
            // already been sent. A tracker still behind after taking it has refused or
            // ignored it (a title it could not match, an episode past its own count), and
            // sending it again changes nothing: it did, every pull, for a week. New progress
            // past what was sent goes out as usual.
            let already_sent = local.last_pushed.is_some_and(|sent| sent >= local_effective);
            if pending_progress != Some(local_effective) && !already_sent {
                merged.push.push(TrackOp::SetProgress {
                    anilist_id: remote.anilist_id,
                    episode: local_effective,
                });
            }
        }
        std::cmp::Ordering::Less => merged.adopt_progress = Some(remote.progress),
        std::cmp::Ordering::Equal => {}
    }

    // Status: last write wins, and an unknowable ordering is a conflict rather than a guess.
    match (local.status, local.status_at) {
        (Some(status), _) if status == remote.status => {}
        (Some(status), Some(at)) => {
            // We know when local changed but never when the remote did, so a local edit is
            // only allowed to win if it is *newer than the pull*. The caller passes the pull
            // time as the remote's timestamp; see `reconcile_at`.
            merged.push.push(TrackOp::SetStatus { anilist_id: remote.anilist_id, status, at });
        }
        (Some(status), None) => {
            // Local has an opinion with no idea when it was formed. Refusing to guess is the
            // whole reason the Conflicts overlay exists.
            merged.conflicts.push(Conflict {
                anilist_id: remote.anilist_id,
                field: Field::Status,
                local: format!("{status:?}"),
                remote: format!("{:?}", remote.status),
            });
        }
        (None, _) => {}
    }

    // Score: same shape as status.
    match (local.score, local.score_at) {
        (Some(score), _) if scores_match(Some(score), remote.score) => {}
        (Some(score), Some(at)) => {
            merged.push.push(TrackOp::SetScore { anilist_id: remote.anilist_id, score, at });
        }
        (Some(score), None) => merged.conflicts.push(Conflict {
            anilist_id: remote.anilist_id,
            field: Field::Score,
            local: format!("{score}"),
            remote: remote.score.map_or_else(|| "—".into(), |s| format!("{s}")),
        }),
        (None, _) => {}
    }

    merged
}

/// Reconcile with knowledge of when the remote value was last observed.
///
/// This is the version worth using when a previous pull recorded a timestamp: a local status
/// older than the last successful pull has already been superseded remotely, so pushing it
/// would undo whatever the user did on the website.
pub fn reconcile_at(
    local: &LocalState,
    remote: &TrackedEntry,
    pending_progress: Option<u32>,
    remote_observed_at: i64,
) -> Merged {
    let mut merged = reconcile(local, remote, pending_progress);

    // Drop any push whose local timestamp predates the remote observation, and record the
    // disagreement instead. Progress is exempt: it is monotonic, so it is never destructive.
    let mut kept = Vec::with_capacity(merged.push.len());
    for op in merged.push.drain(..) {
        let (at, field, local_repr, remote_repr) = match &op {
            TrackOp::SetProgress { .. }
            | TrackOp::SetReadProgress { .. }
            | TrackOp::SetReadStatus { .. }
            | TrackOp::SetReadScore { .. } => {
                kept.push(op);
                continue;
            }
            TrackOp::SetStatus { status, at, .. } => {
                (*at, Field::Status, format!("{status:?}"), format!("{:?}", remote.status))
            }
            TrackOp::SetScore { score, at, .. } => (
                *at,
                Field::Score,
                format!("{score}"),
                remote.score.map_or_else(|| "—".into(), |s| format!("{s}")),
            ),
        };

        if at >= remote_observed_at {
            kept.push(op);
        } else {
            merged.conflicts.push(Conflict {
                anilist_id: remote.anilist_id,
                field,
                local: local_repr,
                remote: remote_repr,
            });
        }
    }
    merged.push = kept;
    merged
}

/// Local state for one manga or novel, as the merge sees it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReadLocal {
    /// The highest whole chapter read.
    pub chapters: u32,
    /// The volume known to be finished, where there is one.
    pub volumes: Option<u32>,
    pub status: Option<WatchStatus>,
    /// Read again from the start: the status goes out as Repeating.
    pub rereading: bool,
    pub status_at: Option<i64>,
    pub score: Option<f32>,
    pub score_at: Option<i64>,
    /// The chapters and volumes this tracker was last sent for the work.
    pub last_pushed: Option<(u32, Option<u32>)>,
    /// When the person last changed how far they are here: a chapter read or unread, or read
    /// up to a chapter.
    pub progress_at: Option<i64>,
    /// How progress settles when the two disagree: the person's setting.
    pub rule: WhenTrackersDisagree,
}

/// What reconciling one manga or novel produced.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReadMerged {
    pub push: Vec<TrackOp>,
    /// The chapters and volumes local should adopt, when the remote is ahead.
    pub adopt: Option<(u32, Option<u32>)>,
    /// The progress pushed takes the tracker's count down, to a change made here after the
    /// tracker was last read: it replaces whatever progress is queued.
    pub walk_back: bool,
    pub conflicts: Vec<Conflict>,
}

/// Reconcile one manga or novel with the rules of [`reconcile_at`]: chapters and volumes are
/// each `max(local, remote)` and never walk back, status and score are last write wins.
///
/// When the person chose [`WhenTrackersDisagree::Latest`], progress follows the latest change
/// too: a count changed here since the tracker was last read goes out even when it is lower.
/// Read again never lowers a count, whatever the rule.
///
/// `pending` is what the outbox already holds for the work. Volumes are left alone where the
/// remote has no field for them (`None`, as Kitsu).
pub fn reconcile_read_at(
    local: &ReadLocal,
    remote: &TrackedReadEntry,
    pending: Option<(u32, Option<u32>)>,
    remote_observed_at: i64,
) -> ReadMerged {
    let mut merged = ReadMerged::default();
    let id = remote.anilist_id;

    let changed_here_since = local.rule == WhenTrackersDisagree::Latest
        && !local.rereading
        && remote_observed_at > 0
        && local.progress_at.is_some_and(|at| at > remote_observed_at);
    let lower = local.chapters < remote.chapters
        || matches!((local.volumes, remote.volumes), (Some(l), Some(r)) if l < r);
    if changed_here_since && lower {
        merged.push.push(TrackOp::SetReadProgress {
            anilist_id: id,
            chapter: local.chapters,
            volume: if remote.volumes.is_some() { local.volumes } else { None },
        });
        merged.walk_back = true;
        settle_status_and_score(local, remote, remote_observed_at, &mut merged);
        return merged;
    }

    let (pending_chapters, pending_volumes) = pending.unwrap_or((0, None));
    let chapters = local.chapters.max(pending_chapters);
    let volumes = local.volumes.max(pending_volumes);

    let chapters_ahead = chapters > remote.chapters;
    let volumes_ahead = match (volumes, remote.volumes) {
        (Some(local), Some(remote)) => local > remote,
        _ => false,
    };
    // A value already queued, or already sent, is not queued again.
    let queued = pending == Some((chapters, volumes));
    let sent = local.last_pushed.is_some_and(|(c, v)| c >= chapters && v >= volumes);
    if (chapters_ahead || volumes_ahead) && !queued && !sent {
        merged.push.push(TrackOp::SetReadProgress {
            anilist_id: id,
            chapter: chapters.max(remote.chapters),
            volume: if remote.volumes.is_some() { volumes } else { None },
        });
    }
    let chapters_behind = chapters < remote.chapters;
    let volumes_behind = match (volumes, remote.volumes) {
        (Some(local), Some(remote)) => local < remote,
        (None, Some(remote)) => remote > 0,
        _ => false,
    };
    if chapters_behind || volumes_behind {
        merged.adopt = Some((
            remote.chapters.max(chapters),
            remote.volumes.filter(|v| *v > 0).or(local.volumes),
        ));
    }
    settle_status_and_score(local, remote, remote_observed_at, &mut merged);
    merged
}

/// Status and score: the side changed last wins, and an undated local one is a conflict.
fn settle_status_and_score(
    local: &ReadLocal,
    remote: &TrackedReadEntry,
    remote_observed_at: i64,
    merged: &mut ReadMerged,
) {
    let id = remote.anilist_id;
    let remote_status = if remote.rereading { WatchStatus::Repeating } else { remote.status };
    if let Some(status) = local.status {
        let local_status = if local.rereading { WatchStatus::Repeating } else { status };
        if local_status != remote_status {
            match local.status_at {
                Some(at) if at >= remote_observed_at => {
                    merged.push.push(TrackOp::SetReadStatus {
                        anilist_id: id,
                        status: if local_status == WatchStatus::Repeating {
                            WatchStatus::Current
                        } else {
                            local_status
                        },
                        rereading: local_status == WatchStatus::Repeating,
                        at,
                    })
                }
                _ => merged.conflicts.push(Conflict {
                    anilist_id: id,
                    field: Field::Status,
                    local: format!("{local_status:?}"),
                    remote: format!("{remote_status:?}"),
                }),
            }
        }
    }
    if let Some(score) = local.score
        && !scores_match(Some(score), remote.score)
    {
        match local.score_at {
            Some(at) if at >= remote_observed_at => {
                merged.push.push(TrackOp::SetReadScore { anilist_id: id, score, at });
            }
            _ => merged.conflicts.push(Conflict {
                anilist_id: id,
                field: Field::Score,
                local: format!("{score}"),
                remote: remote.score.map_or_else(|| "-".into(), |s| format!("{s}")),
            }),
        }
    }
}

/// Whether a title present locally but absent from the remote list should be pushed.
///
/// Only if something was actually watched. Opening a title to read the synopsis must not add it
/// to someone's list — a sync client that silently populates your AniList with everything you
/// glanced at is worse than one that does nothing.
pub fn should_add_to_remote(local: &LocalState) -> bool {
    local.progress > 0 || local.status.is_some() || local.score.is_some()
}

fn scores_match(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (Some(x), Some(y)) => (x - y).abs() < SCORE_EPSILON,
        (None, None) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOSHIKUZU: AnilistId = AnilistId::new(154_587);

    fn remote(progress: u32, status: WatchStatus, score: Option<f32>) -> TrackedEntry {
        TrackedEntry { anilist_id: HOSHIKUZU, progress, status, score }
    }

    #[test]
    fn the_higher_progress_wins_in_either_direction() {
        // The whole reason progress needs no prompting: it only ever moves forward, so the
        // maximum is always right.
        let ahead = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile(&ahead, &remote(7, WatchStatus::Current, None), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 }]
        );
        assert_eq!(merged.adopt_progress, None);

        let behind = LocalState { progress: 3, ..Default::default() };
        let merged = reconcile(&behind, &remote(9, WatchStatus::Current, None), None);
        assert!(merged.push.is_empty());
        assert_eq!(merged.adopt_progress, Some(9), "watched elsewhere should be adopted");
    }

    #[test]
    fn equal_progress_produces_no_work() {
        let local = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile(&local, &remote(12, WatchStatus::Current, None), None);
        assert!(merged.is_empty(), "a no-op resync must not generate traffic: {merged:?}");
    }

    #[test]
    fn a_queued_push_is_not_mistaken_for_the_remote_being_ahead() {
        // Local shows 3 because the projection has not caught up, the outbox already holds 12,
        // and the remote still says 7. Without the pending value the merge would adopt 7 and
        // walk progress backwards.
        let local = LocalState { progress: 3, ..Default::default() };
        let merged = reconcile(&local, &remote(7, WatchStatus::Current, None), Some(12));
        assert_eq!(merged.adopt_progress, None, "adopted a stale remote over a queued push");
        assert!(merged.push.is_empty(), "12 is already queued; queueing it again is noise");
    }

    #[test]
    fn a_queued_push_below_local_still_pushes_local() {
        let local = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile(&local, &remote(2, WatchStatus::Current, None), Some(5));
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 }]
        );
    }

    #[test]
    fn a_value_already_sent_is_not_sent_again_while_the_remote_stays_behind() {
        // The echo: the tracker answered the push with success, kept reporting 7, and every
        // pull queued 12 again. Once sent, 12 waits for local progress to move past it.
        let local = LocalState { progress: 12, last_pushed: Some(12), ..Default::default() };
        let merged = reconcile(&local, &remote(7, WatchStatus::Current, None), None);
        assert!(merged.push.is_empty(), "re-sent a value the tracker already had: {merged:?}");
        assert_eq!(merged.adopt_progress, None, "and the lower remote is still not adopted");

        let moved_on = LocalState { progress: 13, ..local };
        let merged = reconcile(&moved_on, &remote(7, WatchStatus::Current, None), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 13 }],
            "new progress still goes out"
        );
    }

    #[test]
    fn a_matching_status_is_left_alone() {
        let local = LocalState {
            progress: 5,
            status: Some(WatchStatus::Current),
            status_at: Some(1_000),
            ..Default::default()
        };
        let merged = reconcile(&local, &remote(5, WatchStatus::Current, None), None);
        assert!(merged.is_empty());
    }

    #[test]
    fn a_timestamped_local_status_is_pushed() {
        let local = LocalState {
            status: Some(WatchStatus::Completed),
            status_at: Some(2_000),
            ..Default::default()
        };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, None), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetStatus {
                anilist_id: HOSHIKUZU,
                status: WatchStatus::Completed,
                at: 2_000
            }]
        );
        assert!(merged.conflicts.is_empty());
    }

    #[test]
    fn a_status_with_no_timestamp_is_surfaced_rather_than_guessed() {
        // Status is not monotonic, so with no way to order the two edits, picking one would
        // be overwriting someone's deliberate choice on a coin flip.
        let local = LocalState {
            status: Some(WatchStatus::Dropped),
            status_at: None,
            ..Default::default()
        };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, None), None);
        assert!(merged.push.is_empty(), "must not push an unorderable status");
        assert_eq!(merged.conflicts.len(), 1);
        assert_eq!(merged.conflicts[0].field, Field::Status);
        assert_eq!(merged.conflicts[0].local, "Dropped");
        assert_eq!(merged.conflicts[0].remote, "Current");
    }

    #[test]
    fn a_local_status_older_than_the_last_pull_loses() {
        // The user set it to Dropped locally, then changed it to Current on the website. The
        // website edit is newer, so pushing the local value would undo it.
        let local = LocalState {
            status: Some(WatchStatus::Dropped),
            status_at: Some(1_000),
            ..Default::default()
        };
        let merged = reconcile_at(&local, &remote(0, WatchStatus::Current, None), None, 5_000);
        assert!(merged.push.is_empty(), "a stale local status overwrote a newer remote one");
        assert_eq!(merged.conflicts.len(), 1, "the divergence still has to be visible");
    }

    #[test]
    fn a_local_status_newer_than_the_last_pull_wins() {
        let local = LocalState {
            status: Some(WatchStatus::Completed),
            status_at: Some(9_000),
            ..Default::default()
        };
        let merged = reconcile_at(&local, &remote(0, WatchStatus::Current, None), None, 5_000);
        assert_eq!(merged.push.len(), 1);
        assert!(merged.conflicts.is_empty());
    }

    #[test]
    fn progress_is_never_held_back_by_the_pull_timestamp() {
        // Progress is monotonic, so it is exempt from last-write-wins entirely. Making it
        // wait for a fresher timestamp would lose episodes for no benefit.
        let local = LocalState { progress: 12, ..Default::default() };
        let merged = reconcile_at(&local, &remote(7, WatchStatus::Current, None), None, 9_999);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetProgress { anilist_id: HOSHIKUZU, episode: 12 }]
        );
    }

    #[test]
    fn scores_that_only_differ_by_float_noise_are_the_same_score() {
        // AniList stores scores as floats across several scales, so a round-trip can perturb
        // the last bit. Prompting about that would be prompting about nothing.
        let local =
            LocalState { score: Some(8.5), score_at: Some(1_000), ..Default::default() };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, Some(8.500_001)), None);
        assert!(merged.is_empty(), "float noise was treated as a conflict: {merged:?}");
    }

    #[test]
    fn a_genuinely_different_score_is_pushed() {
        let local =
            LocalState { score: Some(9.0), score_at: Some(1_000), ..Default::default() };
        let merged = reconcile(&local, &remote(0, WatchStatus::Current, Some(6.0)), None);
        assert_eq!(
            merged.push,
            vec![TrackOp::SetScore { anilist_id: HOSHIKUZU, score: 9.0, at: 1_000 }]
        );
    }

    #[test]
    fn a_score_only_on_the_remote_is_left_alone() {
        // Local has no opinion, so there is nothing to push and nothing to resolve.
        let merged = reconcile(
            &LocalState::default(),
            &remote(0, WatchStatus::Current, Some(7.0)),
            None,
        );
        assert!(merged.is_empty());
    }

    #[test]
    fn glancing_at_a_title_does_not_add_it_to_your_list() {
        // The distinction that keeps a sync client trustworthy: opening a title to read the
        // synopsis writes nothing anywhere.
        assert!(!should_add_to_remote(&LocalState::default()));
        assert!(should_add_to_remote(&LocalState { progress: 1, ..Default::default() }));
        assert!(should_add_to_remote(&LocalState {
            status: Some(WatchStatus::Planning),
            ..Default::default()
        }));
        assert!(should_add_to_remote(&LocalState { score: Some(8.0), ..Default::default() }));
    }

    #[test]
    fn every_op_produced_is_addressed_to_the_right_title() {
        // A merge that emitted an op for the wrong id would corrupt an unrelated entry.
        let local = LocalState {
            progress: 12,
            status: Some(WatchStatus::Completed),
            status_at: Some(2_000),
            score: Some(9.0),
            score_at: Some(2_000),
            last_pushed: None,
        };
        let merged = reconcile(&local, &remote(1, WatchStatus::Current, Some(3.0)), None);
        assert_eq!(merged.push.len(), 3);
        for op in &merged.push {
            assert_eq!(op.anilist_id(), HOSHIKUZU);
        }
    }

    fn read_remote(
        chapters: u32,
        volumes: Option<u32>,
        status: WatchStatus,
    ) -> TrackedReadEntry {
        TrackedReadEntry {
            anilist_id: HOSHIKUZU,
            medium: kaeri_core::media::Medium::Manga,
            chapters,
            volumes,
            status,
            rereading: false,
            score: None,
        }
    }

    #[test]
    fn a_read_chapter_count_only_moves_forward_in_either_direction() {
        let local = ReadLocal { chapters: 12, ..ReadLocal::default() };
        let ahead =
            reconcile_read_at(&local, &read_remote(8, None, WatchStatus::Current), None, 0);
        assert_eq!(
            ahead.push,
            [TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 12, volume: None }]
        );
        assert_eq!(ahead.adopt, None);
        let behind =
            reconcile_read_at(&local, &read_remote(20, None, WatchStatus::Current), None, 0);
        assert!(behind.push.is_empty());
        assert_eq!(behind.adopt, Some((20, None)));
    }

    #[test]
    fn a_count_already_sent_or_queued_is_not_sent_again() {
        let local =
            ReadLocal { chapters: 12, last_pushed: Some((12, None)), ..ReadLocal::default() };
        let remote = read_remote(8, None, WatchStatus::Current);
        assert!(reconcile_read_at(&local, &remote, None, 0).push.is_empty());
        let fresh = ReadLocal { chapters: 12, ..ReadLocal::default() };
        assert!(reconcile_read_at(&fresh, &remote, Some((12, None)), 0).push.is_empty());
    }

    #[test]
    fn volumes_are_left_alone_where_the_tracker_has_no_field() {
        let local = ReadLocal { chapters: 12, volumes: Some(2), ..ReadLocal::default() };
        let merged =
            reconcile_read_at(&local, &read_remote(8, None, WatchStatus::Current), None, 0);
        assert_eq!(
            merged.push,
            [TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 12, volume: None }]
        );
        let with =
            reconcile_read_at(&local, &read_remote(12, Some(1), WatchStatus::Current), None, 0);
        assert_eq!(
            with.push,
            [TrackOp::SetReadProgress { anilist_id: HOSHIKUZU, chapter: 12, volume: Some(2) }]
        );
    }

    #[test]
    fn a_newer_local_status_is_sent_and_an_undated_one_is_a_conflict() {
        let remote = read_remote(5, None, WatchStatus::Current);
        let newer = ReadLocal {
            chapters: 5,
            status: Some(WatchStatus::Completed),
            status_at: Some(100),
            ..ReadLocal::default()
        };
        let merged = reconcile_read_at(&newer, &remote, None, 50);
        assert!(matches!(
            merged.push.as_slice(),
            [TrackOp::SetReadStatus { status: WatchStatus::Completed, rereading: false, .. }]
        ));
        let older = ReadLocal { status_at: None, ..newer };
        let conflict = reconcile_read_at(&older, &remote, None, 50);
        assert!(conflict.push.is_empty());
        assert_eq!(conflict.conflicts.len(), 1);
        assert_eq!(conflict.conflicts[0].field, Field::Status);
    }

    #[test]
    fn with_the_latest_change_winning_a_lower_count_read_here_since_the_pull_goes_out() {
        let remote = read_remote(20, Some(3), WatchStatus::Current);
        let unread = ReadLocal {
            chapters: 12,
            volumes: Some(2),
            progress_at: Some(600),
            rule: WhenTrackersDisagree::Latest,
            ..ReadLocal::default()
        };
        let merged = reconcile_read_at(&unread, &remote, Some((20, Some(3))), 500);
        assert_eq!(
            merged.push,
            [TrackOp::SetReadProgress {
                anilist_id: remote.anilist_id,
                chapter: 12,
                volume: Some(2)
            }]
        );
        assert!(merged.walk_back, "it replaces the higher count queued");
        assert_eq!(merged.adopt, None);

        // Changed on the tracker since: the tracker's count is the latest, and adopted.
        let older = ReadLocal { progress_at: Some(400), ..unread.clone() };
        let merged = reconcile_read_at(&older, &remote, None, 500);
        assert!(merged.push.is_empty() && !merged.walk_back);
        assert_eq!(merged.adopt, Some((20, Some(3))));

        // Keep the furthest, the default, never lowers a count.
        let furthest = ReadLocal { rule: WhenTrackersDisagree::Furthest, ..unread.clone() };
        let merged = reconcile_read_at(&furthest, &remote, None, 500);
        assert!(merged.push.is_empty() && !merged.walk_back);
        assert_eq!(merged.adopt, Some((20, Some(3))));

        // Read again starts the count afresh here; the tracker keeps its own.
        let again = ReadLocal { chapters: 0, rereading: true, ..unread.clone() };
        assert!(!reconcile_read_at(&again, &remote, None, 500).walk_back);

        // The first pull has nothing to be newer than.
        assert!(!reconcile_read_at(&unread, &remote, None, 0).walk_back);
    }

    #[test]
    fn read_again_goes_out_as_current_with_the_flag() {
        let local = ReadLocal {
            chapters: 5,
            status: Some(WatchStatus::Current),
            rereading: true,
            status_at: Some(100),
            ..ReadLocal::default()
        };
        let merged =
            reconcile_read_at(&local, &read_remote(5, None, WatchStatus::Completed), None, 50);
        assert!(matches!(
            merged.push.as_slice(),
            [TrackOp::SetReadStatus { status: WatchStatus::Current, rereading: true, .. }]
        ));
    }
}

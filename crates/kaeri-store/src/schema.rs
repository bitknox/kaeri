//! Schema and forward-only migrations.
//!
//! The table layout encodes two decisions from the design that are hard to retrofit, so
//! they are here from the first migration rather than added later:
//!
//! 1. **`watch_event` is an append-only log, and `watch_progress` is a projection of it.**
//!    History is richer than any tracker can represent — position, duration watched, which
//!    provider served it, which translation — and none of that survives a round-trip
//!    through AniList. So local is the source of truth and sync is derived from it, not
//!    the other way around.
//!
//! 2. **`sync_outbox` is a durable table, not an in-memory queue.** Progress recorded
//!    while offline has to survive a process kill, otherwise "watched on a plane" silently
//!    loses episodes.
//!
//! `mapping_override` is deliberately separate from `mapping_resolution`: a dataset
//! refresh clears cached resolutions but must never touch a correction the user made by
//! hand.

use rusqlite::Connection;

/// Ordered migrations. Append only — never edit an existing entry, since it may already
/// have run on a user's database.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "0001_initial",
        r#"
    -- Cached AniList responses, keyed by media id.
    CREATE TABLE media_cache (
        anilist_id   INTEGER PRIMARY KEY,
        payload      TEXT    NOT NULL,
        fetched_at   INTEGER NOT NULL
    );

    -- Materialised ID mapping, merged from the upstream datasets on refresh.
    -- Parsed once per refresh rather than on every launch: re-reading ~15 MB of JSON at
    -- startup would be pure waste.
    CREATE TABLE mapping (
        anilist_id      INTEGER PRIMARY KEY,
        mal_id          INTEGER,
        kitsu_id        INTEGER,
        anidb_id        INTEGER,
        tvdb_id         INTEGER,
        tmdb_id         INTEGER,
        title_normal    TEXT,
        -- INTEGER, not TEXT: SQLite's TEXT affinity silently coerces a written integer
        -- into a string, so reading it back as a number then fails at runtime.
        episode_offset  INTEGER,
        source          TEXT    NOT NULL
    );
    CREATE INDEX idx_mapping_mal   ON mapping(mal_id);
    CREATE INDEX idx_mapping_title ON mapping(title_normal);

    -- Episode redirection rules from the episode numbering dataset, for continuous numbering.
    CREATE TABLE episode_relation (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id    INTEGER NOT NULL,
        src_from      INTEGER NOT NULL,
        src_to        INTEGER NOT NULL,
        dst_anilist   INTEGER NOT NULL,
        dst_from      INTEGER NOT NULL,
        dst_to        INTEGER NOT NULL
    );
    CREATE INDEX idx_relation_anilist ON episode_relation(anilist_id);

    -- Refresh bookkeeping for each RefreshableDataset: the stored ETag makes the
    -- steady-state check a 304 with no body.
    CREATE TABLE dataset_state (
        name        TEXT PRIMARY KEY,
        url         TEXT    NOT NULL,
        etag        TEXT,
        fetched_at  INTEGER,
        item_count  INTEGER,
        last_error  TEXT
    );

    -- Rung 1 of the resolution ladder: a correction the user made by hand.
    -- Always wins, never expires, and survives every dataset refresh.
    CREATE TABLE mapping_override (
        anilist_id   INTEGER NOT NULL,
        provider_id  TEXT    NOT NULL,
        provider_key TEXT    NOT NULL,
        created_at   INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, provider_id)
    );

    -- Rung 2: a previously successful automatic resolution, with the rung that produced
    -- it so low-confidence matches can be re-examined later.
    CREATE TABLE mapping_resolution (
        anilist_id   INTEGER NOT NULL,
        provider_id  TEXT    NOT NULL,
        provider_key TEXT    NOT NULL,
        confidence   REAL    NOT NULL,
        rung         INTEGER NOT NULL,
        resolved_at  INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, provider_id)
    );

    -- Append-only watch log. The source of truth.
    CREATE TABLE watch_event (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id    INTEGER NOT NULL,
        episode       TEXT    NOT NULL,
        position_secs REAL    NOT NULL,
        duration_secs REAL,
        watched_secs  REAL    NOT NULL DEFAULT 0,
        provider_id   TEXT,
        translation   TEXT,
        completed     INTEGER NOT NULL DEFAULT 0,
        at            INTEGER NOT NULL
    );
    CREATE INDEX idx_event_title ON watch_event(anilist_id, at DESC);
    CREATE INDEX idx_event_at    ON watch_event(at DESC);

    -- Fast-read projection for the CONTINUE rail and resume prompt.
    CREATE TABLE watch_progress (
        anilist_id     INTEGER PRIMARY KEY,
        last_episode   TEXT    NOT NULL,
        last_position  REAL    NOT NULL,
        last_duration  REAL,
        episodes_done  INTEGER NOT NULL DEFAULT 0,
        updated_at     INTEGER NOT NULL
    );
    CREATE INDEX idx_progress_recent ON watch_progress(updated_at DESC);

    -- Durable per-tracker outbox. Survives a process kill; drains on reconnect.
    CREATE TABLE sync_outbox (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        tracker_id  TEXT    NOT NULL,
        op          TEXT    NOT NULL,
        anilist_id  INTEGER NOT NULL,
        created_at  INTEGER NOT NULL,
        attempts    INTEGER NOT NULL DEFAULT 0,
        next_retry  INTEGER NOT NULL DEFAULT 0,
        last_error  TEXT
    );
    CREATE INDEX idx_outbox_ready ON sync_outbox(tracker_id, next_retry);

    -- Per-episode extras: aniskip intervals and filler/recap flags.
    CREATE TABLE episode_meta (
        anilist_id  INTEGER NOT NULL,
        episode     TEXT    NOT NULL,
        op_start    REAL,
        op_end      REAL,
        ed_start    REAL,
        ed_end      REAL,
        is_filler   INTEGER NOT NULL DEFAULT 0,
        is_recap    INTEGER NOT NULL DEFAULT 0,
        fetched_at  INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, episode)
    );
    "#,
    ),
    (
        "0002_sync_state",
        r#"
    -- Small key/value store for cursors that are not worth a table of their own.
    -- The tracker sync uses it for the last successful pull time per tracker, which is the
    -- baseline last-write-wins compares a local edit against.
    CREATE TABLE app_state (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    -- Titles for ids we have seen, so a sync conflict can name the show rather than showing a
    -- bare AniList id. Separate from `media_cache` because it has no TTL: a title is stable,
    -- and this is a lookup table rather than a cache.
    CREATE TABLE title_index (
        anilist_id INTEGER PRIMARY KEY,
        title      TEXT NOT NULL,
        seen_at    INTEGER NOT NULL
    );
    "#,
    ),
    (
        "0002_downloads",
        r#"
    -- The download queue, and it has to be a table rather than in-memory state.
    --
    -- A download outlives the process that started it in the only sense that matters: the *files*
    -- are still on disk after a restart, and a queue that forgot about them would either re-fetch
    -- from zero or leave orphans nobody can find. librqbit can resume a partial torrent, so the
    -- magnet and the target path are the two things worth persisting.
    CREATE TABLE download (
        id           INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id   INTEGER NOT NULL,
        episode      TEXT    NOT NULL,
        title        TEXT    NOT NULL,
        magnet       TEXT    NOT NULL,
        -- queued · active · paused · done · failed
        state        TEXT    NOT NULL,
        -- Where the finished file landed. Null until it is known, which is after metadata arrives.
        path         TEXT,
        downloaded   INTEGER NOT NULL DEFAULT 0,
        total        INTEGER NOT NULL DEFAULT 0,
        -- Null unless `state = 'failed'`. Kept so the screen can say *why* rather than just that.
        error        TEXT,
        created_at   INTEGER NOT NULL,
        updated_at   INTEGER NOT NULL,
        -- One row per episode: asking twice is not two downloads, and without this a held-down key
        -- would queue the same file repeatedly.
        UNIQUE(anilist_id, episode)
    );
    CREATE INDEX idx_download_state ON download(state, created_at);
    "#,
    ),
    (
        "0002_provider_preference",
        r#"
    -- "Use this source for this series." Distinct from `mapping_override`, which answers
    -- *which title* a provider means; this answers *which provider* to ask at all.
    --
    -- The right source can differ per title, so a title can pin one. `providers.order` is the
    -- default; this overrides it for one title.
    --
    -- The provider is stored by id, and a stored id that no longer exists — a plugin the user
    -- removed — is treated as "no preference" at read time rather than being cleaned up here.
    -- Deleting the row would silently discard the choice on a launch where a plugin merely
    -- failed to compile.
    CREATE TABLE provider_preference (
        anilist_id  INTEGER PRIMARY KEY,
        provider_id TEXT    NOT NULL,
        created_at  INTEGER NOT NULL
    );
    "#,
    ),
    (
        "0003_continue_hidden",
        r#"
    -- "Stop showing this on CONTINUE." A flag rather than deleting history: the watch log
    -- is the source of truth and must survive a tidy-up of the rail. Any new watch event
    -- clears it, so a dismissed title earns its way back by being watched, never by
    -- lingering.
    ALTER TABLE watch_progress ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
    "#,
    ),
    (
        "0004_mapping_priority",
        r#"
    -- Which dataset's claim a mapping row currently holds, so a later refresh can tell
    -- "fill this gap" from "overrule this". Without it the merge was last-writer-wins and
    -- the weekly corpus overwrote the daily one whenever it happened to land second — after
    -- which aniskip, MAL and Trakt were talking about a different show.
    --
    -- Defaults to the lowest-priority value so rows written before this migration are
    -- treated as the weakest claim: any dataset may correct them, which is the safe way to
    -- inherit history we cannot attribute.
    ALTER TABLE mapping ADD COLUMN priority INTEGER NOT NULL DEFAULT 255;

    -- Trakt resolves shows by TVDB id, twice per synced item, and both queries scanned the
    -- whole materialised table — tens of thousands of rows.
    CREATE INDEX IF NOT EXISTS idx_mapping_tvdb ON mapping(tvdb_id);
    "#,
    ),
    (
        "0005_track_memory",
        r#"
    -- The audio and subtitle tracks the viewer chose in the player for a series, and the
    -- subtitle delay they settled on, carried to the next episode.
    --
    -- Keyed by translation as well as title: the same viewer wants different tracks from a
    -- dub than from a sub, and one choice overwriting the other would make switching back
    -- and forth start from the wrong place each time.
    --
    -- The choice itself is JSON. It describes tracks by language, title and flags rather
    -- than by id — ids are per file — and that description is the player's vocabulary,
    -- which may grow; a column per field would need a migration every time it did.
    CREATE TABLE track_memory (
        anilist_id  INTEGER NOT NULL,
        translation TEXT    NOT NULL,
        memory      TEXT    NOT NULL,
        updated_at  INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, translation)
    );
    "#,
    ),
    (
        "0006_show_settings",
        r#"
    -- What a show sets for itself over the Settings screen's own: sub or dub, quality, the
    -- opening and filler skipped or not, spoiler protection. One row per setting a show sets;
    -- a setting with no row follows Settings, so a new default reaches every show that never
    -- chose otherwise.
    --
    -- The value is text, as the model's rules for each setting write and read it, and the
    -- setting is named by the model's stable name for it. A name this build no longer knows,
    -- or a value it no longer reads, is passed over rather than refused.
    CREATE TABLE show_setting (
        anilist_id INTEGER NOT NULL,
        setting    TEXT    NOT NULL,
        value      TEXT    NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, setting)
    );
    "#,
    ),
    (
        "0007_server_health",
        r#"
    -- How each server a provider streams from has fared lately, so one that keeps failing is
    -- tried after the ones that play rather than first every time. See `server_health.rs`.
    --
    -- A server is the provider and the origin of its stream's address, `scheme://host[:port]`,
    -- never the path or query: a stream address carries signed tokens, and this table must hold
    -- nothing that could be replayed or that names what was watched.
    CREATE TABLE server_health (
        provider_id          TEXT    NOT NULL,
        origin               TEXT    NOT NULL,
        successes            INTEGER NOT NULL DEFAULT 0,
        -- Within the last week; older failures stop counting.
        failures             INTEGER NOT NULL DEFAULT 0,
        -- The run the server is on, ended by a success.
        consecutive_failures INTEGER NOT NULL DEFAULT 0,
        last_success_at      INTEGER,
        last_failure_at      INTEGER,
        -- `http-502`, `load`, `no-frames` or `stalled`.
        last_failure         TEXT,
        -- Smoothed over the plays it served.
        first_frame_ms       REAL,
        -- When it was last tried. A server not tried for 30 days is deleted.
        seen_at              INTEGER NOT NULL,
        PRIMARY KEY (provider_id, origin)
    );
    CREATE INDEX idx_server_health_seen ON server_health(seen_at);
    "#,
    ),
    (
        "0008_library_status",
        r#"
    -- Where each title is on the signed-in tracker's list, as its last library pull or Library
    -- load found it: what a related title's card says ("Watched", "Planning") without asking the
    -- tracker again. A copy of the tracker's answer, replaced whole by each new one, and only
    -- ever the list the Library screen shows.
    --
    -- The status is the tracker's word for the list: CURRENT, PLANNING, COMPLETED, PAUSED or
    -- DROPPED.
    CREATE TABLE library_status (
        anilist_id INTEGER PRIMARY KEY,
        status     TEXT    NOT NULL,
        tracker_id TEXT    NOT NULL,
        updated_at INTEGER NOT NULL
    );
    "#,
    ),
    (
        "0009_episode_notices",
        r#"
    -- What the new-episode check has said of each show it follows, so each episode is said
    -- once: the newest episode said, 0 before any, and when the show was first followed. An
    -- episode that aired before then is never said, which is what keeps a first check, or a
    -- show newly followed, from announcing a backlog.
    --
    -- How far the check has looked is kept in app_state, under `episode_check_through`.
    CREATE TABLE episode_notice (
        anilist_id INTEGER PRIMARY KEY,
        episode    INTEGER NOT NULL,
        since      INTEGER NOT NULL,
        updated_at INTEGER NOT NULL
    );
    "#,
    ),
    (
        "0010_airing_times",
        r#"
    -- When each episode airs, as this device last saw AniList's schedule, and when it was to air
    -- before the schedule moved it later: what lets a calendar row say "Delayed (was 23:00)".
    -- AniList keeps no record of a move, only the time as it stands, so the earlier time is only
    -- known here, and only when this device saw it before it moved.
    --
    -- `was` is the time the episode stood at before it moved later, kept while it stands later
    -- than that by more than a nudge, and cleared once it is back. Rows not seen for 120 days are
    -- deleted.
    CREATE TABLE airing_time (
        anilist_id INTEGER NOT NULL,
        episode    INTEGER NOT NULL,
        airs_at    INTEGER NOT NULL,
        was        INTEGER,
        seen_at    INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, episode)
    );
    CREATE INDEX idx_airing_time_seen ON airing_time(seen_at);
    "#,
    ),
    (
        "0011_episode_notice_dub",
        r#"
    -- For a show watched dubbed, the newest episode whose dub the new-episode check has said, or
    -- let go. AniList dates only the original broadcast, so a dub is said once the source a play
    -- would use lists it, which the check asks only while a broadcast is ahead of this. NULL for a
    -- show watched subtitled, and for one watched dubbed until its first broadcast since it was
    -- followed, which sets where its dub is said from.
    ALTER TABLE episode_notice ADD COLUMN dubbed INTEGER;
    "#,
    ),
    (
        // Numbered after the merge train's 0009 and 0010, and native-layer/notify-follow-ups'
        // 0011, which this lands beside.
        "0012_download_ahead",
        r#"
    -- Whether keep next queued a download, to have a show's next episodes on the device, rather
    -- than the viewer asking for it. The Downloads row says so, and nothing deletes a download
    -- the viewer asked for on its own. Asking for one keep next queued makes it the viewer's.
    ALTER TABLE download ADD COLUMN ahead INTEGER NOT NULL DEFAULT 0;

    -- The episodes the viewer let go of from the queue: cancelled, deleted, or cleared from the
    -- list once finished. Keep next never queues one again, so it does not fight the viewer;
    -- asking for the episode by hand takes it off this list.
    CREATE TABLE download_dismissed (
        anilist_id INTEGER NOT NULL,
        episode    TEXT    NOT NULL,
        at         INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, episode)
    );
    "#,
    ),
    (
        "0013_download_front",
        r#"
    -- When the viewer asked a waiting download to start now, which puts it at the front of the
    -- queue: the latest asked first, then the rest oldest first. NULL for one never asked.
    ALTER TABLE download ADD COLUMN front_at INTEGER;
    "#,
    ),
    (
        "0014_release_preference",
        r#"
    -- A release pinned within the pinned source, for a source that offers several (a torrent
    -- source, with a batch and single releases for one show). The id is the source's own and
    -- means nothing to another. NULL for a source pinned as a whole.
    ALTER TABLE provider_preference ADD COLUMN release_id    TEXT;
    ALTER TABLE provider_preference ADD COLUMN release_title TEXT;
    "#,
    ),
    (
        "0015_match_title",
        r#"
    -- The source's own name for the entry a title was matched to, as it was when matched, so a
    -- page can say what the title is matched to without asking the source again. NULL for a
    -- match made before this was kept.
    ALTER TABLE mapping_override   ADD COLUMN title TEXT;
    ALTER TABLE mapping_resolution ADD COLUMN title TEXT;
    "#,
    ),
    (
        "0016_mapping_external_ids",
        r#"
    -- Media servers and the library know a title by its AniDB or TMDB id, so those are looked up
    -- the way MAL and TVDB ids are.
    CREATE INDEX IF NOT EXISTS idx_mapping_anidb ON mapping(anidb_id);
    CREATE INDEX IF NOT EXISTS idx_mapping_tmdb  ON mapping(tmdb_id);
    "#,
    ),
    (
        "0018_skip_marks",
        r#"
    -- Where the viewer marked an episode's opening, ending or recap, one row per kind, the
    -- latest mark kept. `kind` is AniSkip's name for it: op, ed or recap. The episode's length
    -- when marked is kept, so a mark is only used on a file of about that length. `skip_id` is
    -- the AniSkip time the mark was sent as, once it has been.
    CREATE TABLE skip_marks (
        anilist_id INTEGER NOT NULL,
        episode    TEXT    NOT NULL,
        kind       TEXT    NOT NULL,
        start_ms   INTEGER NOT NULL,
        end_ms     INTEGER NOT NULL,
        length_ms  INTEGER NOT NULL,
        marked_at  INTEGER NOT NULL,
        skip_id    TEXT,
        PRIMARY KEY (anilist_id, episode, kind)
    );

    -- The viewer's vote on an AniSkip time, by its id: -1 for a report, 1 for a mark that
    -- matched it. A time reported here is not used on this device again.
    CREATE TABLE skip_votes (
        skip_id TEXT    PRIMARY KEY,
        vote    INTEGER NOT NULL,
        at      INTEGER NOT NULL
    );

    -- Made in the first schema for AniSkip's times and never written to: they are asked for
    -- at play instead.
    DROP TABLE episode_meta;
    "#,
    ),
    (
        "0019_aniskip_outbox",
        r#"
    -- What waits to be sent to AniSkip, kept only while sharing is on: a mark to submit or a
    -- vote, as JSON. A submission names the mark it came from, so the id AniSkip answers with
    -- is kept on it. `due_at` is when to try next; `failed` is set once it is let go.
    CREATE TABLE aniskip_outbox (
        id         INTEGER PRIMARY KEY AUTOINCREMENT,
        request    TEXT    NOT NULL,
        anilist_id INTEGER,
        episode    TEXT,
        kind       TEXT,
        attempts   INTEGER NOT NULL DEFAULT 0,
        due_at     INTEGER NOT NULL,
        failed     INTEGER NOT NULL DEFAULT 0
    );
    CREATE INDEX aniskip_outbox_due ON aniskip_outbox (failed, due_at);
    "#,
    ),
    (
        "0020_party_room",
        r#"
    -- The last ten watch party rooms this device was in: where the room is, what it watched, the
    -- last episode it reached and who was there, one per line. Only names and titles, never a
    -- source or a stream.
    CREATE TABLE party_room (
        server    TEXT    NOT NULL,
        room      TEXT    NOT NULL,
        anilist_id INTEGER NOT NULL,
        title     TEXT    NOT NULL,
        episode   TEXT    NOT NULL,
        names     TEXT    NOT NULL DEFAULT '',
        cover_url TEXT,
        last_at   INTEGER NOT NULL,
        PRIMARY KEY (server, room)
    );
    "#,
    ),
    (
        "0021_sync_log",
        r#"
    -- Cross-device sync. Nothing here is written until `Store::sync_enable` has run, so an
    -- install that never turns sync on carries empty tables and the two event columns.
    ALTER TABLE watch_event ADD COLUMN uid   TEXT;
    ALTER TABLE watch_event ADD COLUMN stamp INTEGER;
    CREATE UNIQUE INDEX idx_event_uid ON watch_event(uid) WHERE uid IS NOT NULL;

    CREATE TABLE sync_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    -- Every change this group has seen: one device's ops are numbered from 1 with no gaps.
    -- `body` is JSON; compaction rewrites a superseded body to a no-op and keeps the number.
    CREATE TABLE sync_op (
        device   TEXT    NOT NULL,
        seq      INTEGER NOT NULL,
        stamp    INTEGER NOT NULL,
        body     TEXT    NOT NULL,
        received INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (device, seq)
    ) WITHOUT ROWID;

    -- Last-writer-wins registers: the winning stamp and writer, and the value when it is not
    -- held in a table of its own. A `NULL` value is a cleared register.
    CREATE TABLE sync_reg (
        kind   TEXT    NOT NULL,
        key    TEXT    NOT NULL,
        stamp  INTEGER NOT NULL,
        device TEXT    NOT NULL,
        value  TEXT,
        PRIMARY KEY (kind, key)
    ) WITHOUT ROWID;

    -- Events an unwatch removed, kept so that the event arriving later is not put back.
    CREATE TABLE sync_tomb (
        uid   TEXT PRIMARY KEY,
        stamp INTEGER NOT NULL
    ) WITHOUT ROWID;

    -- The newest unwatch of an episode and who wrote it, for noticing a kept watch.
    CREATE TABLE sync_unwatch (
        anilist_id INTEGER NOT NULL,
        episode    TEXT    NOT NULL,
        stamp      INTEGER NOT NULL,
        device     TEXT    NOT NULL,
        PRIMARY KEY (anilist_id, episode)
    ) WITHOUT ROWID;

    CREATE TABLE sync_mark (
        tracker    TEXT    NOT NULL,
        anilist_id INTEGER NOT NULL,
        episode    INTEGER NOT NULL,
        PRIMARY KEY (tracker, anilist_id)
    ) WITHOUT ROWID;

    CREATE TABLE sync_peer (
        device    TEXT PRIMARY KEY,
        name      TEXT,
        pubkey    TEXT,
        removed   INTEGER NOT NULL DEFAULT 0,
        trackers  TEXT    NOT NULL DEFAULT '[]',
        name_stamp INTEGER NOT NULL DEFAULT 0,
        added_at  INTEGER,
        last_seen INTEGER
    ) WITHOUT ROWID;

    -- What other devices hold downloaded: facts only they can state.
    CREATE TABLE sync_remote_download (
        device     TEXT    NOT NULL,
        anilist_id INTEGER NOT NULL,
        episode    TEXT    NOT NULL,
        done       INTEGER NOT NULL,
        stamp      INTEGER NOT NULL,
        PRIMARY KEY (device, anilist_id, episode)
    ) WITHOUT ROWID;

    -- A watch kept against an unwatch it had not seen: surfaced for seven days.
    CREATE TABLE sync_note (
        id             INTEGER PRIMARY KEY AUTOINCREMENT,
        anilist_id     INTEGER NOT NULL,
        episode        TEXT    NOT NULL,
        unwatch_device TEXT    NOT NULL,
        watch_device   TEXT    NOT NULL,
        at             INTEGER NOT NULL,
        UNIQUE (anilist_id, episode, unwatch_device, watch_device)
    );
    "#,
    ),
    (
        "0022_dub_extent",
        r#"
    -- What this device's sources said of a title's dub, kept so a card can say "Dub 8" after a
    -- restart without asking. One row per title and the sources asked (`route`).
    -- `has_dub` is 0 for a source that listed the title and found
    -- no dub. `through` is null for a dub with no count. `until` is when the answer is due to
    -- be asked again; the row stays and is still shown past it, as a dub does not take an
    -- episode back.
    CREATE TABLE dub_extent (
        anilist_id INTEGER NOT NULL,
        route      TEXT    NOT NULL,
        has_dub    INTEGER NOT NULL,
        through    INTEGER,
        complete   INTEGER NOT NULL DEFAULT 0,
        aired      INTEGER,
        said_at    INTEGER NOT NULL,
        until      INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, route)
    );
    CREATE INDEX dub_extent_said ON dub_extent (said_at);
    "#,
    ),
    (
        "0023_skip_detect",
        r#"
    -- Skip times found from the audio. Fingerprints of an episode's head (its first six
    -- minutes) or tail (its last four), as little-endian chromaprint items from `start_ms` on,
    -- for the last three episodes of a show, never shared. Kept on this device only.
    CREATE TABLE skip_prints (
        anilist_id INTEGER NOT NULL,
        episode    TEXT    NOT NULL,
        part       TEXT    NOT NULL,
        length_ms  INTEGER NOT NULL,
        start_ms   INTEGER NOT NULL,
        items      BLOB    NOT NULL,
        taken_at   INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, episode, part)
    );

    -- A stretch found in an episode, by AniSkip's kind (op or ed), with the episodes whose
    -- prints agreed and what became of it: found, skipped, confirmed, rejected or shared.
    CREATE TABLE skip_found (
        anilist_id INTEGER NOT NULL,
        episode    TEXT    NOT NULL,
        kind       TEXT    NOT NULL,
        start_ms   INTEGER NOT NULL,
        end_ms     INTEGER NOT NULL,
        length_ms  INTEGER NOT NULL,
        agreed     TEXT    NOT NULL DEFAULT '',
        state      TEXT    NOT NULL,
        found_at   INTEGER NOT NULL,
        PRIMARY KEY (anilist_id, episode, kind)
    );

    -- Per show and kind: found stretches skipped without an undo since the last undo, whether
    -- the person confirmed one, and whether they were asked.
    CREATE TABLE skip_found_show (
        anilist_id INTEGER NOT NULL,
        kind       TEXT    NOT NULL,
        skips      INTEGER NOT NULL DEFAULT 0,
        confirmed  INTEGER NOT NULL DEFAULT 0,
        asked      INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY (anilist_id, kind)
    );
    "#,
    ),
    (
        "0024_read_progress",
        r#"
    -- Reading, like watching: an append-only log of what was read (`read_event`, the source of
    -- truth), and projections of it per chapter and per work. `work` and `chapter` are the
    -- engine's WorkId and ChapterId strings: `a:<anilist id>` or `l:<hex>` for a work, a chapter
    -- key such as `12`, `10.5`, `v2/3` or `x/-/Extra` for a chapter. `uid` is `device:seq` of
    -- the op that carried the event, null while sync is off or for a work only this device has.
    CREATE TABLE read_event (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        uid           TEXT,
        stamp         INTEGER,
        work          TEXT    NOT NULL,
        chapter       TEXT    NOT NULL,
        page          INTEGER,
        of_pages      INTEGER,
        strip_image   INTEGER,
        strip_offset  REAL,
        novel_pos     TEXT,
        completed     INTEGER NOT NULL DEFAULT 0,
        source_id     TEXT,
        version_label TEXT,
        at            INTEGER NOT NULL
    );
    CREATE INDEX idx_read_event_work ON read_event (work, at DESC);
    CREATE UNIQUE INDEX idx_read_event_uid ON read_event (uid) WHERE uid IS NOT NULL;

    -- Every chapter at or below a watermark counts as read: a tracker's count adopted, or
    -- Mark as read up to here. Not written per chapter, as numbers can be decimals and a source
    -- can list more chapters later.
    CREATE TABLE read_adopt (
        uid          TEXT    PRIMARY KEY,
        work         TEXT    NOT NULL,
        upto_chapter TEXT    NOT NULL,
        upto_milli   INTEGER NOT NULL,
        upto_volume  TEXT,
        stamp        INTEGER NOT NULL,
        at           INTEGER NOT NULL
    );
    CREATE INDEX idx_read_adopt_work ON read_adopt (work);

    -- Read again from the start: events and watermarks older than a reset are set aside, and
    -- come back when it is undone (`undone` is the stamp of the undo).
    CREATE TABLE read_reset (
        uid    TEXT    PRIMARY KEY,
        work   TEXT    NOT NULL,
        stamp  INTEGER NOT NULL,
        at     INTEGER NOT NULL,
        undone INTEGER
    );
    CREATE INDEX idx_read_reset_work ON read_reset (work);

    -- Per chapter, from the events since the newest reset: whether it was read, and the newest
    -- place in it with the stamp and device that wrote it.
    CREATE TABLE read_chapter (
        work         TEXT    NOT NULL,
        chapter      TEXT    NOT NULL,
        read         INTEGER NOT NULL DEFAULT 0,
        read_at      INTEGER,
        last_page    INTEGER,
        of_pages     INTEGER,
        strip_image  INTEGER,
        strip_offset REAL,
        novel_pos    TEXT,
        stamp        INTEGER NOT NULL,
        device       TEXT,
        PRIMARY KEY (work, chapter)
    ) WITHOUT ROWID;

    -- Per work, for Continue: the newest chapter, the reset and the watermark in force, and
    -- whether it is hidden from Continue.
    CREATE TABLE read_work (
        work         TEXT    PRIMARY KEY,
        last_chapter TEXT,
        stamp        INTEGER NOT NULL DEFAULT 0,
        reset_stamp  INTEGER NOT NULL DEFAULT 0,
        adopt_milli  INTEGER,
        adopt_volume TEXT,
        hidden       INTEGER NOT NULL DEFAULT 0,
        updated_at   INTEGER NOT NULL
    ) WITHOUT ROWID;
    CREATE INDEX idx_read_work_recent ON read_work (stamp DESC);

    -- A series' own reader choices, as `show_setting` keeps a show's. `scope` is `series` for a
    -- choice that follows the person (mode, direction, colour) or `here` for one that stays on
    -- this device (crop). Names this version does not know are passed over.
    CREATE TABLE read_series_setting (
        work       TEXT    NOT NULL,
        setting    TEXT    NOT NULL,
        value      TEXT    NOT NULL,
        scope      TEXT    NOT NULL,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (work, setting)
    ) WITHOUT ROWID;

    CREATE TABLE read_status (
        work       TEXT    PRIMARY KEY,
        status     TEXT,
        score      REAL,
        updated_at INTEGER NOT NULL
    ) WITHOUT ROWID;

    CREATE TABLE read_bookmark (
        work    TEXT    NOT NULL,
        chapter TEXT    NOT NULL,
        at      INTEGER NOT NULL,
        PRIMARY KEY (work, chapter)
    ) WITHOUT ROWID;

    -- What was sent to an e-reader, so the series page can ask where the person stopped.
    -- `chapters` is a JSON list of chapter ids.
    CREATE TABLE read_sent (
        id       INTEGER PRIMARY KEY AUTOINCREMENT,
        work     TEXT    NOT NULL,
        chapters TEXT    NOT NULL,
        target   TEXT    NOT NULL,
        at       INTEGER NOT NULL,
        answered INTEGER NOT NULL DEFAULT 0
    );
    "#,
    ),
    (
        "0025_read_trackers",
        r#"
    -- Reading's tracker writes, with the columns of `sync_outbox`, kept apart so a version that
    -- drains only watching never sees them.
    CREATE TABLE read_outbox (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        tracker_id  TEXT    NOT NULL,
        op          TEXT    NOT NULL,
        anilist_id  INTEGER NOT NULL,
        created_at  INTEGER NOT NULL,
        attempts    INTEGER NOT NULL DEFAULT 0,
        next_retry  INTEGER NOT NULL DEFAULT 0,
        last_error  TEXT
    );
    CREATE INDEX idx_read_outbox_ready ON read_outbox (tracker_id, next_retry);

    -- Which entry a work is on each tracker. `tracker_id` is text so a 64-bit id fits.
    -- `origin` is where it came from (anilist, kitsu, mangabaka, search, hand); a choice made
    -- by hand is never replaced by a lookup. A null `tracker_id` is "not on this tracker".
    CREATE TABLE read_track_map (
        work       TEXT    NOT NULL,
        tracker    TEXT    NOT NULL,
        tracker_id TEXT,
        origin     TEXT    NOT NULL,
        checked_at INTEGER NOT NULL,
        PRIMARY KEY (work, tracker)
    ) WITHOUT ROWID;

    -- The newest chapter a notification was sent for, per work.
    CREATE TABLE read_notice (
        work        TEXT    NOT NULL,
        chapter     TEXT    NOT NULL,
        notified_at INTEGER NOT NULL,
        PRIMARY KEY (work, chapter)
    ) WITHOUT ROWID;

    -- The highest chapter and volume a tracker is known to hold for a work: a max register.
    CREATE TABLE read_mark (
        tracker    TEXT    NOT NULL,
        anilist_id INTEGER NOT NULL,
        chapter    INTEGER NOT NULL,
        volume     INTEGER,
        PRIMARY KEY (tracker, anilist_id)
    ) WITHOUT ROWID;
    "#,
    ),
    (
        "0026_book_catalog",
        r#"
    -- What this device knows of a work, as AniList or a source described it. `titles` and
    -- `meta` are JSON.
    CREATE TABLE book_work (
        work           TEXT    PRIMARY KEY,
        anilist_id     INTEGER,
        medium         TEXT    NOT NULL,
        format         TEXT,
        origin         TEXT,
        titles         TEXT    NOT NULL,
        cover          TEXT,
        status         TEXT,
        chapters_total INTEGER,
        volumes_total  INTEGER,
        meta           TEXT,
        fetched_at     INTEGER NOT NULL
    ) WITHOUT ROWID;

    -- When a work's chapters usually come out, learnt from the chapters seen.
    CREATE TABLE book_rhythm (
        work           TEXT    PRIMARY KEY,
        weekday        INTEGER,
        every_days     INTEGER,
        last_published INTEGER,
        checked_at     INTEGER NOT NULL
    ) WITHOUT ROWID;

    -- A source's chapter list for a series, as it last answered. `chapters` is JSON.
    CREATE TABLE book_chapter_cache (
        source_id  TEXT    NOT NULL,
        series_ref TEXT    NOT NULL,
        chapters   TEXT    NOT NULL,
        fetched_at INTEGER NOT NULL,
        PRIMARY KEY (source_id, series_ref)
    ) WITHOUT ROWID;

    -- Which work a source's series is, as `mapping_override` keeps a show's match.
    CREATE TABLE book_source_match (
        source_id  TEXT    NOT NULL,
        series_ref TEXT    NOT NULL,
        work       TEXT    NOT NULL,
        confidence INTEGER NOT NULL DEFAULT 0,
        confirmed  INTEGER NOT NULL DEFAULT 0,
        updated_at INTEGER NOT NULL,
        PRIMARY KEY (source_id, series_ref)
    ) WITHOUT ROWID;

    -- The edition a work is read from, as `provider_preference` keeps a show's source.
    CREATE TABLE book_edition_pref (
        work          TEXT    PRIMARY KEY,
        source_id     TEXT    NOT NULL,
        version_label TEXT,
        language      TEXT
    ) WITHOUT ROWID;
    "#,
    ),
    (
        "0027_book_sources",
        r#"
    -- Book folders. A folder is a path, an Android document tree or an Apple bookmark.
    CREATE TABLE book_folder (
        id        TEXT    PRIMARY KEY,
        path      TEXT,
        tree_uri  TEXT,
        bookmark  BLOB,
        name      TEXT,
        added_at  INTEGER NOT NULL,
        last_scan INTEGER,
        available INTEGER NOT NULL DEFAULT 1
    ) WITHOUT ROWID;

    -- A folder's books grouped as series, and what each was matched to.
    CREATE TABLE book_local_series (
        id         TEXT    PRIMARY KEY,
        folder     TEXT    NOT NULL,
        name       TEXT    NOT NULL,
        medium     TEXT    NOT NULL,
        work       TEXT,
        state      TEXT    NOT NULL,
        confidence INTEGER NOT NULL DEFAULT 0,
        reason     TEXT
    ) WITHOUT ROWID;

    -- One book file. `number_milli` is the chapter number in thousandths; `comicinfo` is JSON.
    CREATE TABLE book_local_file (
        path         TEXT    PRIMARY KEY,
        series       TEXT    NOT NULL,
        volume       TEXT,
        label        TEXT    NOT NULL,
        number_milli INTEGER,
        format       TEXT    NOT NULL,
        pages        INTEGER,
        size         INTEGER NOT NULL,
        mtime        INTEGER NOT NULL,
        comicinfo    TEXT
    ) WITHOUT ROWID;
    CREATE INDEX idx_book_local_file_series ON book_local_file (series);

    -- Book servers. The token is in the secure store under `book-server/<id>`, never here.
    CREATE TABLE book_server (
        id              TEXT    PRIMARY KEY,
        kind            TEXT    NOT NULL,
        name            TEXT    NOT NULL,
        address         TEXT    NOT NULL,
        user            TEXT,
        report_progress INTEGER NOT NULL DEFAULT 1,
        signed_out      INTEGER NOT NULL DEFAULT 0,
        last_seen       INTEGER
    ) WITHOUT ROWID;
    "#,
    ),
    (
        "0028_book_downloads",
        r#"
    -- Chapters downloaded or queued. `keep_ready` marks one fetched ahead on its own, which
    -- reading past it lets go.
    CREATE TABLE book_download (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        work          TEXT    NOT NULL,
        chapter       TEXT    NOT NULL,
        source_id     TEXT    NOT NULL,
        version_label TEXT,
        state         TEXT    NOT NULL,
        path          TEXT,
        pages_done    INTEGER NOT NULL DEFAULT 0,
        pages_total   INTEGER,
        bytes         INTEGER NOT NULL DEFAULT 0,
        error         TEXT,
        keep_ready    INTEGER NOT NULL DEFAULT 0,
        created_at    INTEGER NOT NULL,
        updated_at    INTEGER NOT NULL,
        UNIQUE (work, chapter)
    );
    "#,
    ),
    (
        "0029_read_unread",
        r#"
    -- The newest unread of a chapter and the device that wrote it ('' with sync off), as
    -- `sync_unwatch` keeps an episode's. It tells a read that arrives later from one that saw
    -- the unread, and keeps a chapter a watermark covers unread until it is read again.
    CREATE TABLE read_unread (
        work    TEXT    NOT NULL,
        chapter TEXT    NOT NULL,
        stamp   INTEGER NOT NULL,
        device  TEXT    NOT NULL DEFAULT '',
        PRIMARY KEY (work, chapter)
    ) WITHOUT ROWID;

    -- A finished chapter kept against an unread its reading device had not seen, as `sync_note`
    -- keeps a watch: surfaced for seven days.
    CREATE TABLE read_note (
        id            INTEGER PRIMARY KEY AUTOINCREMENT,
        work          TEXT    NOT NULL,
        chapter       TEXT    NOT NULL,
        unread_device TEXT    NOT NULL,
        read_device   TEXT    NOT NULL,
        at            INTEGER NOT NULL,
        UNIQUE (work, chapter, unread_device, read_device)
    );
    "#,
    ),
    (
        "0030_book_progress_outbox",
        r#"
    -- Reading progress waiting to be told to a book server: made offline, or tried and not
    -- taken yet. One row per chapter of a series on a server, holding the newest mark; a mark
    -- the server took is deleted. `due_at` is when to try next. `held` names why a row waits
    -- for Try again instead (a refused key, the series gone, no answer for a day), and
    -- `problem` why the last try failed. `first_failed_at` starts the day of retries.
    CREATE TABLE book_progress_outbox (
        server          TEXT    NOT NULL,
        series          TEXT    NOT NULL,
        chapter         TEXT    NOT NULL,
        read            INTEGER NOT NULL,
        at              INTEGER NOT NULL,
        attempts        INTEGER NOT NULL DEFAULT 0,
        due_at          INTEGER NOT NULL,
        first_failed_at INTEGER,
        problem         TEXT,
        held            TEXT,
        PRIMARY KEY (server, series, chapter)
    ) WITHOUT ROWID;
    CREATE INDEX book_progress_outbox_due ON book_progress_outbox (held, due_at);

    -- How writing progress back to each server last went: when it last took a mark, and why
    -- the last try failed while none has been taken since.
    CREATE TABLE book_progress_state (
        server     TEXT PRIMARY KEY,
        sent_at    INTEGER,
        problem    TEXT,
        problem_at INTEGER
    ) WITHOUT ROWID;
    "#,
    ),
];

/// Apply any migrations the database has not yet seen.
pub fn migrate(conn: &Connection) -> rusqlite::Result<u32> {
    migrate_until(conn, None)
}

/// [`migrate`], stopping before the migration called `stop`: a database as an older release left
/// it, for the tests of what a migration does to existing data.
pub fn migrate_until(conn: &Connection, stop: Option<&str>) -> rusqlite::Result<u32> {
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;

    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migration (
            name       TEXT PRIMARY KEY,
            applied_at INTEGER NOT NULL
        );",
    )?;

    let mut applied = 0;
    for (name, sql) in MIGRATIONS {
        if Some(*name) == stop {
            break;
        }
        let already: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migration WHERE name = ?1)",
            [name],
            |r| r.get(0),
        )?;
        if already {
            continue;
        }
        // Each migration is one transaction, **and the record of it is inside that
        // transaction**. Committing the schema change and then recording it separately leaves a
        // window in which the DDL is durable and the bookkeeping is not: lose power there and
        // the next launch re-runs a migration that has already been applied, fails on
        // `table … already exists`, and `Store::open` returns an error every time from then on.
        // There is no repair path for that short of deleting the database, so the two facts
        // have to become durable together.
        conn.execute_batch(&format!(
            "BEGIN;\n\
             {sql}\n\
             INSERT INTO schema_migration (name, applied_at) VALUES ('{name}', unixepoch());\n\
             COMMIT;"
        ))?;
        tracing::info!(migration = name, "applied migration");
        applied += 1;
    }
    Ok(applied)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_migration_and_the_record_of_it_commit_together() {
        // Every migration name is a bare identifier, which is what makes interpolating it into
        // the batch safe. Asserted rather than assumed, because the day one arrives with an
        // apostrophe in it is the day the batch stops parsing — or worse, stops meaning what it
        // says.
        for (name, _) in MIGRATIONS {
            assert!(
                name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'),
                "{name} is not a bare identifier"
            );
        }

        // The bookkeeping row must be visible to the same transaction the DDL ran in, so a
        // crash between the two cannot leave a schema that claims never to have been migrated.
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let recorded: i64 =
            conn.query_row("SELECT COUNT(*) FROM schema_migration", [], |r| r.get(0)).unwrap();
        assert_eq!(recorded as usize, MIGRATIONS.len(), "every migration recorded itself");
    }

    #[test]
    fn a_lost_bookkeeping_row_would_brick_the_database() {
        // The failure the transaction prevents, demonstrated: with the record missing, the
        // migration re-runs and fails on a table that already exists. This is what a power cut
        // between the two statements used to produce — permanently, on every launch.
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let (first, _) = MIGRATIONS[0];
        conn.execute("DELETE FROM schema_migration WHERE name = ?1", [first]).unwrap();

        assert!(
            migrate(&conn).is_err(),
            "re-running an applied migration should fail — which is why the record must be \
             durable with it, not after it"
        );
    }

    #[test]
    fn migrations_apply_once_and_are_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        assert_eq!(migrate(&conn).unwrap(), MIGRATIONS.len() as u32);
        // Running again must be a no-op, not an error — this is what happens on every
        // subsequent startup.
        assert_eq!(migrate(&conn).unwrap(), 0);
    }

    #[test]
    fn every_expected_table_exists_after_migration() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        for table in [
            "media_cache",
            "mapping",
            "episode_relation",
            "dataset_state",
            "mapping_override",
            "mapping_resolution",
            "watch_event",
            "watch_progress",
            "sync_outbox",
            "provider_preference",
            "track_memory",
            "show_setting",
            "server_health",
            "library_status",
            "episode_notice",
            "airing_time",
            "download_dismissed",
            "skip_marks",
            "skip_votes",
            "aniskip_outbox",
            "party_room",
            "read_event",
            "read_adopt",
            "read_reset",
            "read_chapter",
            "read_work",
            "read_series_setting",
            "read_status",
            "read_bookmark",
            "read_sent",
            "read_outbox",
            "read_track_map",
            "read_notice",
            "read_mark",
            "book_work",
            "book_rhythm",
            "book_chapter_cache",
            "book_source_match",
            "book_edition_pref",
            "book_folder",
            "book_local_series",
            "book_local_file",
            "book_server",
            "book_download",
            "read_unread",
            "read_note",
            "book_progress_outbox",
            "book_progress_state",
        ] {
            let found: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1)",
                    [table],
                    |r| r.get(0),
                )
                .unwrap();
            assert!(found, "table {table} missing");
        }
    }

    #[test]
    fn migration_names_are_unique() {
        let mut names: Vec<&str> = MIGRATIONS.iter().map(|(n, _)| *n).collect();
        let count = names.len();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), count, "duplicate migration name");
    }
}

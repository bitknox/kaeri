//! Settings: each one a front end shows or changes, named for what it is; the rules for what a
//! row shows and what a step makes of it; and what a front end is told of the config.
//!
//! The engine owns the config and runs these rules over it. A front end asks for a change with
//! a command and is told the outcome as a [`SettingsView`]: the config itself never crosses.

use kaeri_core::{
    config::{
        Config, CountsAsRead, NewChapters, SendProgressAfter, StyledSubtitles,
        SubtitleBackground, SubtitleColour, SubtitleEdge, SubtitleFont, SubtitleThickness,
        SubtitlesConfig, ThemeMode, TitleChoice, TrailerAutoplay, WebtoonMode,
        WhenTrackersDisagree,
    },
    media::Translation,
    settings::SettingValue,
};

use crate::sources::ConfiguredSources;

/// Every setting a front end shows or changes, named for what it is rather than for where the
/// file keeps it: the Settings screen's rows, in display order in [`Self::ROWS`]; the native
/// apps' own, in [`Self::NATIVE_ROWS`]; and the one with no row on either, the source order.
///
/// An enum rather than positional indices: the cycling logic and the renderer both key off this,
/// so inserting a row cannot make the screen edit the setting below the one it is showing. And
/// rather than the file's table and key, which are where it is written and nothing more.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingKey {
    Theme,
    Motion,
    /// Which form of each title is shown: the signed-in AniList account's own choice, English
    /// or romaji. One setting for every front end, which shows what the engine names.
    Titles,
    /// When a title's trailer plays by itself in place of its banner: always, on Wi-Fi only, or
    /// never. The terminal shows it and plays none. See [`crate::trailer::autoplay`].
    Trailers,
    Translation,
    Quality,
    Subtitles,
    CommitThreshold,
    AutoNext,
    SkipOpening,
    /// Skip a recap by itself. Off by default, apart from the opening's own switch.
    SkipRecap,
    /// Ask "Was that the opening?" after a skip by hand of about an opening's length near the
    /// start of an episode with no opening known. On by default; its Don't ask again turns it
    /// off.
    AskWasOpening,
    /// Find openings and endings from the audio when AniSkip has none, on this device: the
    /// native apps' row, where the player can hand the audio over. On by default.
    FindSkipTimes,
    /// Share the skip times the viewer marks with AniSkip. Unset until the first Save asks,
    /// which reads as off.
    ShareSkipTimes,
    SkipFiller,
    MarkChapters,
    /// Offer a skip button while the opening or ending plays, where the skip is not automatic.
    SkipButton,
    Fullscreen,
    Upscaling,
    PlayerUi,
    NightMode,
    DownloadDir,
    MergeSubtitles,
    KeepSeeding,
    DownloadHook,
    Presence,
    PresenceTitle,
    /// Discord's "Watching" with the cover, a progress bar, a link to AniList and the player's
    /// state. Needs the title shown, since every part of it names the show.
    PresenceRich,
    Torrents,
    /// The proxy torrent traffic goes through, or none. Shown, not written from here: the
    /// native apps set it with [`Command::SetTorrentProxy`], the terminal in the file.
    ///
    /// [`Command::SetTorrentProxy`]: crate::command::Command::SetTorrentProxy
    TorrentProxy,
    SeedRatio,
    SeedHours,
    UploadLimit,
    TokenStorage,
    Syncplay,
    SyncplayServer,
    SyncplayRoom,
    UpdateCheck,
    /// Whether the update check follows the beta channel. Off by default.
    UpdateBeta,
    /// The order failover walks the sources in. No row on Settings: `K` and `J` on Providers
    /// move a source up and down it.
    SourceOrder,
    /// How many seconds Up Next counts down before the next episode, `0` for none: a native
    /// app's row, as are the rest below.
    UpNextCountdown,
    /// Blur the stills of unwatched episodes after the next one.
    SpoilerProtection,
    /// Which version of an episode a download takes: standard or higher.
    DownloadQuality,
    /// How many of a show's next episodes downloads keep, `0` for none.
    KeepNextEpisodes,
    /// Whether downloads run over a cellular connection.
    DownloadsOnCellular,
    /// Whether chapter downloads run over a cellular connection.
    ChapterDownloadsOnCellular,
    /// Whether this computer serves its downloads and folders as an OPDS catalogue on the home
    /// network, while Kaeri runs. Desktop builds only.
    OpdsFeed,
    /// The password e-readers sign in to the feed with; never shown back.
    OpdsPassword,
    /// The port the feed listens on; empty picks a free one.
    OpdsPort,
    /// The switch form of [`Self::NewChapters`], for a front end that draws one: on is the
    /// Reading list, off is off. Written as `reading.new_chapters`.
    KeepChaptersReady,
    /// Whether a downloaded chapter is deleted once it is read; bookmarked chapters stay.
    DeleteAfterReading,
    /// Whether a series' own mode and direction follow the person to their other devices. On
    /// by default. Set by the switch "Mode and direction".
    SeriesChoicesFollow,
    /// Whether a series' own look (its colour) follows the person to their other devices. On
    /// by default.
    LookFollows,
    /// Which series' new chapters download by themselves: `off` or `reading` (the Reading
    /// list). Unset follows the build: the Reading list in the e-reader edition, off elsewhere.
    NewChapters,
    /// How a webtoon opens when the series has no mode of its own: `strip` or `paged`.
    WebtoonMode,
    /// When chapter progress goes to the reading trackers: `chapter` (as each is read) or
    /// `close` (once, when the reader closes).
    SendProgressAfter,
    /// Which chapter progress wins when a tracker and this device disagree: `furthest` or
    /// `latest`. Status and score always follow the latest change.
    WhenTrackersDisagree,
    /// When the reader counts a chapter as read, which Continue, the read marks and the
    /// trackers follow: `last-page` (a short stay on it, or moving past it), `moving-on` (only
    /// moving past it) or `marked` (only the person's own marks).
    CountsAsRead,
    /// Whether the app notifies of new chapters in the series on the Reading list, on its own
    /// channel. Off by default; the check runs only while reading is on.
    ChapterNotifications,
    /// Whether the app notifies when the chapters it was downloading are all done. Off by
    /// default.
    DownloadsFinished,
    /// Which Home the app opens at while it is for watching and reading: `watch` or `read`.
    OpenAt,
    /// Whether torrents run over a cellular connection.
    TorrentsOnCellular,
    /// Whether the app notifies of new episodes of the shows being watched, from
    /// [`Command::CheckNewEpisodes`]. Off by default. A show can turn it off for itself, or on
    /// when it is only planned, with [`ShowSettingKey::NotifyNewEpisodes`]; with this off, no
    /// show is notified of.
    ///
    /// [`Command::CheckNewEpisodes`]: crate::command::Command::CheckNewEpisodes
    /// [`ShowSettingKey::NotifyNewEpisodes`]: crate::show::ShowSettingKey::NotifyNewEpisodes
    EpisodeNotifications,
    /// The viewer's own mpv options, `playback.mpv_args`, each as mpv's command line takes it
    /// (`--sub-font-size=40`): passed after Kaeri's own, so one here wins over its flags.
    /// The Mac's Advanced table.
    MpvOptions,
    /// Whether Search and the lists show titles AniList marks adult, with its adult genre and
    /// tags among the filters. Off by default.
    AdultTitles,
    /// Episode discussions: an episode's AniList forum thread, looked up once it counts as
    /// watched. On by default.
    EpisodeDiscussions,
    /// Whether images in discussion comments load inline rather than as a line that opens them
    /// in the browser. Off by default; shown while discussions are on.
    CommentImages,
    /// Drive the app with a connected game controller. On by default; the Linux app's, and any
    /// other that takes one.
    ControllerInput,
    /// Swap A and B, for a controller laid out as Nintendo's are. Off by default.
    SwapFaceButtons,
    /// Whether Calendar shows only the shows on the person's list rather than everything
    /// airing. Off by default, so Calendar opens on All, and remembered from the last pick.
    CalendarMine,
    /// Whether the TV picker lists TVs with Chromecast built in, and looks for them. On by
    /// default; under Play on TV.
    CastTvs,
    /// Whether a cast plays from torrents, which this device serves for the whole episode. Off
    /// on phones and tablets and on elsewhere, until chosen; under Sources, shown while
    /// torrents are on. Off, a torrent is passed over for a cast and the next source tried.
    TorrentsWhileCasting,
    /// Whether a cover the pointer rests on grows into a card with the title's details and its
    /// trailer, where trailers may play: the Mac's grids, and the desktop apps' later. On by
    /// default. With it off, a cover opens its title on a click and shows nothing on hover.
    HoverCards,
    /// How large subtitles are, percent of the player's size: 50 to 200 in steps of 10.
    SubtitleSize,
    SubtitleFont,
    SubtitleColour,
    SubtitleEdge,
    SubtitleThickness,
    SubtitleBackground,
    /// How opaque the plate behind the text is.
    SubtitleOpacity,
    /// How far subtitles are raised, percent of the picture.
    SubtitleRaise,
    /// What styled subtitles (ASS) keep of their authors' look: keep, restyle or strip.
    StyledSubtitles,
    /// Whether the system's caption style, where the platform has one, takes the place of the
    /// font, colour, edge and background chosen here.
    FollowSystemCaptions,
    /// Whether a crash is kept on this device, redacted, for a bug report. Off until turned on.
    SaveCrashReports,
    /// Whether a notice at the next launch after a crash offers a report. Shown while saving
    /// is on.
    AskAfterCrash,
}

impl SettingKey {
    /// The Settings screen's rows, in display order. Grouped by [`Self::category`] — the
    /// renderer draws a heading each time the category changes, so rows of one category must
    /// be contiguous here.
    pub const ROWS: [Self; 50] = [
        Self::Theme,
        Self::Motion,
        Self::Titles,
        Self::Trailers,
        Self::Translation,
        Self::Quality,
        Self::Subtitles,
        Self::CommitThreshold,
        Self::AutoNext,
        Self::SkipOpening,
        Self::SkipRecap,
        Self::AskWasOpening,
        Self::ShareSkipTimes,
        Self::SkipFiller,
        Self::MarkChapters,
        Self::SkipButton,
        Self::Fullscreen,
        Self::Upscaling,
        Self::PlayerUi,
        Self::NightMode,
        Self::SubtitleSize,
        Self::SubtitleFont,
        Self::SubtitleColour,
        Self::SubtitleEdge,
        Self::SubtitleThickness,
        Self::SubtitleBackground,
        Self::SubtitleOpacity,
        Self::SubtitleRaise,
        Self::StyledSubtitles,
        Self::FollowSystemCaptions,
        Self::DownloadDir,
        Self::MergeSubtitles,
        Self::KeepSeeding,
        Self::DownloadHook,
        Self::Torrents,
        Self::TorrentProxy,
        Self::SeedRatio,
        Self::SeedHours,
        Self::UploadLimit,
        Self::Presence,
        Self::PresenceTitle,
        Self::PresenceRich,
        Self::TokenStorage,
        Self::Syncplay,
        Self::SyncplayServer,
        Self::SyncplayRoom,
        Self::UpdateCheck,
        Self::UpdateBeta,
        Self::SaveCrashReports,
        Self::AskAfterCrash,
    ];

    /// The native apps' own rows, which the terminal's Settings screen does not show: a front
    /// end finds the rows both show among [`Self::ROWS`]. In no order of their own, since each
    /// app lays them out on its own panes; grouped by [`Self::category`] all the same.
    pub const NATIVE_ROWS: [Self; 31] = [
        Self::UpNextCountdown,
        Self::SpoilerProtection,
        Self::DownloadQuality,
        Self::KeepNextEpisodes,
        Self::DownloadsOnCellular,
        Self::TorrentsOnCellular,
        Self::EpisodeNotifications,
        Self::MpvOptions,
        Self::AdultTitles,
        Self::EpisodeDiscussions,
        Self::CommentImages,
        Self::ControllerInput,
        Self::SwapFaceButtons,
        Self::CalendarMine,
        Self::CastTvs,
        Self::TorrentsWhileCasting,
        Self::HoverCards,
        Self::FindSkipTimes,
        Self::ChapterDownloadsOnCellular,
        // Before its switch form, which shares its key: the key names this row.
        Self::NewChapters,
        Self::KeepChaptersReady,
        Self::DeleteAfterReading,
        Self::SeriesChoicesFollow,
        Self::ChapterNotifications,
        Self::DownloadsFinished,
        Self::OpenAt,
        Self::LookFollows,
        Self::WebtoonMode,
        Self::SendProgressAfter,
        Self::WhenTrackersDisagree,
        Self::CountsAsRead,
    ];

    /// Where the file keeps it: the containing table, as the dotted path
    /// `kaeri_core::settings::write_key` takes, and the key. `None` for the two shown and
    /// never written from here, the torrent proxy and where tokens are kept: set elsewhere.
    pub const fn path(self) -> Option<(&'static [&'static str], &'static str)> {
        Some(match self {
            Self::Theme => (&["theme"], "mode"),
            Self::Motion => (&["theme"], "motion"),
            Self::Titles => (&["meta"], "title_language"),
            Self::Trailers => (&["theme"], "trailers"),
            Self::AdultTitles => (&["meta"], "adult"),
            Self::EpisodeDiscussions => (&["meta"], "discussions"),
            Self::CommentImages => (&["meta"], "discussion_images"),
            Self::ControllerInput => (&["controller"], "enabled"),
            Self::SwapFaceButtons => (&["controller"], "swap_face"),
            Self::CalendarMine => (&["meta"], "calendar_mine"),
            Self::CastTvs => (&["cast"], "show_tvs"),
            Self::TorrentsWhileCasting => (&["providers", "torrent"], "while_casting"),
            Self::HoverCards => (&["theme"], "hover_cards"),
            Self::SubtitleSize => (&["subtitles"], "size"),
            Self::SubtitleFont => (&["subtitles"], "font"),
            Self::SubtitleColour => (&["subtitles"], "colour"),
            Self::SubtitleEdge => (&["subtitles"], "edge"),
            Self::SubtitleThickness => (&["subtitles"], "thickness"),
            Self::SubtitleBackground => (&["subtitles"], "background"),
            Self::SubtitleOpacity => (&["subtitles"], "opacity"),
            Self::SubtitleRaise => (&["subtitles"], "raise"),
            Self::StyledSubtitles => (&["subtitles"], "styled"),
            Self::FollowSystemCaptions => (&["subtitles"], "follow_system"),
            Self::SaveCrashReports => (&["crash_reports"], "save"),
            Self::AskAfterCrash => (&["crash_reports"], "ask"),
            Self::Translation => (&["playback"], "translation"),
            Self::Quality => (&["playback"], "quality"),
            Self::Subtitles => (&["playback"], "subtitle_language"),
            Self::CommitThreshold => (&["playback"], "commit_threshold"),
            Self::AutoNext => (&["playback"], "auto_next"),
            Self::SkipOpening => (&["playback"], "skip_opening"),
            Self::SkipRecap => (&["playback"], "skip_recap_auto"),
            Self::AskWasOpening => (&["playback"], "ask_was_opening"),
            Self::FindSkipTimes => (&["playback"], "find_skip_times"),
            Self::ShareSkipTimes => (&["playback"], "share_skip_times"),
            Self::SkipFiller => (&["playback"], "skip_filler"),
            Self::MarkChapters => (&["playback"], "mark_chapters"),
            Self::SkipButton => (&["playback"], "skip_button"),
            Self::Fullscreen => (&["playback"], "fullscreen"),
            Self::Upscaling => (&["playback"], "upscaling"),
            Self::PlayerUi => (&["playback"], "player_ui"),
            Self::NightMode => (&["playback"], "night_mode"),
            Self::DownloadDir => (&["downloads"], "directory"),
            Self::MergeSubtitles => (&["downloads"], "merge_subtitles"),
            Self::KeepSeeding => (&["downloads"], "keep_seeding"),
            Self::DownloadHook => (&["downloads"], "on_complete"),
            Self::Presence => (&["presence"], "enabled"),
            Self::PresenceTitle => (&["presence"], "show_title"),
            Self::PresenceRich => (&["presence"], "rich"),
            Self::Torrents => (&["providers", "torrent"], "enabled"),
            Self::SeedRatio => (&["providers", "torrent", "seeding"], "ratio"),
            Self::SeedHours => (&["providers", "torrent", "seeding"], "hours"),
            Self::UploadLimit => (&["providers", "torrent", "seeding"], "upload_limit_kib"),
            Self::Syncplay => (&["syncplay"], "enabled"),
            Self::SyncplayServer => (&["syncplay"], "server"),
            Self::SyncplayRoom => (&["syncplay"], "room"),
            Self::UpdateCheck => (&["updates"], "check"),
            Self::UpdateBeta => (&["updates"], "beta"),
            Self::SourceOrder => (&["providers"], "order"),
            Self::UpNextCountdown => (&["playback"], "up_next_countdown"),
            Self::SpoilerProtection => (&["playback"], "spoiler_protection"),
            Self::DownloadQuality => (&["downloads"], "quality"),
            Self::KeepNextEpisodes => (&["downloads"], "keep_next"),
            Self::DownloadsOnCellular => (&["downloads"], "on_cellular"),
            Self::ChapterDownloadsOnCellular => (&["reading"], "downloads_on_cellular"),
            Self::KeepChaptersReady | Self::NewChapters => (&["reading"], "new_chapters"),
            Self::DeleteAfterReading => (&["reading"], "delete_after_reading"),
            Self::OpdsFeed => (&["reading", "feed"], "on"),
            Self::OpdsPassword => (&["reading", "feed"], "password_set"),
            Self::OpdsPort => (&["reading", "feed"], "port"),
            Self::SeriesChoicesFollow => (&["reading"], "mode_follows"),
            Self::LookFollows => (&["reading"], "look_follows"),
            Self::WebtoonMode => (&["reading", "webtoon"], "mode"),
            Self::SendProgressAfter => (&["reading"], "send_progress_after"),
            Self::WhenTrackersDisagree => (&["reading"], "when_trackers_disagree"),
            Self::CountsAsRead => (&["reading"], "counts_as_read"),
            Self::ChapterNotifications => (&["notifications"], "chapters"),
            Self::DownloadsFinished => (&["notifications"], "downloads"),
            Self::OpenAt => (&["reading"], "open_at"),
            Self::TorrentsOnCellular => (&["providers", "torrent"], "on_cellular"),
            Self::EpisodeNotifications => (&["notifications"], "episodes"),
            Self::MpvOptions => (&["playback"], "mpv_args"),
            Self::TorrentProxy | Self::TokenStorage => return None,
        })
    }

    /// The setting the file keeps at `table`.`key`, when it is one a front end changes.
    pub fn at(table: &[&str], key: &str) -> Option<Self> {
        Self::ROWS
            .into_iter()
            .chain(Self::NATIVE_ROWS)
            .chain([Self::SourceOrder])
            .find(|setting| setting.path().is_some_and(|(t, k)| t == table && k == key))
    }

    /// Whether its value is typed rather than cycled. Everything else either has a closed set
    /// of values or is read-only on purpose.
    pub const fn typed(self) -> bool {
        matches!(
            self,
            Self::Subtitles
                | Self::DownloadDir
                | Self::DownloadHook
                | Self::SyncplayServer
                | Self::SyncplayRoom
                | Self::MpvOptions
                | Self::OpdsPassword
                | Self::OpdsPort
        )
    }

    /// Copy this setting's value from one configuration to another.
    ///
    /// What lets one setting be put back to what the file holds after its write failed,
    /// without disturbing any other — including one changed a moment later whose own write
    /// worked.
    pub fn copy(self, from: &Config, to: &mut Config) {
        let (src, dst) = (&from.playback, &mut to.playback);
        match self {
            Self::Theme => to.theme.mode = from.theme.mode,
            Self::Motion => to.theme.motion = from.theme.motion,
            Self::Titles => to.meta.title_language = from.meta.title_language,
            Self::Trailers => to.theme.trailers = from.theme.trailers,
            Self::Translation => dst.translation = src.translation,
            Self::Quality => dst.quality = src.quality,
            Self::Subtitles => dst.subtitle_language.clone_from(&src.subtitle_language),
            Self::CommitThreshold => dst.commit_threshold = src.commit_threshold,
            Self::AutoNext => dst.auto_next = src.auto_next,
            Self::SkipOpening => dst.skip_opening = src.skip_opening,
            Self::SkipRecap => dst.skip_recap_auto = src.skip_recap_auto,
            Self::AskWasOpening => dst.ask_was_opening = src.ask_was_opening,
            Self::FindSkipTimes => dst.find_skip_times = src.find_skip_times,
            Self::ShareSkipTimes => dst.share_skip_times = src.share_skip_times,
            Self::SkipFiller => dst.skip_filler = src.skip_filler,
            Self::MarkChapters => dst.mark_chapters = src.mark_chapters,
            Self::SkipButton => dst.skip_button = src.skip_button,
            Self::Fullscreen => dst.fullscreen = src.fullscreen,
            Self::Upscaling => dst.upscaling = src.upscaling,
            Self::PlayerUi => dst.player_ui = src.player_ui,
            Self::NightMode => dst.night_mode = src.night_mode,
            Self::DownloadDir => to.downloads.directory.clone_from(&from.downloads.directory),
            Self::MergeSubtitles => {
                to.downloads.merge_subtitles = from.downloads.merge_subtitles
            }
            Self::KeepSeeding => to.downloads.keep_seeding = from.downloads.keep_seeding,
            Self::DownloadHook => {
                to.downloads.on_complete.clone_from(&from.downloads.on_complete)
            }
            Self::Presence => to.presence.enabled = from.presence.enabled,
            Self::PresenceTitle => to.presence.show_title = from.presence.show_title,
            Self::PresenceRich => to.presence.rich = from.presence.rich,
            Self::Torrents => to.providers.torrent.enabled = from.providers.torrent.enabled,
            Self::SeedRatio => {
                to.providers.torrent.seeding.ratio = from.providers.torrent.seeding.ratio;
            }
            Self::SeedHours => {
                to.providers.torrent.seeding.hours = from.providers.torrent.seeding.hours;
            }
            Self::UploadLimit => {
                to.providers.torrent.seeding.upload_limit_kib =
                    from.providers.torrent.seeding.upload_limit_kib;
            }
            Self::Syncplay => to.syncplay.enabled = from.syncplay.enabled,
            Self::SyncplayServer => to.syncplay.server.clone_from(&from.syncplay.server),
            Self::SyncplayRoom => to.syncplay.room.clone_from(&from.syncplay.room),
            Self::UpdateCheck => to.updates.check = from.updates.check,
            Self::UpdateBeta => to.updates.beta = from.updates.beta,
            Self::AdultTitles => to.meta.adult = from.meta.adult,
            Self::EpisodeDiscussions => to.meta.discussions = from.meta.discussions,
            Self::CommentImages => to.meta.discussion_images = from.meta.discussion_images,
            Self::ControllerInput => to.controller.enabled = from.controller.enabled,
            Self::SwapFaceButtons => to.controller.swap_face = from.controller.swap_face,
            Self::CalendarMine => to.meta.calendar_mine = from.meta.calendar_mine,
            Self::CastTvs => to.cast.show_tvs = from.cast.show_tvs,
            Self::TorrentsWhileCasting => {
                to.providers.torrent.while_casting = from.providers.torrent.while_casting;
            }
            Self::HoverCards => to.theme.hover_cards = from.theme.hover_cards,
            Self::SubtitleSize => to.subtitles.size = from.subtitles.size,
            Self::SubtitleFont => to.subtitles.font = from.subtitles.font,
            Self::SubtitleColour => to.subtitles.colour = from.subtitles.colour,
            Self::SubtitleEdge => to.subtitles.edge = from.subtitles.edge,
            Self::SubtitleThickness => to.subtitles.thickness = from.subtitles.thickness,
            Self::SubtitleBackground => to.subtitles.background = from.subtitles.background,
            Self::SubtitleOpacity => to.subtitles.opacity = from.subtitles.opacity,
            Self::SubtitleRaise => to.subtitles.raise = from.subtitles.raise,
            Self::StyledSubtitles => to.subtitles.styled = from.subtitles.styled,
            Self::FollowSystemCaptions => {
                to.subtitles.follow_system = from.subtitles.follow_system;
            }
            Self::SaveCrashReports => {
                to.crash_reports.save = from.crash_reports.save;
                to.crash_reports.enabled_at = from.crash_reports.enabled_at;
            }
            Self::AskAfterCrash => to.crash_reports.ask = from.crash_reports.ask,
            Self::MpvOptions => dst.mpv_args.clone_from(&src.mpv_args),
            Self::SourceOrder => to.providers.order.clone_from(&from.providers.order),
            Self::UpNextCountdown => dst.up_next_countdown = src.up_next_countdown,
            Self::SpoilerProtection => dst.spoiler_protection = src.spoiler_protection,
            Self::DownloadQuality => to.downloads.quality = from.downloads.quality,
            Self::KeepNextEpisodes => to.downloads.keep_next = from.downloads.keep_next,
            Self::DownloadsOnCellular => to.downloads.on_cellular = from.downloads.on_cellular,
            Self::ChapterDownloadsOnCellular => {
                to.reading.downloads_on_cellular = from.reading.downloads_on_cellular;
            }
            Self::KeepChaptersReady | Self::NewChapters => {
                to.reading.new_chapters_choice = from.reading.new_chapters_choice;
                to.reading.keep_chapters_ready = from.reading.keep_chapters_ready;
            }
            Self::DeleteAfterReading => {
                to.reading.delete_after_reading = from.reading.delete_after_reading;
            }
            Self::OpdsFeed => to.reading.feed.on = from.reading.feed.on,
            Self::OpdsPassword => {
                to.reading.feed.password_set = from.reading.feed.password_set;
            }
            Self::OpdsPort => to.reading.feed.port = from.reading.feed.port,
            Self::SeriesChoicesFollow => {
                to.reading.mode_follows = from.reading.mode_follows;
                to.reading.series_choices_follow = from.reading.series_choices_follow;
            }
            Self::LookFollows => {
                to.reading.look_follows = from.reading.look_follows;
                to.reading.series_choices_follow = from.reading.series_choices_follow;
            }
            Self::WebtoonMode => to.reading.webtoon = from.reading.webtoon,
            Self::SendProgressAfter => {
                to.reading.send_progress_after = from.reading.send_progress_after;
            }
            Self::WhenTrackersDisagree => {
                to.reading.when_trackers_disagree = from.reading.when_trackers_disagree;
            }
            Self::CountsAsRead => to.reading.counts_as_read = from.reading.counts_as_read,
            Self::ChapterNotifications => {
                to.notifications.chapters = from.notifications.chapters;
            }
            Self::DownloadsFinished => {
                to.notifications.downloads = from.notifications.downloads
            }
            Self::OpenAt => to.reading.open_at.clone_from(&from.reading.open_at),
            Self::TorrentsOnCellular => {
                to.providers.torrent.on_cellular = from.providers.torrent.on_cellular;
            }
            Self::EpisodeNotifications => {
                to.notifications.episodes = from.notifications.episodes;
            }
            // Not editable here, so never written and never rolled back.
            Self::TorrentProxy | Self::TokenStorage => {}
        }
    }

    /// Which heading a row sits under. Purely visual grouping — navigation walks the
    /// whole list, so nothing is ever hidden behind a tab.
    pub const fn category(self) -> &'static str {
        match self {
            Self::Theme
            | Self::Motion
            | Self::Titles
            | Self::Trailers
            | Self::AdultTitles
            | Self::ControllerInput
            | Self::SwapFaceButtons
            | Self::CalendarMine
            | Self::HoverCards
            | Self::OpenAt => "appearance",
            Self::Translation
            | Self::Quality
            | Self::Subtitles
            | Self::CommitThreshold
            | Self::AutoNext
            | Self::SkipOpening
            | Self::SkipRecap
            | Self::AskWasOpening
            | Self::FindSkipTimes
            | Self::ShareSkipTimes
            | Self::SkipFiller
            | Self::MarkChapters
            | Self::SkipButton
            | Self::Fullscreen
            | Self::Upscaling
            | Self::PlayerUi
            | Self::NightMode
            | Self::SubtitleSize
            | Self::SubtitleFont
            | Self::SubtitleColour
            | Self::SubtitleEdge
            | Self::SubtitleThickness
            | Self::SubtitleBackground
            | Self::SubtitleOpacity
            | Self::SubtitleRaise
            | Self::StyledSubtitles
            | Self::FollowSystemCaptions
            | Self::UpNextCountdown
            | Self::SpoilerProtection => "playback",
            Self::DownloadDir
            | Self::MergeSubtitles
            | Self::KeepSeeding
            | Self::DownloadHook
            | Self::DownloadQuality
            | Self::KeepNextEpisodes
            | Self::DownloadsOnCellular
            | Self::ChapterDownloadsOnCellular
            | Self::KeepChaptersReady
            | Self::NewChapters
            | Self::DeleteAfterReading => "downloads",
            Self::OpdsFeed | Self::OpdsPassword | Self::OpdsPort => "downloads",
            Self::SeriesChoicesFollow
            | Self::LookFollows
            | Self::WebtoonMode
            | Self::CountsAsRead => "reading",
            Self::SendProgressAfter | Self::WhenTrackersDisagree => "accounts",
            Self::Torrents
            | Self::TorrentProxy
            | Self::SeedRatio
            | Self::SeedHours
            | Self::UploadLimit
            | Self::SourceOrder
            | Self::TorrentsOnCellular
            | Self::TorrentsWhileCasting => "sources",
            Self::Presence
            | Self::PresenceTitle
            | Self::PresenceRich
            | Self::TokenStorage
            | Self::Syncplay
            | Self::SyncplayServer
            | Self::SyncplayRoom
            | Self::UpdateCheck
            | Self::UpdateBeta
            | Self::EpisodeDiscussions
            | Self::CommentImages
            | Self::CastTvs => "integrations",
            Self::EpisodeNotifications
            | Self::ChapterNotifications
            | Self::DownloadsFinished => "notifications",
            Self::SaveCrashReports | Self::AskAfterCrash => "about",
            Self::MpvOptions => "advanced",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Theme => "theme",
            Self::Motion => "motion",
            Self::Titles => "titles",
            Self::Trailers => "play trailers",
            Self::Translation => "translation",
            Self::Quality => "quality",
            Self::Subtitles => "subtitles",
            Self::CommitThreshold => "counts as watched at",
            Self::AutoNext => "auto-play next",
            // With this on, the opening is skipped, not proposed. mpv shows
            // a note on its own OSD as it happens, which is a courtesy, not a prompt.
            Self::SkipOpening => "skip opening automatically",
            Self::SkipRecap => "skip recaps automatically",
            Self::AskWasOpening => "ask about skip times",
            Self::FindSkipTimes => "find skip times from the audio",
            Self::ShareSkipTimes => "share skip times with AniSkip",
            Self::SkipFiller => "skip filler automatically",
            Self::MarkChapters => "mark opening and ending",
            Self::SkipButton => "show skip button",
            Self::Fullscreen => "start fullscreen",
            Self::Upscaling => "upscaling",
            Self::PlayerUi => "player interface",
            Self::NightMode => "night mode",
            Self::DownloadDir => "download folder",
            Self::MergeSubtitles => "merge subtitles in",
            Self::KeepSeeding => "keep seeding",
            Self::DownloadHook => "after each download",
            Self::Syncplay => "syncplay parties",
            Self::SyncplayServer => "syncplay server",
            Self::SyncplayRoom => "syncplay room",
            Self::UpdateCheck => "check for updates",
            Self::UpdateBeta => "beta updates",
            Self::Presence => "discord presence",
            Self::PresenceTitle => "presence shows the title",
            Self::PresenceRich => "rich presence",
            Self::Torrents => "torrent playback",
            Self::SeedRatio => "seed after watching",
            Self::SeedHours => "seed for at most",
            Self::UploadLimit => "upload limit",
            Self::TorrentProxy => "torrent proxy",
            Self::TokenStorage => "token storage",
            Self::SourceOrder => "source order",
            Self::UpNextCountdown => "up next countdown",
            Self::SpoilerProtection => "spoiler protection",
            // Not "quality", which is playback's: the toast that says it was saved would not
            // say which.
            Self::DownloadQuality => "download quality",
            Self::KeepNextEpisodes => "keep next episodes",
            Self::DownloadsOnCellular => "downloads on cellular",
            Self::ChapterDownloadsOnCellular => "chapter downloads on cellular",
            Self::KeepChaptersReady => "keep chapters ready",
            Self::NewChapters => "new chapters",
            Self::DeleteAfterReading => "delete after reading",
            Self::OpdsFeed => "share on this network",
            Self::OpdsPassword => "feed password",
            Self::OpdsPort => "feed port",
            Self::SeriesChoicesFollow => "mode and direction follow you",
            Self::LookFollows => "look follows you",
            Self::WebtoonMode => "webtoons",
            Self::SendProgressAfter => "send progress after",
            Self::WhenTrackersDisagree => "when trackers disagree",
            Self::CountsAsRead => "counts as read",
            Self::ChapterNotifications => "new chapter notifications",
            Self::DownloadsFinished => "downloads finished",
            Self::OpenAt => "open at",
            Self::TorrentsOnCellular => "torrents on cellular",
            Self::AdultTitles => "adult titles",
            Self::EpisodeDiscussions => "episode discussions",
            Self::CommentImages => "load images in comments",
            Self::ControllerInput => "use a controller",
            Self::SwapFaceButtons => "swap a and b",
            Self::CalendarMine => "calendar shows only your list",
            Self::CastTvs => "show tvs with chromecast built in",
            Self::TorrentsWhileCasting => "torrents while casting",
            Self::HoverCards => "expand covers on hover",
            Self::EpisodeNotifications => "new episode notifications",
            Self::SubtitleSize => "subtitle size",
            Self::SubtitleFont => "subtitle font",
            Self::SubtitleColour => "subtitle colour",
            Self::SubtitleEdge => "subtitle edge",
            Self::SubtitleThickness => "edge thickness",
            Self::SubtitleBackground => "subtitle background",
            Self::SubtitleOpacity => "background opacity",
            Self::SubtitleRaise => "raise subtitles",
            Self::StyledSubtitles => "styled subtitles",
            Self::FollowSystemCaptions => "follow system captions",
            Self::SaveCrashReports => "save crash reports",
            Self::AskAfterCrash => "ask after a crash",
            Self::MpvOptions => "mpv options",
        }
    }
}

/// Whether a cast plays from torrents. The engine fills the setting in at launch from the
/// device it runs on; unset here is a computer's default, on.
pub fn while_casting(config: &Config) -> bool {
    config.providers.torrent.while_casting.unwrap_or(true)
}

/// Whether writing this setting changes which sources exist, and so needs the chain rebuilt.
///
/// Listed rather than inferred: a rebuild tears down a live torrent session, which is far
/// too blunt to trigger on a guess about a setting. The torrent proxy is listed though nothing
/// here writes it, since the torrent session is built on it; the sources switched off by name
/// and the remote's address, which the file alone sets, have no key to list.
pub fn redefines_sources(setting: SettingKey) -> bool {
    matches!(setting, SettingKey::Torrents | SettingKey::TorrentProxy | SettingKey::SourceOrder)
}

// The Settings screen's rules, over the config in effect and the file's: what each row shows and
// how it changes, and what a step or a typed value makes of a setting.
//
// Pure, so they can run wherever the config is owned, and a front end shows what they say: the
// ladders, the two-state values and their ends, and the wording of each value.

/// What a front end is told of the config: the Settings screen's rows, and everything else it
/// shows or acts on that the config decides.
///
/// Sent whole whenever any of it changes, so a front end never holds a config of its own: what
/// it shows is what the engine has in effect.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SettingsView {
    /// The Settings screen's rows, in display order.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub rows: Vec<SettingRow>,
    /// The setting whose write was just refused and put back to what the file holds: on the
    /// view that answers the refusal, and on no other. The notice saying why comes after it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub refused: Option<SettingKey>,
    /// Sub or dub, in effect: what the status line says, and what `t` turns over.
    pub translation: Translation,
    /// The quality asked of the sources, in effect, for the status line.
    pub quality: u32,
    /// The theme in effect, which the terminal paints with.
    pub theme: ThemeMode,
    /// Whether the front end may animate: off, the terminal's wipe cuts straight to its card and
    /// its pulses hold still.
    #[cfg_attr(feature = "ffi", uniffi(default = true))]
    pub motion: bool,
    /// The sources the config sets up: the header's chips, the source the status line names,
    /// and the order Providers moves a source up and down.
    pub sources: ConfiguredSources,
    /// The speed a playback starts at, when one is remembered: what Now Playing shows of a
    /// playback the front end stages as it asks for it, until the player says. One the engine
    /// starts on its own comes staged with it.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub speed: Option<f64>,
    /// Whether Discord presence has a client to publish under, which its row's note depends on.
    pub presence_client: bool,
    /// Whether this engine can publish a presence at all: it is a desktop's, not a phone's. A
    /// front end shows no Discord setting or preview where it is not. The terminal's own is
    /// always so; the engine says what it was built with.
    #[cfg_attr(feature = "ffi", uniffi(default = true))]
    pub presence_available: bool,
    /// The native apps' own rows, in [`SettingKey::NATIVE_ROWS`] order. The terminal draws
    /// none of them.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub native_rows: Vec<SettingRow>,
    /// The settings some shows set for themselves, and for how many: what "Overridden for 3
    /// shows" says under a row. Only those at least one show sets, each once.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub overridden: Vec<SettingOverride>,
    /// How many shows switch new-episode notifications on for themselves, which notify whether
    /// [`SettingKey::EpisodeNotifications`] is on or not. A native app keeps its background
    /// check scheduled while either is, and cancels it only when neither is.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub notify_switched_on: u32,
    /// The subtitle style in effect: what the Subtitles group shows and the player draws with.
    /// Always set by the engine. The apps send it, and the system's caption style, to
    /// `mpv_subtitle_options`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub subtitle_style: Option<crate::subtitle_style::SubtitleStyle>,
    /// Where torrent traffic goes, directly or through a proxy, as the torrent sheet and
    /// Sources show it. Boxed, as the bindings see the view itself.
    pub torrent: Box<crate::proxy::TorrentView>,
    /// Why finding skip times from the audio is paused, while
    /// [`SettingKey::FindSkipTimes`] is on: the line under its row. `None` while it runs, or
    /// while it is off.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub skip_find_paused: Option<crate::playback::SkipFindPause>,
}

/// How many shows set a setting for themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SettingOverride {
    /// The setting: the Settings row the count goes under.
    pub key: SettingKey,
    /// How many shows set it, one or more.
    pub shows: u32,
}

impl Default for SettingsView {
    /// The view of the defaults, in effect and in the file alike.
    fn default() -> Self {
        let config = Config::default();
        view(&config, &config)
    }
}

/// What a front end is told of the config, from the config in effect and the file's.
pub fn view(effective: &Config, saved: &Config) -> SettingsView {
    SettingsView {
        rows: rows(effective, saved),
        refused: None,
        translation: effective.playback.translation,
        quality: effective.playback.quality,
        theme: effective.theme.mode,
        motion: effective.theme.motion,
        sources: ConfiguredSources::of(&effective.providers).with_demo(&effective.sources),
        speed: effective.playback.persisted_speed,
        presence_client: effective.presence.resolved_client_id().is_some(),
        presence_available: true,
        native_rows: native_rows(effective, saved),
        // The shows' own are the database's, which the engine counts and puts in.
        overridden: Vec::new(),
        notify_switched_on: 0,
        subtitle_style: Some(effective.subtitles.clone().into()),
        torrent: Box::new(crate::proxy::view(&effective.providers.torrent)),
        // The device's state is the engine's, which puts it in.
        skip_find_paused: None,
    }
}

/// Every subtitle setting at its default, for Reset subtitle style.
pub fn subtitle_defaults() -> Vec<(SettingKey, SettingValue)> {
    use SettingValue as V;
    let d = SubtitlesConfig::default();
    vec![
        (SettingKey::SubtitleSize, V::Int(i64::from(d.size))),
        (SettingKey::SubtitleFont, V::Str(d.font.name().into())),
        (SettingKey::SubtitleColour, V::Str(d.colour.name().into())),
        (SettingKey::SubtitleEdge, V::Str(d.edge.name().into())),
        (SettingKey::SubtitleThickness, V::Str(d.thickness.name().into())),
        (SettingKey::SubtitleBackground, V::Str(d.background.name().into())),
        (SettingKey::SubtitleOpacity, V::Int(i64::from(d.opacity))),
        (SettingKey::SubtitleRaise, V::Int(i64::from(d.raise))),
        (SettingKey::StyledSubtitles, V::Str(d.styled.name().into())),
        (SettingKey::FollowSystemCaptions, V::Bool(d.follow_system)),
    ]
}

/// One row of the Settings screen, as the config makes it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SettingRow {
    /// The setting it shows.
    pub key: SettingKey,
    /// Its value as the row shows it: `on`, `1080p`, `adaptive · immersive this session`.
    pub value: String,
    /// How it is changed from here.
    pub change: SettingChange,
}

/// How a row's value is changed.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SettingChange {
    /// Stepped through a closed set of values: a ladder, or two states. `back` and `forward`
    /// say whether a step that way would change it — not at a ladder's end, and not toward
    /// the side a two-state value already stands on.
    Steps { back: bool, forward: bool },
    /// Typed, starting from `text`: too many values to step through.
    Typed { text: String },
    /// Not changed from here: the file is where, on purpose.
    Fixed,
}

impl SettingChange {
    /// Whether the row can be changed from here at all.
    pub fn editable(&self) -> bool {
        !matches!(self, Self::Fixed)
    }
}

/// The Settings screen's rows, in display order, from the config in effect and the file's.
///
/// Each shows the value in effect, so a row says what is actually in force the instant after
/// it is changed. The two that can also be changed for a session only, theme and sub/dub,
/// show what is saved and mark the session's value beside it: the file and the next launch
/// go by the saved one.
pub fn rows(effective: &Config, saved: &Config) -> Vec<SettingRow> {
    SettingKey::ROWS.into_iter().map(|key| row(effective, saved, key)).collect()
}

/// The native apps' own rows, in [`SettingKey::NATIVE_ROWS`] order, as [`rows`] makes the
/// Settings screen's.
pub fn native_rows(effective: &Config, saved: &Config) -> Vec<SettingRow> {
    SettingKey::NATIVE_ROWS.into_iter().map(|key| row(effective, saved, key)).collect()
}

fn row(effective: &Config, saved: &Config, key: SettingKey) -> SettingRow {
    SettingRow {
        key,
        value: value(effective, saved, key),
        change: change(effective, saved, key),
    }
}

/// The value a step of `delta` makes of `key`, as the file would be given it, or `None` for
/// a setting no step changes: typed, read-only, or the order.
///
/// A ladder steps one rung toward `delta`'s sign and stops at its ends, where the value comes
/// back as it is; a two-state value flips whichever way. Theme and sub/dub step from the
/// saved value, the one their row leads with: the arrows change what is saved, and a session
/// override simply gives way to it.
pub fn step(
    effective: &Config,
    saved: &Config,
    key: SettingKey,
    delta: i64,
) -> Option<SettingValue> {
    use SettingValue as V;
    use kaeri_core::config::{DownloadQuality, PlayerUi, ThemeMode, Upscaling};
    use kaeri_core::media::Translation;

    let playback = &effective.playback;
    let seeding = &effective.providers.torrent.seeding;
    Some(match key {
        // Round the four, either way: adaptive first, as it is the one most want.
        SettingKey::Theme => {
            const RING: [ThemeMode; 4] =
                [ThemeMode::Adaptive, ThemeMode::Immersive, ThemeMode::Light, ThemeMode::Dark];
            let at = RING.iter().position(|m| *m == saved.theme.mode).unwrap_or(0) as i64;
            let next = RING[(at + delta.signum()).rem_euclid(RING.len() as i64) as usize];
            V::Str(format!("{next:?}").to_lowercase())
        }
        SettingKey::Motion => V::Bool(!effective.theme.motion),
        SettingKey::Titles => {
            const LADDER: [TitleChoice; 3] =
                [TitleChoice::Anilist, TitleChoice::English, TitleChoice::Romaji];
            let current = effective.meta.title_language;
            let at = LADDER.iter().position(|c| *c == current).unwrap_or(0) as i64;
            V::Str(LADDER[(at + delta).clamp(0, 2) as usize].name().into())
        }
        SettingKey::Trailers => {
            const LADDER: [TrailerAutoplay; 3] = TrailerAutoplay::ALL;
            let at = LADDER.iter().position(|c| *c == effective.theme.trailers).unwrap_or(0);
            V::Str(LADDER[(at as i64 + delta).clamp(0, 2) as usize].name().into())
        }
        SettingKey::Translation => V::Str(
            match saved.playback.translation {
                Translation::Sub => Translation::Dub,
                Translation::Dub => Translation::Sub,
            }
            .to_string(),
        ),
        SettingKey::Quality => {
            const LADDER: [u32; 4] = [480, 720, 1080, 2160];
            V::Int(i64::from(step_through(&LADDER, playback.quality, delta)))
        }
        SettingKey::CommitThreshold => {
            const LADDER: [u32; 5] = [70, 80, 85, 90, 95];
            let current = (playback.commit_threshold * 100.0).round() as u32;
            V::Float(f64::from(step_through(&LADDER, current, delta)) / 100.0)
        }
        SettingKey::AutoNext => V::Bool(!playback.auto_next),
        SettingKey::Upscaling => {
            const LADDER: [Upscaling; 3] =
                [Upscaling::Off, Upscaling::Natural, Upscaling::Sharp];
            let at = LADDER.iter().position(|u| *u == playback.upscaling).unwrap_or(0) as i64;
            V::Str(LADDER[(at + delta).clamp(0, 2) as usize].name().into())
        }
        SettingKey::PlayerUi => V::Str(
            match playback.player_ui {
                PlayerUi::Kaeri => "mpv",
                PlayerUi::Mpv => "kaeri",
            }
            .into(),
        ),
        SettingKey::NightMode => V::Bool(!playback.night_mode),
        SettingKey::MergeSubtitles => V::Bool(!effective.downloads.merge_subtitles),
        SettingKey::KeepSeeding => V::Bool(!effective.downloads.keep_seeding),
        SettingKey::Syncplay => V::Bool(!effective.syncplay.enabled),
        SettingKey::UpdateCheck => V::Bool(!effective.updates.check),
        SettingKey::UpdateBeta => V::Bool(!effective.updates.beta),
        SettingKey::SkipOpening => V::Bool(!playback.skip_opening),
        SettingKey::SkipRecap => V::Bool(!playback.skip_recap_auto),
        SettingKey::AskWasOpening => V::Bool(!playback.ask_was_opening),
        SettingKey::FindSkipTimes => V::Bool(!playback.find_skip_times),
        SettingKey::ShareSkipTimes => V::Bool(playback.share_skip_times != Some(true)),
        SettingKey::SkipFiller => V::Bool(!playback.skip_filler),
        SettingKey::MarkChapters => V::Bool(!playback.mark_chapters),
        SettingKey::SkipButton => V::Bool(!playback.skip_button),
        SettingKey::Fullscreen => V::Bool(!playback.fullscreen),
        SettingKey::Presence => V::Bool(!effective.presence.enabled),
        SettingKey::PresenceTitle => V::Bool(!effective.presence.show_title),
        SettingKey::PresenceRich => V::Bool(!effective.presence.rich),
        SettingKey::Torrents => V::Bool(!effective.providers.torrent.enabled),
        SettingKey::SeedRatio => {
            // Tenths, so the ladder can be matched exactly rather than as floats.
            const LADDER: [u32; 5] = [0, 5, 10, 15, 20];
            let current = (seeding.ratio * 10.0).round() as u32;
            V::Float(f64::from(step_through(&LADDER, current, delta)) / 10.0)
        }
        SettingKey::SeedHours => {
            const LADDER: [u32; 5] = [1, 6, 12, 24, 72];
            V::Int(i64::from(step_through(&LADDER, seeding.hours, delta)))
        }
        SettingKey::UploadLimit => {
            const LADDER: [u32; 6] = [0, 256, 512, 1024, 2048, 5120];
            V::Int(i64::from(step_through(&LADDER, seeding.upload_limit_kib, delta)))
        }
        SettingKey::UpNextCountdown => {
            const LADDER: [u32; 6] = [0, 5, 10, 15, 20, 30];
            V::Int(i64::from(step_through(&LADDER, playback.up_next_countdown, delta)))
        }
        SettingKey::SpoilerProtection => V::Bool(!playback.spoiler_protection),
        SettingKey::DownloadQuality => V::Str(
            match effective.downloads.quality {
                DownloadQuality::Standard => "higher",
                DownloadQuality::Higher => "standard",
            }
            .into(),
        ),
        SettingKey::KeepNextEpisodes => {
            const LADDER: [u32; 6] = [0, 1, 2, 3, 5, 10];
            V::Int(i64::from(step_through(&LADDER, effective.downloads.keep_next, delta)))
        }
        SettingKey::DownloadsOnCellular => V::Bool(!effective.downloads.on_cellular),
        SettingKey::ChapterDownloadsOnCellular => {
            V::Bool(!effective.reading.downloads_on_cellular)
        }
        SettingKey::KeepChaptersReady => V::Bool(!keeps_ready(effective)),
        SettingKey::NewChapters => V::Str(stepped(
            NewChapters::ALL,
            effective.reading.new_chapters(NewChapters::Off),
            delta,
        )),
        SettingKey::DeleteAfterReading => V::Bool(!effective.reading.delete_after_reading),
        SettingKey::OpdsFeed => V::Bool(!effective.reading.feed.on),
        SettingKey::SeriesChoicesFollow => V::Bool(!effective.reading.mode_follows()),
        SettingKey::LookFollows => V::Bool(!effective.reading.look_follows()),
        SettingKey::WebtoonMode => {
            V::Str(stepped(WebtoonMode::ALL, effective.reading.webtoon.mode, delta))
        }
        SettingKey::SendProgressAfter => V::Str(stepped(
            SendProgressAfter::ALL,
            effective.reading.send_progress_after,
            delta,
        )),
        SettingKey::WhenTrackersDisagree => V::Str(stepped(
            WhenTrackersDisagree::ALL,
            effective.reading.when_trackers_disagree,
            delta,
        )),
        SettingKey::CountsAsRead => {
            V::Str(stepped(CountsAsRead::ALL, effective.reading.counts_as_read, delta))
        }
        SettingKey::ChapterNotifications => V::Bool(!effective.notifications.chapters),
        SettingKey::DownloadsFinished => V::Bool(!effective.notifications.downloads),
        SettingKey::OpenAt => {
            const LADDER: [&str; 2] = ["watch", "read"];
            let at = LADDER.iter().position(|c| *c == effective.reading.open_at).unwrap_or(0);
            V::Str(LADDER[(at as i64 + delta).clamp(0, 1) as usize].into())
        }
        SettingKey::TorrentsOnCellular => V::Bool(!effective.providers.torrent.on_cellular),
        SettingKey::EpisodeNotifications => V::Bool(!effective.notifications.episodes),
        SettingKey::AdultTitles => V::Bool(!effective.meta.adult),
        SettingKey::EpisodeDiscussions => V::Bool(!effective.meta.discussions),
        SettingKey::CommentImages => V::Bool(!effective.meta.discussion_images),
        SettingKey::ControllerInput => V::Bool(!effective.controller.enabled),
        SettingKey::SwapFaceButtons => V::Bool(!effective.controller.swap_face),
        SettingKey::CalendarMine => V::Bool(!effective.meta.calendar_mine),
        SettingKey::CastTvs => V::Bool(!effective.cast.show_tvs),
        SettingKey::TorrentsWhileCasting => V::Bool(!while_casting(effective)),
        SettingKey::HoverCards => V::Bool(!effective.theme.hover_cards),
        SettingKey::SubtitleSize => {
            let (low, high, by) = SubtitlesConfig::SIZES;
            let ladder: Vec<u32> = (low..=high).step_by(by as usize).collect();
            V::Int(i64::from(step_through(&ladder, effective.subtitles.size, delta)))
        }
        SettingKey::SubtitleOpacity => V::Int(i64::from(step_through(
            &SubtitlesConfig::OPACITIES,
            effective.subtitles.opacity,
            delta,
        ))),
        SettingKey::SubtitleRaise => {
            let ladder: Vec<u32> = (0..=SubtitlesConfig::MAX_RAISE).step_by(2).collect();
            V::Int(i64::from(step_through(&ladder, effective.subtitles.raise, delta)))
        }
        SettingKey::SubtitleFont => {
            V::Str(stepped(SubtitleFont::ALL, effective.subtitles.font, delta))
        }
        SettingKey::SubtitleColour => {
            V::Str(stepped(SubtitleColour::ALL, effective.subtitles.colour, delta))
        }
        SettingKey::SubtitleEdge => {
            V::Str(stepped(SubtitleEdge::ALL, effective.subtitles.edge, delta))
        }
        SettingKey::SubtitleThickness => {
            V::Str(stepped(SubtitleThickness::ALL, effective.subtitles.thickness, delta))
        }
        SettingKey::SubtitleBackground => {
            V::Str(stepped(SubtitleBackground::ALL, effective.subtitles.background, delta))
        }
        SettingKey::StyledSubtitles => {
            V::Str(stepped(StyledSubtitles::ALL, effective.subtitles.styled, delta))
        }
        SettingKey::FollowSystemCaptions => V::Bool(!effective.subtitles.follow_system),
        SettingKey::SaveCrashReports => V::Bool(!effective.crash_reports.save),
        SettingKey::AskAfterCrash => V::Bool(!effective.crash_reports.ask),
        // Typed, read-only, or moved on Providers: nothing to step through.
        SettingKey::Subtitles
        | SettingKey::DownloadDir
        | SettingKey::DownloadHook
        | SettingKey::SyncplayServer
        | SettingKey::SyncplayRoom
        | SettingKey::OpdsPassword
        | SettingKey::OpdsPort
        | SettingKey::TorrentProxy
        | SettingKey::TokenStorage
        | SettingKey::SourceOrder
        | SettingKey::MpvOptions => return None,
    })
}

/// Put `value`, as written for `key`, into `config`: what a write puts in effect.
///
/// `false`, with nothing changed, for a value the setting does not take: the wrong kind, a
/// name it does not know, a setting that is not written from here, or no mpv options over
/// none.
pub fn put(config: &mut Config, key: SettingKey, value: &SettingValue) -> bool {
    use SettingValue as V;
    use kaeri_core::config::{DownloadQuality, PlayerUi, ThemeMode, Upscaling};
    use kaeri_core::media::Translation;

    let playback = &mut config.playback;
    let seeding = &mut config.providers.torrent.seeding;
    match (key, value) {
        (SettingKey::Theme, V::Str(mode)) => {
            config.theme.mode = match mode.as_str() {
                "adaptive" => ThemeMode::Adaptive,
                "immersive" => ThemeMode::Immersive,
                "light" => ThemeMode::Light,
                "dark" => ThemeMode::Dark,
                _ => return false,
            };
        }
        (SettingKey::Motion, V::Bool(on)) => config.theme.motion = *on,
        (SettingKey::Titles, V::Str(name)) => {
            let Some(choice) = TitleChoice::from_name(name) else { return false };
            config.meta.title_language = choice;
        }
        (SettingKey::Trailers, V::Str(name)) => {
            let Some(choice) = TrailerAutoplay::from_name(name) else { return false };
            config.theme.trailers = choice;
        }
        (SettingKey::Translation, V::Str(name)) => {
            let Some(translation) =
                [Translation::Sub, Translation::Dub].into_iter().find(|t| t.as_str() == name)
            else {
                return false;
            };
            playback.translation = translation;
        }
        (SettingKey::Quality, V::Int(quality)) => {
            let Ok(quality) = u32::try_from(*quality) else { return false };
            playback.quality = quality;
        }
        (SettingKey::Subtitles, V::Str(language)) => {
            playback.subtitle_language.clone_from(language);
        }
        (SettingKey::CommitThreshold, V::Float(threshold)) => {
            playback.commit_threshold = *threshold;
        }
        (SettingKey::AutoNext, V::Bool(on)) => playback.auto_next = *on,
        (SettingKey::SkipOpening, V::Bool(on)) => playback.skip_opening = *on,
        (SettingKey::SkipRecap, V::Bool(on)) => playback.skip_recap_auto = *on,
        (SettingKey::AskWasOpening, V::Bool(on)) => playback.ask_was_opening = *on,
        (SettingKey::FindSkipTimes, V::Bool(on)) => playback.find_skip_times = *on,
        (SettingKey::ShareSkipTimes, V::Bool(on)) => playback.share_skip_times = Some(*on),
        (SettingKey::SkipFiller, V::Bool(on)) => playback.skip_filler = *on,
        (SettingKey::MarkChapters, V::Bool(on)) => playback.mark_chapters = *on,
        (SettingKey::SkipButton, V::Bool(on)) => playback.skip_button = *on,
        (SettingKey::Fullscreen, V::Bool(on)) => playback.fullscreen = *on,
        (SettingKey::Upscaling, V::Str(name)) => {
            let Some(mode) = Upscaling::from_name(name) else { return false };
            playback.upscaling = mode;
        }
        (SettingKey::PlayerUi, V::Str(name)) => {
            playback.player_ui = match name.as_str() {
                "kaeri" => PlayerUi::Kaeri,
                "mpv" => PlayerUi::Mpv,
                _ => return false,
            };
        }
        (SettingKey::NightMode, V::Bool(on)) => playback.night_mode = *on,
        (SettingKey::DownloadDir, V::Str(dir)) => {
            config.downloads.directory = Some(dir.clone());
        }
        (SettingKey::MergeSubtitles, V::Bool(on)) => config.downloads.merge_subtitles = *on,
        (SettingKey::KeepSeeding, V::Bool(on)) => config.downloads.keep_seeding = *on,
        (SettingKey::DownloadHook, V::Str(command)) => {
            config.downloads.on_complete = Some(command.clone());
        }
        (SettingKey::Presence, V::Bool(on)) => config.presence.enabled = *on,
        (SettingKey::PresenceTitle, V::Bool(on)) => config.presence.show_title = *on,
        (SettingKey::PresenceRich, V::Bool(on)) => config.presence.rich = *on,
        (SettingKey::Torrents, V::Bool(on)) => config.providers.torrent.enabled = *on,
        (SettingKey::SeedRatio, V::Float(ratio)) => seeding.ratio = *ratio,
        (SettingKey::SeedHours, V::Int(hours)) => {
            let Ok(hours) = u32::try_from(*hours) else { return false };
            seeding.hours = hours;
        }
        (SettingKey::UploadLimit, V::Int(kib)) => {
            let Ok(kib) = u32::try_from(*kib) else { return false };
            seeding.upload_limit_kib = kib;
        }
        (SettingKey::Syncplay, V::Bool(on)) => config.syncplay.enabled = *on,
        (SettingKey::SyncplayServer, V::Str(server)) => {
            config.syncplay.server.clone_from(server);
        }
        (SettingKey::SyncplayRoom, V::Str(room)) => config.syncplay.room = Some(room.clone()),
        (SettingKey::UpdateCheck, V::Bool(on)) => config.updates.check = *on,
        (SettingKey::UpdateBeta, V::Bool(on)) => config.updates.beta = *on,
        (SettingKey::SourceOrder, V::Strings(order)) => {
            config.providers.order.clone_from(order);
        }
        (SettingKey::MpvOptions, V::Strings(options)) => {
            let Some(options) = mpv_options(options.iter().map(String::as_str)) else {
                return false;
            };
            if options.is_empty() && playback.mpv_args.is_empty() {
                return false;
            }
            playback.mpv_args = options;
        }
        // Typed, one to a line, as the row's text gives them. An empty field over no options
        // is nothing to write: a stray Enter in it saves nothing and says nothing.
        (SettingKey::MpvOptions, V::Str(lines)) => {
            let Some(options) = mpv_options(lines.lines()) else { return false };
            if options.is_empty() && playback.mpv_args.is_empty() {
                return false;
            }
            playback.mpv_args = options;
        }
        (SettingKey::UpNextCountdown, V::Int(seconds)) => {
            let Ok(seconds) = u32::try_from(*seconds) else { return false };
            playback.up_next_countdown = seconds;
        }
        (SettingKey::SpoilerProtection, V::Bool(on)) => playback.spoiler_protection = *on,
        (SettingKey::DownloadQuality, V::Str(name)) => {
            config.downloads.quality = match name.as_str() {
                "standard" => DownloadQuality::Standard,
                "higher" => DownloadQuality::Higher,
                _ => return false,
            };
        }
        (SettingKey::KeepNextEpisodes, V::Int(episodes)) => {
            let Ok(episodes) = u32::try_from(*episodes) else { return false };
            config.downloads.keep_next = episodes;
        }
        (SettingKey::DownloadsOnCellular, V::Bool(on)) => config.downloads.on_cellular = *on,
        (SettingKey::ChapterDownloadsOnCellular, V::Bool(on)) => {
            config.reading.downloads_on_cellular = *on;
        }
        (SettingKey::KeepChaptersReady, V::Bool(on)) => {
            config.reading.new_chapters_choice =
                Some(if *on { NewChapters::Reading } else { NewChapters::Off });
        }
        (SettingKey::KeepChaptersReady | SettingKey::NewChapters, V::Str(name)) => {
            let Some(choice) = NewChapters::from_name(name) else { return false };
            config.reading.new_chapters_choice = Some(choice);
        }
        (SettingKey::DeleteAfterReading, V::Bool(on)) => {
            config.reading.delete_after_reading = *on;
        }
        (SettingKey::OpdsFeed, V::Bool(on)) => config.reading.feed.on = *on,
        (SettingKey::OpdsPassword, V::Str(password)) => {
            config.reading.feed.password_set = !password.trim().is_empty();
        }
        (SettingKey::OpdsPort, V::Str(port)) => {
            let port = port.trim();
            config.reading.feed.port = if port.is_empty() || port == "0" {
                None
            } else {
                match port.parse::<u16>() {
                    Ok(n) if n >= 1024 => Some(n),
                    _ => return false,
                }
            };
        }
        (SettingKey::SeriesChoicesFollow, V::Bool(on)) => {
            config.reading.mode_follows = Some(*on);
        }
        (SettingKey::LookFollows, V::Bool(on)) => config.reading.look_follows = Some(*on),
        (SettingKey::WebtoonMode, V::Str(name)) => {
            let Some(choice) = WebtoonMode::from_name(name) else { return false };
            config.reading.webtoon.mode = choice;
        }
        (SettingKey::SendProgressAfter, V::Str(name)) => {
            let Some(choice) = SendProgressAfter::from_name(name) else { return false };
            config.reading.send_progress_after = choice;
        }
        (SettingKey::WhenTrackersDisagree, V::Str(name)) => {
            let Some(choice) = WhenTrackersDisagree::from_name(name) else { return false };
            config.reading.when_trackers_disagree = choice;
        }
        (SettingKey::CountsAsRead, V::Str(name)) => {
            let Some(choice) = CountsAsRead::from_name(name) else { return false };
            config.reading.counts_as_read = choice;
        }
        (SettingKey::ChapterNotifications, V::Bool(on)) => config.notifications.chapters = *on,
        (SettingKey::DownloadsFinished, V::Bool(on)) => config.notifications.downloads = *on,
        (SettingKey::OpenAt, V::Str(name)) => {
            if !matches!(name.as_str(), "watch" | "read") {
                return false;
            }
            config.reading.open_at.clone_from(name);
        }
        (SettingKey::EpisodeNotifications, V::Bool(on)) => config.notifications.episodes = *on,
        (SettingKey::AdultTitles, V::Bool(on)) => config.meta.adult = *on,
        (SettingKey::EpisodeDiscussions, V::Bool(on)) => config.meta.discussions = *on,
        (SettingKey::CommentImages, V::Bool(on)) => config.meta.discussion_images = *on,
        (SettingKey::ControllerInput, V::Bool(on)) => config.controller.enabled = *on,
        (SettingKey::SwapFaceButtons, V::Bool(on)) => config.controller.swap_face = *on,
        (SettingKey::CalendarMine, V::Bool(on)) => config.meta.calendar_mine = *on,
        (SettingKey::CastTvs, V::Bool(on)) => config.cast.show_tvs = *on,
        (SettingKey::TorrentsWhileCasting, V::Bool(on)) => {
            config.providers.torrent.while_casting = Some(*on);
        }
        (SettingKey::HoverCards, V::Bool(on)) => config.theme.hover_cards = *on,
        (SettingKey::SubtitleSize, V::Int(size)) => {
            let (low, high, by) = SubtitlesConfig::SIZES;
            let Ok(size) = u32::try_from(*size) else { return false };
            if !(low..=high).contains(&size) || (size - low) % by != 0 {
                return false;
            }
            config.subtitles.size = size;
        }
        (SettingKey::SubtitleOpacity, V::Int(opacity)) => {
            let Ok(opacity) = u32::try_from(*opacity) else { return false };
            if !SubtitlesConfig::OPACITIES.contains(&opacity) {
                return false;
            }
            config.subtitles.opacity = opacity;
        }
        (SettingKey::SubtitleRaise, V::Int(raise)) => {
            let Ok(raise) = u32::try_from(*raise) else { return false };
            if raise > SubtitlesConfig::MAX_RAISE {
                return false;
            }
            config.subtitles.raise = raise;
        }
        (SettingKey::SubtitleFont, V::Str(name)) => {
            let Some(choice) = SubtitleFont::from_name(name) else { return false };
            config.subtitles.font = choice;
        }
        (SettingKey::SubtitleColour, V::Str(name)) => {
            let Some(choice) = SubtitleColour::from_name(name) else { return false };
            config.subtitles.colour = choice;
        }
        (SettingKey::SubtitleEdge, V::Str(name)) => {
            let Some(choice) = SubtitleEdge::from_name(name) else { return false };
            config.subtitles.edge = choice;
        }
        (SettingKey::SubtitleThickness, V::Str(name)) => {
            let Some(choice) = SubtitleThickness::from_name(name) else { return false };
            config.subtitles.thickness = choice;
        }
        (SettingKey::SubtitleBackground, V::Str(name)) => {
            let Some(choice) = SubtitleBackground::from_name(name) else { return false };
            config.subtitles.background = choice;
        }
        (SettingKey::StyledSubtitles, V::Str(name)) => {
            let Some(choice) = StyledSubtitles::from_name(name) else { return false };
            config.subtitles.styled = choice;
        }
        (SettingKey::FollowSystemCaptions, V::Bool(on)) => config.subtitles.follow_system = *on,
        // The time it was turned on is the engine's to stamp: it reads the clock.
        (SettingKey::SaveCrashReports, V::Bool(on)) => config.crash_reports.save = *on,
        (SettingKey::AskAfterCrash, V::Bool(on)) => config.crash_reports.ask = *on,
        (SettingKey::TorrentsOnCellular, V::Bool(on)) => {
            config.providers.torrent.on_cellular = *on;
        }
        _ => return false,
    }
    true
}

/// A row's value, as it shows it.
pub(crate) fn value(effective: &Config, saved: &Config, key: SettingKey) -> String {
    use kaeri_core::config::{DownloadQuality, PlayerUi};

    let playback = &effective.playback;
    let torrent = &effective.providers.torrent;
    match key {
        SettingKey::Theme => session_marked(
            format!("{:?}", saved.theme.mode).to_lowercase(),
            format!("{:?}", effective.theme.mode).to_lowercase(),
        ),
        SettingKey::Motion => on_off(effective.theme.motion),
        SettingKey::Titles => match effective.meta.title_language {
            TitleChoice::Anilist => "AniList's choice",
            TitleChoice::English => "English",
            TitleChoice::Romaji => "Romaji",
        }
        .into(),
        SettingKey::Trailers => match effective.theme.trailers {
            TrailerAutoplay::Always => "always",
            TrailerAutoplay::WifiOnly => "Wi-Fi only",
            TrailerAutoplay::Off => "off",
        }
        .into(),
        SettingKey::Translation => session_marked(
            saved.playback.translation.to_string(),
            playback.translation.to_string(),
        ),
        SettingKey::Quality => format!("{}p", playback.quality),
        // Text, not a closed set — a language code cycler would be a list of every ISO 639
        // code, which is a config-file job rather than a screen job.
        SettingKey::Subtitles => playback.subtitle_language.clone(),
        SettingKey::CommitThreshold => {
            format!("{}%", (playback.commit_threshold * 100.0).round())
        }
        SettingKey::AutoNext => on_off(playback.auto_next),
        SettingKey::SkipOpening => on_off(playback.skip_opening),
        SettingKey::SkipRecap => on_off(playback.skip_recap_auto),
        SettingKey::AskWasOpening => on_off(playback.ask_was_opening),
        SettingKey::FindSkipTimes => on_off(playback.find_skip_times),
        SettingKey::ShareSkipTimes => on_off(playback.share_skip_times == Some(true)),
        SettingKey::SkipFiller => on_off(playback.skip_filler),
        SettingKey::MarkChapters => on_off(playback.mark_chapters),
        SettingKey::SkipButton => on_off(playback.skip_button),
        SettingKey::Fullscreen => on_off(playback.fullscreen),
        SettingKey::Upscaling => upscaling_label(playback.upscaling).into(),
        SettingKey::PlayerUi => match playback.player_ui {
            PlayerUi::Kaeri => "kaeri",
            PlayerUi::Mpv => "your mpv setup",
        }
        .into(),
        SettingKey::NightMode => on_off(playback.night_mode),
        SettingKey::DownloadDir => effective
            .downloads
            .directory
            .clone()
            .unwrap_or_else(|| "kept with the torrent cache".into()),
        SettingKey::MergeSubtitles => on_off(effective.downloads.merge_subtitles),
        SettingKey::KeepSeeding => on_off(effective.downloads.keep_seeding),
        SettingKey::DownloadHook => {
            if effective.downloads.on_complete.is_some() { "configured" } else { "none" }.into()
        }
        SettingKey::Syncplay => on_off(effective.syncplay.enabled),
        SettingKey::SyncplayServer => effective.syncplay.server.clone(),
        SettingKey::SyncplayRoom => effective
            .syncplay
            .room
            .clone()
            .unwrap_or_else(|| "unset — needed for parties".into()),
        SettingKey::UpdateCheck => on_off(effective.updates.check),
        SettingKey::UpdateBeta => on_off(effective.updates.beta),
        SettingKey::Presence => on_off(effective.presence.enabled),
        SettingKey::PresenceTitle => on_off(effective.presence.show_title),
        SettingKey::PresenceRich => on_off(effective.presence.rich),
        SettingKey::Torrents => on_off(torrent.enabled),
        SettingKey::TorrentProxy => match crate::proxy::view(torrent).proxy {
            Some(proxy) => format!("{}:{}", proxy.host, proxy.port),
            None => "none".into(),
        },
        SettingKey::SeedRatio => {
            if torrent.seeding.ratio > 0.0 {
                format!("until ratio {:.1}", torrent.seeding.ratio)
            } else {
                "off".into()
            }
        }
        SettingKey::SeedHours => format!("{} h", torrent.seeding.hours),
        SettingKey::UploadLimit => upload_label(torrent.seeding.upload_limit_kib),
        SettingKey::TokenStorage => effective.trackers.token_storage.clone(),
        // Not among the rows: Providers shows the order, as its table.
        SettingKey::SourceOrder => String::new(),
        SettingKey::MpvOptions => match effective.playback.mpv_args.len() {
            0 => "none".into(),
            n => crate::wording::plural(n, "option"),
        },
        SettingKey::UpNextCountdown => match playback.up_next_countdown {
            0 => "off".into(),
            seconds => format!("{seconds} s"),
        },
        SettingKey::SpoilerProtection => on_off(playback.spoiler_protection),
        SettingKey::DownloadQuality => match effective.downloads.quality {
            DownloadQuality::Standard => "standard",
            DownloadQuality::Higher => "higher",
        }
        .into(),
        SettingKey::KeepNextEpisodes => match effective.downloads.keep_next {
            0 => "off".into(),
            n => crate::wording::plural(n as usize, "episode"),
        },
        SettingKey::DownloadsOnCellular => on_off(effective.downloads.on_cellular),
        SettingKey::ChapterDownloadsOnCellular => {
            on_off(effective.reading.downloads_on_cellular)
        }
        SettingKey::KeepChaptersReady => on_off(keeps_ready(effective)),
        SettingKey::NewChapters => {
            effective.reading.new_chapters(NewChapters::Off).name().into()
        }
        SettingKey::DeleteAfterReading => on_off(effective.reading.delete_after_reading),
        SettingKey::OpdsFeed => on_off(effective.reading.feed.on),
        // The password is never read back.
        SettingKey::OpdsPassword => {
            if effective.reading.feed.password_set { "set" } else { "not set" }.into()
        }
        SettingKey::OpdsPort => match effective.reading.feed.port.filter(|p| *p != 0) {
            Some(port) => port.to_string(),
            None => "automatic".into(),
        },
        SettingKey::SeriesChoicesFollow => on_off(effective.reading.mode_follows()),
        SettingKey::LookFollows => on_off(effective.reading.look_follows()),
        SettingKey::WebtoonMode => effective.reading.webtoon.mode.name().into(),
        SettingKey::SendProgressAfter => effective.reading.send_progress_after.name().into(),
        SettingKey::WhenTrackersDisagree => {
            effective.reading.when_trackers_disagree.name().into()
        }
        SettingKey::CountsAsRead => effective.reading.counts_as_read.name().into(),
        SettingKey::ChapterNotifications => on_off(effective.notifications.chapters),
        SettingKey::DownloadsFinished => on_off(effective.notifications.downloads),
        SettingKey::OpenAt => effective.reading.open_at.clone(),
        SettingKey::TorrentsOnCellular => on_off(torrent.on_cellular),
        SettingKey::EpisodeNotifications => on_off(effective.notifications.episodes),
        SettingKey::AdultTitles => on_off(effective.meta.adult),
        SettingKey::EpisodeDiscussions => on_off(effective.meta.discussions),
        SettingKey::CommentImages => on_off(effective.meta.discussion_images),
        SettingKey::ControllerInput => on_off(effective.controller.enabled),
        SettingKey::SwapFaceButtons => on_off(effective.controller.swap_face),
        SettingKey::CalendarMine => on_off(effective.meta.calendar_mine),
        SettingKey::CastTvs => on_off(effective.cast.show_tvs),
        SettingKey::TorrentsWhileCasting => on_off(while_casting(effective)),
        SettingKey::HoverCards => on_off(effective.theme.hover_cards),
        SettingKey::SubtitleSize => format!("{}%", effective.subtitles.size),
        SettingKey::SubtitleFont => match effective.subtitles.font {
            SubtitleFont::Default => "default",
            SubtitleFont::ZenKakuGothicNew => "Zen Kaku Gothic New",
            SubtitleFont::Serif => "serif",
            SubtitleFont::Monospace => "monospace",
        }
        .into(),
        SettingKey::SubtitleColour => effective.subtitles.colour.name().into(),
        SettingKey::SubtitleEdge => match effective.subtitles.edge {
            SubtitleEdge::Outline => "outline",
            SubtitleEdge::DropShadow => "drop shadow",
            SubtitleEdge::NoEdge => "none",
        }
        .into(),
        SettingKey::SubtitleThickness => effective.subtitles.thickness.name().into(),
        SettingKey::SubtitleBackground => match effective.subtitles.background {
            SubtitleBackground::NoBackground => "none",
            SubtitleBackground::EachLine => "behind each line",
            SubtitleBackground::AllLines => "behind all lines",
        }
        .into(),
        SettingKey::SubtitleOpacity => format!("{}%", effective.subtitles.opacity),
        SettingKey::SubtitleRaise => format!("{}% of the picture", effective.subtitles.raise),
        SettingKey::StyledSubtitles => match effective.subtitles.styled {
            StyledSubtitles::Keep => "keep their look",
            StyledSubtitles::Restyle => "restyle",
            StyledSubtitles::Strip => "remove all styling",
        }
        .into(),
        SettingKey::FollowSystemCaptions => on_off(effective.subtitles.follow_system),
        SettingKey::SaveCrashReports => on_off(effective.crash_reports.save),
        SettingKey::AskAfterCrash => on_off(effective.crash_reports.ask),
    }
}

/// How a row changes: typed from the value in effect, stepped with its ends marked, or not
/// at all.
fn change(effective: &Config, saved: &Config, key: SettingKey) -> SettingChange {
    if key.typed() {
        return SettingChange::Typed { text: text(effective, key) };
    }
    // Not editable here: the proxy takes a form, and token storage is the file's.
    if key.path().is_none() {
        return SettingChange::Fixed;
    }
    // A two-state value steps like a two-rung ladder: back is off, or the first of its two
    // choices, and forward the other.
    if let Some(second) = two_state(effective, saved, key) {
        return SettingChange::Steps { back: second, forward: !second };
    }
    let now = ladder_value(effective, key);
    let moves = |delta| now.is_some() && step(effective, saved, key, delta) != now;
    SettingChange::Steps { back: moves(-1), forward: moves(1) }
}

/// Where a two-state row stands: `Some(true)` on its second state — on, or the second of
/// its two choices. `None` for every other row.
fn two_state(effective: &Config, saved: &Config, key: SettingKey) -> Option<bool> {
    use SettingKey as S;
    use kaeri_core::config::{PlayerUi, ThemeMode};
    let c = effective;
    Some(match key {
        // Theme and sub/dub can differ for this session (`--theme`, `t`), and their arrows
        // change the saved choice, so that is the one they stand on.
        S::Theme => saved.theme.mode != ThemeMode::Adaptive,
        S::Translation => saved.playback.translation == kaeri_core::media::Translation::Dub,
        S::PlayerUi => c.playback.player_ui == PlayerUi::Mpv,
        S::Motion => c.theme.motion,
        S::AutoNext => c.playback.auto_next,
        S::SkipOpening => c.playback.skip_opening,
        S::SkipRecap => c.playback.skip_recap_auto,
        S::AskWasOpening => c.playback.ask_was_opening,
        S::FindSkipTimes => c.playback.find_skip_times,
        S::ShareSkipTimes => c.playback.share_skip_times == Some(true),
        S::SkipFiller => c.playback.skip_filler,
        S::MarkChapters => c.playback.mark_chapters,
        S::SkipButton => c.playback.skip_button,
        S::Fullscreen => c.playback.fullscreen,
        S::NightMode => c.playback.night_mode,
        S::MergeSubtitles => c.downloads.merge_subtitles,
        S::KeepSeeding => c.downloads.keep_seeding,
        S::Presence => c.presence.enabled,
        S::PresenceTitle => c.presence.show_title,
        S::PresenceRich => c.presence.rich,
        S::Torrents => c.providers.torrent.enabled,
        S::Syncplay => c.syncplay.enabled,
        S::UpdateCheck => c.updates.check,
        S::UpdateBeta => c.updates.beta,
        S::SpoilerProtection => c.playback.spoiler_protection,
        S::DownloadQuality => {
            c.downloads.quality == kaeri_core::config::DownloadQuality::Higher
        }
        S::DownloadsOnCellular => c.downloads.on_cellular,
        S::ChapterDownloadsOnCellular => c.reading.downloads_on_cellular,
        S::KeepChaptersReady => keeps_ready(c),
        S::DeleteAfterReading => c.reading.delete_after_reading,
        S::OpdsFeed => c.reading.feed.on,
        S::SeriesChoicesFollow => c.reading.mode_follows(),
        S::LookFollows => c.reading.look_follows(),
        S::ChapterNotifications => c.notifications.chapters,
        S::DownloadsFinished => c.notifications.downloads,
        S::TorrentsOnCellular => c.providers.torrent.on_cellular,
        S::EpisodeNotifications => c.notifications.episodes,
        S::AdultTitles => c.meta.adult,
        S::EpisodeDiscussions => c.meta.discussions,
        S::CommentImages => c.meta.discussion_images,
        S::ControllerInput => c.controller.enabled,
        S::SwapFaceButtons => c.controller.swap_face,
        S::CalendarMine => c.meta.calendar_mine,
        S::CastTvs => c.cast.show_tvs,
        S::TorrentsWhileCasting => while_casting(c),
        S::HoverCards => c.theme.hover_cards,
        S::FollowSystemCaptions => c.subtitles.follow_system,
        S::SaveCrashReports => c.crash_reports.save,
        S::AskAfterCrash => c.crash_reports.ask,
        _ => return None,
    })
}

/// A laddered row's value as it would be written, to tell a step from a stay.
fn ladder_value(effective: &Config, key: SettingKey) -> Option<SettingValue> {
    use SettingKey as S;
    use SettingValue as V;
    let playback = &effective.playback;
    let seeding = &effective.providers.torrent.seeding;
    Some(match key {
        S::Quality => V::Int(i64::from(playback.quality)),
        S::CommitThreshold => V::Float((playback.commit_threshold * 100.0).round() / 100.0),
        S::Upscaling => V::Str(playback.upscaling.name().into()),
        S::Titles => V::Str(effective.meta.title_language.name().into()),
        S::OpenAt => V::Str(effective.reading.open_at.clone()),
        S::NewChapters => {
            V::Str(effective.reading.new_chapters(NewChapters::Off).name().into())
        }
        S::WebtoonMode => V::Str(effective.reading.webtoon.mode.name().into()),
        S::SendProgressAfter => V::Str(effective.reading.send_progress_after.name().into()),
        S::WhenTrackersDisagree => {
            V::Str(effective.reading.when_trackers_disagree.name().into())
        }
        S::CountsAsRead => V::Str(effective.reading.counts_as_read.name().into()),
        S::Trailers => V::Str(effective.theme.trailers.name().into()),
        S::SeedRatio => V::Float(seeding.ratio),
        S::SeedHours => V::Int(i64::from(seeding.hours)),
        S::UploadLimit => V::Int(i64::from(seeding.upload_limit_kib)),
        S::UpNextCountdown => V::Int(i64::from(playback.up_next_countdown)),
        S::KeepNextEpisodes => V::Int(i64::from(effective.downloads.keep_next)),
        S::SubtitleSize => V::Int(i64::from(effective.subtitles.size)),
        S::SubtitleOpacity => V::Int(i64::from(effective.subtitles.opacity)),
        S::SubtitleRaise => V::Int(i64::from(effective.subtitles.raise)),
        S::SubtitleFont => V::Str(effective.subtitles.font.name().into()),
        S::SubtitleColour => V::Str(effective.subtitles.colour.name().into()),
        S::SubtitleEdge => V::Str(effective.subtitles.edge.name().into()),
        S::SubtitleThickness => V::Str(effective.subtitles.thickness.name().into()),
        S::SubtitleBackground => V::Str(effective.subtitles.background.name().into()),
        S::StyledSubtitles => V::Str(effective.subtitles.styled.name().into()),
        _ => return None,
    })
}

/// The live value a typed setting starts editing from.
fn text(effective: &Config, key: SettingKey) -> String {
    match key {
        SettingKey::Subtitles => effective.playback.subtitle_language.clone(),
        SettingKey::DownloadDir => effective.downloads.directory.clone().unwrap_or_default(),
        SettingKey::DownloadHook => effective.downloads.on_complete.clone().unwrap_or_default(),
        SettingKey::SyncplayServer => effective.syncplay.server.clone(),
        SettingKey::SyncplayRoom => effective.syncplay.room.clone().unwrap_or_default(),
        // Typing a new password starts empty: the old one is not shown.
        SettingKey::OpdsPort => effective
            .reading
            .feed
            .port
            .filter(|p| *p != 0)
            .map(|p| p.to_string())
            .unwrap_or_default(),
        SettingKey::MpvOptions => effective.playback.mpv_args.join("\n"),
        _ => String::new(),
    }
}

/// The options a viewer wrote, each as mpv's command line takes one, or `None` when one is not
/// an option: `--name` or `--name=value`, the name without spaces. Blank lines are passed over
/// and the rest kept as written, since mpv is the judge of what its options mean.
fn mpv_options<'a>(written: impl Iterator<Item = &'a str>) -> Option<Vec<String>> {
    written
        .map(str::trim)
        .filter(|option| !option.is_empty())
        .map(|option| {
            let name = option.strip_prefix("--")?.split('=').next()?;
            let named = !name.is_empty() && !name.contains(char::is_whitespace);
            named.then(|| option.to_owned())
        })
        .collect()
}

fn upscaling_label(mode: kaeri_core::config::Upscaling) -> &'static str {
    use kaeri_core::config::Upscaling as U;
    match mode {
        U::Off => "off",
        U::Natural => "natural · ArtCNN",
        U::Sharp => "sharp · Anime4K",
    }
}

/// The value as the file takes it: the switch form of new chapters writes the picker's choice,
/// and mpv options typed one to a line write a list.
pub fn as_written(key: SettingKey, value: SettingValue) -> SettingValue {
    match (key, value) {
        (SettingKey::KeepChaptersReady, SettingValue::Bool(on)) => SettingValue::Str(
            if on { NewChapters::Reading } else { NewChapters::Off }.name().into(),
        ),
        // Typed one to a line, and kept in the file as a list: a string there would leave the
        // file unreadable.
        (SettingKey::MpvOptions, SettingValue::Str(lines)) => {
            SettingValue::Strings(mpv_options(lines.lines()).unwrap_or_default())
        }
        (_, value) => value,
    }
}

/// Whether new chapters download, as the switch form of the setting shows it.
fn keeps_ready(config: &Config) -> bool {
    config.reading.new_chapters(NewChapters::Off) != NewChapters::Off
}

fn on_off(yes: bool) -> String {
    if yes { "on".into() } else { "off".into() }
}

/// A setting's saved value, with the session's own beside it when they differ.
fn session_marked(saved: String, current: String) -> String {
    if saved == current { saved } else { format!("{saved} · {current} this session") }
}

/// An upload cap as people read it: `unlimited`, `524 kB/s`, `2.1 MB/s`.
///
/// The config counts KiB/s, and this said `KB/s` and `MB/s` of those — binary values under
/// the decimal names every other size here uses. Converted rather than relabelled, so the
/// cap reads in the same units as the download sizes beside it.
fn upload_label(kib: u32) -> String {
    match kib {
        0 => "unlimited".into(),
        k => format!("{}/s", crate::wording::human_bytes(u64::from(k) * 1024)),
    }
}

/// The name of the choice a step of `delta` makes of `current`, clamping at both ends.
fn stepped<T: Copy + PartialEq + Named>(all: &[T], current: T, delta: i64) -> String {
    let at = all.iter().position(|c| *c == current).unwrap_or(0) as i64;
    all[(at + delta.signum()).clamp(0, all.len() as i64 - 1) as usize].name().into()
}

/// A choice with the name the config file uses.
trait Named {
    fn name(self) -> &'static str;
}

macro_rules! named {
    ($($t:ty),+) => { $(impl Named for $t { fn name(self) -> &'static str { <$t>::name(self) } })+ };
}
named!(
    SubtitleFont,
    SubtitleColour,
    SubtitleEdge,
    SubtitleThickness,
    SubtitleBackground,
    StyledSubtitles,
    NewChapters,
    WebtoonMode,
    SendProgressAfter,
    WhenTrackersDisagree,
    CountsAsRead
);

/// Step to the next value in an ascending ladder, clamping at both ends.
///
/// Clamps rather than wraps: arrowing past 2160p round to 480p would be a surprising way to
/// lose your quality preference, and the ends of the ladder are informative.
///
/// A value that is not on the ladder — one typed into config.toml, like 1440p — steps to the
/// nearest rung in the arrow's direction, and stays put if there is none that way. It used
/// to be read as the bottom rung, so → from 1440p saved 720p and → from 97% saved 80%.
fn step_through<T: Copy + PartialOrd>(ladder: &[T], current: T, delta: i64) -> T {
    if let Some(at) = ladder.iter().position(|v| *v == current) {
        let next = (at as i64 + delta.signum()).clamp(0, ladder.len() as i64 - 1) as usize;
        return ladder[next];
    }
    let toward = if delta > 0 {
        ladder.iter().find(|v| **v > current)
    } else {
        ladder.iter().rev().find(|v| **v < current)
    };
    toward.copied().unwrap_or(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_step_from_anilists_choice_to_english_to_romaji() {
        let default = Config::default();
        let titles = |config: &Config| value(config, config, SettingKey::Titles);
        assert_eq!(titles(&default), "AniList's choice");
        assert_eq!(SettingKey::at(&["meta"], "title_language"), Some(SettingKey::Titles));
        assert_eq!(SettingKey::Titles.category(), "appearance");
        let step_to = |config: &Config, delta| step(config, config, SettingKey::Titles, delta);
        assert_eq!(step_to(&default, 1), Some(SettingValue::Str("english".into())));
        assert_eq!(step_to(&default, -1), Some(SettingValue::Str("anilist".into())), "an end");
        assert_eq!(
            change(&default, &default, SettingKey::Titles),
            SettingChange::Steps { back: false, forward: true }
        );

        let mut romaji = Config::default();
        assert!(put(&mut romaji, SettingKey::Titles, &SettingValue::Str("romaji".into())));
        assert_eq!(titles(&romaji), "Romaji");
        assert_eq!(
            step_to(&romaji, 1),
            Some(SettingValue::Str("romaji".into())),
            "the other end"
        );
        assert_eq!(
            change(&romaji, &romaji, SettingKey::Titles),
            SettingChange::Steps { back: true, forward: false }
        );
        assert!(put(&mut romaji, SettingKey::Titles, &SettingValue::Str("english".into())));
        assert_eq!(titles(&romaji), "English");
        assert!(!put(&mut romaji, SettingKey::Titles, &SettingValue::Str("native".into())));
        assert_eq!(titles(&romaji), "English", "a value it does not take changes nothing");

        let mut copied = Config::default();
        SettingKey::Titles.copy(&romaji, &mut copied);
        assert_eq!(titles(&copied), "English");
    }

    #[test]
    fn hover_cards_are_a_switch_of_their_own_beside_trailers() {
        let default = Config::default();
        assert_eq!(value(&default, &default, SettingKey::HoverCards), "on");
        assert_eq!(SettingKey::at(&["theme"], "hover_cards"), Some(SettingKey::HoverCards));
        assert_eq!(SettingKey::HoverCards.category(), "appearance");
        assert_eq!(
            step(&default, &default, SettingKey::HoverCards, 1),
            Some(SettingValue::Bool(false))
        );

        let mut off = Config::default();
        assert!(put(&mut off, SettingKey::HoverCards, &SettingValue::Bool(false)));
        assert!(!off.theme.hover_cards);
        assert_eq!(value(&off, &off, SettingKey::HoverCards), "off");
        assert_eq!(off.theme.trailers, TrailerAutoplay::Always, "trailers stay as they were");
        assert!(put(&mut off, SettingKey::Trailers, &SettingValue::Str("off".into())));
        assert!(!off.theme.hover_cards, "and the other way round");

        let mut copied = Config::default();
        SettingKey::HoverCards.copy(&off, &mut copied);
        assert!(!copied.theme.hover_cards);
    }

    #[test]
    fn trailers_step_from_always_to_wifi_only_to_off_under_the_titles_row() {
        let default = Config::default();
        let trailers = |config: &Config| value(config, config, SettingKey::Trailers);
        assert_eq!(trailers(&default), "always");
        assert_eq!(SettingKey::at(&["theme"], "trailers"), Some(SettingKey::Trailers));
        let at = |key| SettingKey::ROWS.iter().position(|row| *row == key);
        assert_eq!(at(SettingKey::Trailers), at(SettingKey::Titles).map(|i| i + 1));
        assert_eq!(SettingKey::Trailers.category(), "appearance");

        let step_to =
            |config: &Config, delta| step(config, config, SettingKey::Trailers, delta);
        assert_eq!(step_to(&default, 1), Some(SettingValue::Str("wifi-only".into())));
        assert_eq!(step_to(&default, -1), Some(SettingValue::Str("always".into())), "an end");
        assert_eq!(
            change(&default, &default, SettingKey::Trailers),
            SettingChange::Steps { back: false, forward: true }
        );

        let mut off = Config::default();
        assert!(put(&mut off, SettingKey::Trailers, &SettingValue::Str("wifi-only".into())));
        assert_eq!(trailers(&off), "Wi-Fi only");
        assert!(put(&mut off, SettingKey::Trailers, &SettingValue::Str("off".into())));
        assert_eq!(trailers(&off), "off");
        assert_eq!(
            change(&off, &off, SettingKey::Trailers),
            SettingChange::Steps { back: true, forward: false }
        );
        assert!(!put(&mut off, SettingKey::Trailers, &SettingValue::Str("sometimes".into())));
        assert_eq!(trailers(&off), "off", "a value it does not take changes nothing");

        let mut copied = Config::default();
        SettingKey::Trailers.copy(&off, &mut copied);
        assert_eq!(trailers(&copied), "off");
    }

    #[test]
    fn every_setting_written_is_found_again_where_the_file_keeps_it() {
        // So a write named by the file's table and key names one setting, and only one.
        let every = SettingKey::ROWS.into_iter().chain(SettingKey::NATIVE_ROWS);
        // The switch form of new chapters writes the picker's key, which names the picker.
        let every = every.filter(|key| *key != SettingKey::KeepChaptersReady);
        for setting in every.chain([SettingKey::SourceOrder]) {
            if let Some((table, key)) = setting.path() {
                assert_eq!(SettingKey::at(table, key), Some(setting), "{table:?}.{key}");
            }
        }
        // Shown and never written from here: the file's to change.
        assert_eq!(SettingKey::TorrentProxy.path(), None);
        assert_eq!(SettingKey::TokenStorage.path(), None);
        // What the player remembers in passing is no setting of the screen's.
        for key in ["persisted_speed", "persisted_volume", "tone_mapping"] {
            assert_eq!(SettingKey::at(&["playback"], key), None, "{key}");
        }
        // Every row once, and the order, which has none.
        let rows: std::collections::HashSet<_> = SettingKey::ROWS.into_iter().collect();
        assert_eq!(rows.len(), SettingKey::ROWS.len());
        assert!(!SettingKey::ROWS.contains(&SettingKey::SourceOrder));
        // The native apps' rows: each once, and none the terminal draws.
        let native: std::collections::HashSet<_> =
            SettingKey::NATIVE_ROWS.into_iter().collect();
        assert_eq!(native.len(), SettingKey::NATIVE_ROWS.len());
        assert!(
            native.iter().all(|key| !rows.contains(key) && *key != SettingKey::SourceOrder)
        );
    }

    #[test]
    fn the_native_rows_step_put_and_read_back_as_the_file_takes_them() {
        use kaeri_core::config::DownloadQuality;
        let config = Config::default();
        let view = view(&config, &config);
        let shown: Vec<_> =
            view.native_rows.iter().map(|row| (row.key, row.value.as_str())).collect();
        assert_eq!(
            shown,
            [
                (SettingKey::UpNextCountdown, "10 s"),
                (SettingKey::SpoilerProtection, "on"),
                (SettingKey::DownloadQuality, "standard"),
                (SettingKey::KeepNextEpisodes, "off"),
                (SettingKey::DownloadsOnCellular, "off"),
                (SettingKey::TorrentsOnCellular, "off"),
                (SettingKey::EpisodeNotifications, "off"),
                (SettingKey::MpvOptions, "none"),
                (SettingKey::AdultTitles, "off"),
                (SettingKey::EpisodeDiscussions, "on"),
                (SettingKey::CommentImages, "off"),
                (SettingKey::ControllerInput, "on"),
                (SettingKey::SwapFaceButtons, "off"),
                (SettingKey::CalendarMine, "off"),
                (SettingKey::CastTvs, "on"),
                (SettingKey::TorrentsWhileCasting, "on"),
                (SettingKey::HoverCards, "on"),
                (SettingKey::FindSkipTimes, "on"),
                (SettingKey::ChapterDownloadsOnCellular, "off"),
                (SettingKey::NewChapters, "off"),
                (SettingKey::KeepChaptersReady, "off"),
                (SettingKey::DeleteAfterReading, "off"),
                (SettingKey::SeriesChoicesFollow, "on"),
                (SettingKey::ChapterNotifications, "off"),
                (SettingKey::DownloadsFinished, "off"),
                (SettingKey::OpenAt, "watch"),
                (SettingKey::LookFollows, "on"),
                (SettingKey::WebtoonMode, "strip"),
                (SettingKey::SendProgressAfter, "chapter"),
                (SettingKey::WhenTrackersDisagree, "furthest"),
                (SettingKey::CountsAsRead, "last-page"),
            ]
        );
        assert!(view.rows.iter().all(|row| !SettingKey::NATIVE_ROWS.contains(&row.key)));

        // Each one stepped is a value it takes, and changes it. The typed ones are typed.
        for key in SettingKey::NATIVE_ROWS.into_iter().filter(|key| !key.typed()) {
            let stepped = step(&config, &config, key, 1).expect("a step");
            let mut changed = config.clone();
            assert!(put(&mut changed, key, &stepped), "{key:?} takes {stepped:?}");
            assert_ne!(changed, config, "{key:?} changed nothing");
            let row = native_rows(&config, &config).into_iter().find(|r| r.key == key).unwrap();
            let SettingChange::Steps { back, forward } = row.change else { panic!("{key:?}") };
            assert!(back || forward, "{key:?} steps neither way");
        }
        assert_eq!(
            step(&config, &config, SettingKey::KeepNextEpisodes, 1),
            Some(SettingValue::Int(1))
        );
        assert_eq!(
            step(&config, &config, SettingKey::UpNextCountdown, -1),
            Some(SettingValue::Int(5))
        );

        let mut config = config;
        assert!(put(
            &mut config,
            SettingKey::DownloadQuality,
            &SettingValue::Str("higher".into())
        ));
        assert_eq!(config.downloads.quality, DownloadQuality::Higher);
        assert!(!put(
            &mut config,
            SettingKey::DownloadQuality,
            &SettingValue::Str("best".into())
        ));
        assert!(put(&mut config, SettingKey::UpNextCountdown, &SettingValue::Int(0)));
        assert_eq!(value(&config, &config, SettingKey::UpNextCountdown), "off");
        assert!(put(&mut config, SettingKey::KeepNextEpisodes, &SettingValue::Int(3)));
        assert_eq!(value(&config, &config, SettingKey::KeepNextEpisodes), "3 episodes");
    }

    #[test]
    fn the_reading_choices_write_their_own_keys_and_the_follows_part() {
        let mut config = Config::default();
        // The switch form of new chapters is written as the picker's choice.
        let on = SettingValue::Bool(true);
        assert_eq!(
            as_written(SettingKey::KeepChaptersReady, on.clone()),
            SettingValue::Str("reading".into())
        );
        assert!(put(&mut config, SettingKey::KeepChaptersReady, &on));
        assert_eq!(value(&config, &config, SettingKey::NewChapters), "reading");
        assert_eq!(value(&config, &config, SettingKey::KeepChaptersReady), "on");
        assert!(!put(&mut config, SettingKey::NewChapters, &SettingValue::Str("all".into())));
        // One follow off leaves the other on.
        assert!(put(&mut config, SettingKey::LookFollows, &SettingValue::Bool(false)));
        assert!(config.reading.mode_follows() && !config.reading.look_follows());
        assert_eq!(
            SettingKey::SeriesChoicesFollow.path(),
            Some((&["reading"][..], "mode_follows"))
        );
        // Webtoons step to pages, the trackers' choices to their second.
        let step_to = |config: &Config, key| step(config, config, key, 1);
        assert_eq!(
            step_to(&config, SettingKey::WebtoonMode),
            Some(SettingValue::Str("paged".into()))
        );
        assert_eq!(
            step_to(&config, SettingKey::SendProgressAfter),
            Some(SettingValue::Str("close".into()))
        );
        assert_eq!(
            step_to(&config, SettingKey::WhenTrackersDisagree),
            Some(SettingValue::Str("latest".into()))
        );
        assert_eq!(
            step_to(&config, SettingKey::CountsAsRead),
            Some(SettingValue::Str("moving-on".into()))
        );
        assert!(kaeri_core::shared_settings::is_shared(&["reading"], "counts_as_read"));
        assert_eq!(SettingKey::WebtoonMode.path(), Some((&["reading", "webtoon"][..], "mode")));
        assert!(kaeri_core::shared_settings::is_shared(&["reading", "webtoon"], "mode"));
    }

    #[test]
    fn the_proxy_row_shows_where_torrent_traffic_goes() {
        let mut config = Config::default();
        assert_eq!(value(&config, &config, SettingKey::TorrentProxy), "none");
        config.providers.torrent.proxy = Some("socks5://mika:pw@192.0.2.10:1080".into());
        assert_eq!(value(&config, &config, SettingKey::TorrentProxy), "192.0.2.10:1080");
    }

    #[test]
    fn a_row_says_which_way_a_step_would_change_it() {
        let mut config = Config::default();
        let row = |config: &Config, key| {
            rows(config, config).into_iter().find(|row| row.key == key).unwrap().change
        };
        // A ladder, mid-way and at its top: 1080p, then 2160p.
        assert_eq!(
            row(&config, SettingKey::Quality),
            SettingChange::Steps { back: true, forward: true }
        );
        config.playback.quality = 2160;
        assert_eq!(
            row(&config, SettingKey::Quality),
            SettingChange::Steps { back: true, forward: false }
        );
        assert_eq!(
            step(&config, &config, SettingKey::Quality, 1),
            Some(SettingValue::Int(2160)),
            "a step past the end comes back as it is"
        );
        // Two states: on stands on its second, so only back changes it.
        assert_eq!(
            row(&config, SettingKey::AutoNext),
            SettingChange::Steps { back: true, forward: false }
        );
        // Typed from the value in effect; read-only not at all.
        assert_eq!(
            row(&config, SettingKey::Subtitles),
            SettingChange::Typed { text: "eng".into() }
        );
        assert_eq!(row(&config, SettingKey::TorrentProxy), SettingChange::Fixed);
    }

    #[test]
    fn a_value_a_setting_does_not_take_changes_nothing() {
        let mut config = Config::default();
        let before = config.clone();
        assert!(!put(&mut config, SettingKey::Quality, &SettingValue::Str("hd".into())));
        assert!(!put(&mut config, SettingKey::Upscaling, &SettingValue::Str("max".into())));
        assert!(!put(&mut config, SettingKey::Quality, &SettingValue::Int(-1)));
        assert!(!put(&mut config, SettingKey::TorrentProxy, &SettingValue::Str("none".into())));
        assert_eq!(config, before);

        assert!(put(&mut config, SettingKey::Upscaling, &SettingValue::Str("sharp".into())));
        assert_eq!(config.playback.upscaling, kaeri_core::config::Upscaling::Sharp);
        let order = SettingValue::Strings(vec!["plugins".into()]);
        assert!(put(&mut config, SettingKey::SourceOrder, &order));
        assert_eq!(config.providers.order, ["plugins"]);
    }

    #[test]
    fn a_value_off_the_ladder_steps_to_the_nearest_rung_the_arrow_points_at() {
        // From config.toml, not the arrows. → from 1440p saved 720p, and → from 97% saved 80%.
        const QUALITY: [u32; 4] = [480, 720, 1080, 2160];
        assert_eq!(step_through(&QUALITY, 1440, 1), 2160);
        assert_eq!(step_through(&QUALITY, 1440, -1), 1080);
        assert_eq!(step_through(&QUALITY, 360, -1), 360, "nothing lower: stay put");
        assert_eq!(step_through(&QUALITY, 360, 1), 480);

        const THRESHOLD: [u32; 5] = [70, 80, 85, 90, 95];
        assert_eq!(step_through(&THRESHOLD, 97, 1), 97, "nothing higher: stay put");
        assert_eq!(step_through(&THRESHOLD, 97, -1), 95);
        assert_eq!(step_through(&THRESHOLD, 82, 1), 85);
        assert_eq!(step_through(&THRESHOLD, 82, -1), 80);
    }

    #[test]
    fn only_source_defining_keys_force_a_rebuild() {
        // A rebuild tears down a live torrent session, so it must not fire on a setting
        // that has nothing to do with which sources exist.
        assert!(redefines_sources(SettingKey::Torrents));
        assert!(redefines_sources(SettingKey::TorrentProxy));
        assert!(redefines_sources(SettingKey::SourceOrder));

        assert!(!redefines_sources(SettingKey::Upscaling));
        assert!(!redefines_sources(SettingKey::Quality));
        assert!(!redefines_sources(SettingKey::DownloadDir));
        assert!(!redefines_sources(SettingKey::Theme));
        assert!(!redefines_sources(SettingKey::TorrentsOnCellular));
    }

    #[test]
    fn the_viewer_s_own_mpv_options_are_kept_as_written_and_shown_as_a_count() {
        let mut config = Config::default();
        assert_eq!(value(&config, &config, SettingKey::MpvOptions), "none");
        // An empty field over no options writes nothing.
        for empty in [SettingValue::Str(String::new()), SettingValue::Str(" \n ".into())] {
            assert!(!put(&mut config, SettingKey::MpvOptions, &empty), "{empty:?}");
        }
        assert!(!put(&mut config, SettingKey::MpvOptions, &SettingValue::Strings(Vec::new())));
        let options = SettingValue::Strings(vec![
            "--sub-font-size=40".into(),
            " --no-audio-display ".into(),
            String::new(),
        ]);
        assert!(put(&mut config, SettingKey::MpvOptions, &options));
        assert_eq!(config.playback.mpv_args, ["--sub-font-size=40", "--no-audio-display"]);
        assert_eq!(value(&config, &config, SettingKey::MpvOptions), "2 options");
        assert_eq!(
            change(&config, &config, SettingKey::MpvOptions),
            SettingChange::Typed { text: "--sub-font-size=40\n--no-audio-display".into() },
            "typed, one to a line"
        );
        // As typed: one to a line.
        let typed = SettingValue::Str("--hwdec=no\n\n--sub-scale=1.2".into());
        assert!(put(&mut config, SettingKey::MpvOptions, &typed));
        assert_eq!(config.playback.mpv_args, ["--hwdec=no", "--sub-scale=1.2"]);
        // The file keeps typed options as a list, an empty field as an empty one.
        assert_eq!(
            as_written(SettingKey::MpvOptions, typed),
            SettingValue::Strings(vec!["--hwdec=no".into(), "--sub-scale=1.2".into()])
        );
        assert_eq!(
            as_written(SettingKey::MpvOptions, SettingValue::Str(String::new())),
            SettingValue::Strings(Vec::new())
        );
        // Not options, so nothing changes.
        for wrong in ["sub-font-size=40", "--", "--=1", "--sub font=1"] {
            let refused = SettingValue::Strings(vec![wrong.into()]);
            assert!(!put(&mut config, SettingKey::MpvOptions, &refused), "{wrong}");
        }
        assert_eq!(config.playback.mpv_args, ["--hwdec=no", "--sub-scale=1.2"]);
        // Emptying the field clears the options that were there.
        assert!(put(&mut config, SettingKey::MpvOptions, &SettingValue::Str(String::new())));
        assert!(config.playback.mpv_args.is_empty());
        assert_eq!(SettingKey::at(&["playback"], "mpv_args"), Some(SettingKey::MpvOptions));
        assert_eq!(SettingKey::MpvOptions.category(), "advanced");
    }
}

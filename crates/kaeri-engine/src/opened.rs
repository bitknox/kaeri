//! A magnet link or torrent file the viewer opened themselves: read, matched to a title and an
//! episode by its name for the viewer to confirm, then played or queued as that episode, or
//! played with nothing recorded. See [`kaeri_model::opened`] for the flow.
//!
//! Nothing here finds a release. The link is the viewer's, and the guess only reads its name:
//! the words before the episode searched in the catalogue, and the episode the name states.

use kaeri_core::ids::AnilistId;
use kaeri_meta::{BrowseFilter, Media, title::similarity};
use kaeri_model::StoreMsg;
use kaeri_model::{
    Msg,
    catalog::Entry,
    event::Event,
    notice::Notice,
    opened::{OpenedCommand, OpenedGuess, OpenedLink},
};
use kaeri_providers::torrent::{names, open};

use crate::{
    data, dispatch::Lent, downloads, jobs::JobGuard, outbox::Outbox, services::Services,
    state::EngineState,
};

/// The provider id an opened link's stream and history go by: no source has it.
pub const PROVIDER: &str = "opened";

/// How many titles a guess or a search lists for Change.
const CANDIDATES: u32 = 8;

/// How alike a title's name and the link's must be for the engine to ask "Is this…?" about it,
/// rather than asking which title it is. Lower than the source matcher's floor: here the viewer
/// confirms every guess before anything is recorded.
const GUESS_FLOOR: f64 = 0.5;

/// Whether torrent playback is on, which playing or downloading an opened link needs.
pub fn playback_on(engine: &EngineState) -> bool {
    crate::sources::torrent_wanted(engine.config.effective())
}

/// Run a [`Command::Opened`](kaeri_model::command::Command::Opened).
pub fn run(
    command: OpenedCommand,
    job: Option<JobGuard>,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    match command {
        OpenedCommand::OpenLink { input } => open_link(input, job, engine, services, tx),
        OpenedCommand::OpenTorrentFile { bytes } => {
            open_torrent_file(bytes, job, engine, services, tx)
        }
        OpenedCommand::Find { link, query } => {
            find_opened(link, query, job, engine, services, tx)
        }
        OpenedCommand::Play { link, id, episode, title } => {
            #[cfg(feature = "party")]
            crate::party::join_by_default(engine, services, tx);
            crate::playback::start::play_opened(
                link, id, episode, title, job, engine, services, tx,
            )
        }
        OpenedCommand::PlayUntracked { link, name } => {
            crate::playback::start::play_opened_untracked(link, name, job, engine, services, tx)
        }
        OpenedCommand::Download { link, id, episode } => {
            download_opened(link, id, episode, job, engine, services, tx)
        }
    }
}

/// Read a magnet link, an info hash or a torrent file's path, and answer with the guess.
fn open_link(
    input: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let read = crate::sources::torrent_to_open(&input);
    answer(read, job, engine, services, tx);
}

/// [`open_link`] for a torrent file's bytes.
fn open_torrent_file(
    bytes: Vec<u8>,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let read = open::magnet_from_file(&bytes).ok_or(Msg::Store(StoreMsg::TorrentLinkUnusable));
    answer(read, job, engine, services, tx);
}

/// Guess what `read` is and say so, or say it is not a link.
fn answer(
    read: Result<String, Msg>,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let link = match read {
        Ok(link) => link,
        Err(msg) => {
            let _ = tx.send(Event::Toast(Notice::alert(msg)));
            return;
        }
    };
    let adult = engine.config.effective().meta.adult;
    let playback_on = playback_on(engine);
    let _ = crate::dispatch::spawn_abortable(job, services, tx, move |lent| async move {
        let name = open::name_of(&link);
        let words = name.as_deref().and_then(names::title_in);
        let found = match &words {
            Some(words) => search(&lent, words, adult).await.unwrap_or_default(),
            None => Vec::new(),
        };
        let (guess, candidates) = guess(name.as_deref(), words.as_deref(), &found, &lent);
        let opened = OpenedLink { link, name, guess, candidates, playback_on };
        let _ = lent.tx.send(Event::LinkOpened(Box::new(opened)));
    });
}

/// Change: titles matching `query`, for the link the viewer opened.
fn find_opened(
    link: String,
    query: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let adult = engine.config.effective().meta.adult;
    let _ = crate::dispatch::spawn_abortable(job, services, tx, move |lent| async move {
        let (candidates, failed) = match search(&lent, query.trim(), adult).await {
            Ok(found) => {
                (found.iter().map(|m| data::entry_from(m, Some(&lent.store))).collect(), None)
            }
            Err(reason) => {
                (Vec::new(), Some(Msg::Store(StoreMsg::OpenedSearchFailed { reason })))
            }
        };
        let _ = lent.tx.send(Event::OpenedChoices { link, query, candidates, failed });
    });
}

/// Queue an opened link for offline as episode `episode` of title `id`, as any episode is.
fn download_opened(
    link: String,
    id: AnilistId,
    episode: String,
    job: Option<JobGuard>,
    engine: &EngineState,
    services: &Services,
    tx: &Outbox,
) {
    if !playback_on(engine) {
        let _ =
            tx.send(Event::Toast(Notice::alert(Msg::Store(StoreMsg::TorrentLinkPlaybackOff))));
        return;
    }
    // The viewer's trackers on the link, as torrent playback adds them to a play.
    let link = services.registry.torrents().with_trackers(&link);
    let _ = crate::dispatch::spawn_abortable(job, services, tx, move |lent| async move {
        let queued =
            downloads::enqueue_link(&lent.store, &lent.meta, id, &episode, &link).await;
        let _ = lent.tx.send(Event::Toast(downloads::queued_notice(queued)));
        downloads::publish_now(&lent.store, &lent.tx);
    });
}

/// The catalogue's titles for `words`, best first by its own relevance.
async fn search(lent: &Lent, words: &str, adult: bool) -> Result<Vec<Media>, String> {
    if words.is_empty() {
        return Ok(Vec::new());
    }
    let filter = BrowseFilter { adult: (!adult).then_some(false), ..BrowseFilter::default() };
    lent.meta
        .search(words, &filter, 1, CANDIDATES)
        .await
        .map(|page| page.items)
        .map_err(|e| e.to_string())
}

/// The guess from what the search found for the link's `words`, and the titles Change lists:
/// the best match first, the rest in the catalogue's order.
fn guess(
    name: Option<&str>,
    words: Option<&str>,
    found: &[Media],
    lent: &Lent,
) -> (Option<OpenedGuess>, Vec<Entry>) {
    let best = words.and_then(|words| best_match(words, found));
    let mut order: Vec<usize> = (0..found.len()).collect();
    if let Some(best) = best {
        order.retain(|i| *i != best);
        order.insert(0, best);
    }
    let candidates: Vec<Entry> =
        order.iter().map(|i| data::entry_from(&found[*i], Some(&lent.store))).collect();
    let guess = best.map(|_| OpenedGuess {
        title: candidates[0].clone(),
        episode: name.and_then(names::episode_in).map(|n| n.to_string()),
    });
    (guess, candidates)
}

/// Which of `found` the words name, by the best likeness of any of its titles, when one is
/// alike enough to ask about. The catalogue's order breaks a tie.
fn best_match(words: &str, found: &[Media]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (index, media) in found.iter().enumerate() {
        let score =
            media.search_titles().iter().map(|t| similarity(words, t)).fold(0.0, f64::max);
        if score >= GUESS_FLOOR && best.is_none_or(|(_, top)| score > top) {
            best = Some((index, score));
        }
    }
    best.map(|(index, _)| index)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(id: u32, romaji: &str, english: Option<&str>) -> Media {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "title": { "romaji": romaji, "english": english },
            "format": "TV",
            "episodes": 12,
        }))
        .unwrap()
    }

    #[test]
    fn the_title_most_like_the_name_is_the_guess() {
        let found = [
            media(1, "Hoshikuzu no Tabibito Specials", None),
            media(2, "Hoshikuzu no Tabibito", Some("Stardust Wayfarer")),
            media(3, "Kumo no Ue no Toshokan", None),
        ];
        assert_eq!(best_match("Hoshikuzu no Tabibito", &found), Some(1));
        assert_eq!(best_match("Stardust Wayfarer", &found), Some(1), "by its English title");
    }

    #[test]
    fn nothing_alike_enough_is_no_guess() {
        let found = [media(3, "Kumo no Ue no Toshokan", None)];
        assert_eq!(best_match("Hagane no Niwashi", &found), None);
        assert_eq!(best_match("Hagane no Niwashi", &[]), None);
    }
}

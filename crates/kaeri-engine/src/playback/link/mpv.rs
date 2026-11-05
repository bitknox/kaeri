//! mpv's child process as a playback's player: each call the command it has always been, over
//! the socket, and the overlay told to the companion script.

use std::path::Path;

use async_trait::async_trait;
use kaeri_core::traits::PlaybackRequest;
use kaeri_player::{Mpv, MpvSession, PlayerError, companion, protocol::Command};

use super::{LinkError, Opened, Overlay, PlayerLink, PlayerOpener};

/// The fold line's words for every count the column can hold back, "1 more in chat" on: the
/// companion counts and picks, as the words are the terminal's language's.
fn fold_words() -> Vec<String> {
    (1..=FOLD_WORDS).map(kaeri_i18n::t::party_chat_more).collect()
}

/// Lines the fold has words for. The companion holds back no more than this: past it, the
/// oldest waiting line leaves the video unshown, and stays in the Together screen's
/// conversation.
const FOLD_WORDS: i64 = 40;

/// The reaction's glyph, by the name the companion draws it under.
fn glyph(reaction: kaeri_model::party::Reaction) -> &'static str {
    use kaeri_model::party::Reaction;
    match reaction {
        Reaction::Laugh => "laugh",
        Reaction::Gasp => "gasp",
        Reaction::Clap => "clap",
        Reaction::Cry => "cry",
    }
}

impl From<PlayerError> for LinkError {
    fn from(error: PlayerError) -> Self {
        Self(error.to_string())
    }
}

#[async_trait]
impl PlayerOpener for Mpv {
    async fn open(&self, request: &PlaybackRequest) -> Result<Opened, LinkError> {
        let (session, events) = self.spawn(request).await?;
        Ok((Box::new(session), events))
    }

    fn scratch_dir(&self) -> &Path {
        Mpv::scratch_dir(self)
    }
}

#[async_trait]
impl PlayerLink for MpvSession {
    async fn load(&self, url: &str, options: Vec<(String, String)>) -> Result<(), LinkError> {
        tracing::info!(%url, ?options, "loading into mpv");
        let index_slot = self.loadfile_index();
        Ok(self.send(Command::LoadFile { url: url.to_owned(), options, index_slot }).await?)
    }

    async fn add_subtitle(
        &self,
        url: &str,
        title: Option<String>,
        language: Option<String>,
    ) -> Result<(), LinkError> {
        Ok(self.send(Command::SubAdd { url: url.to_owned(), title, language }).await?)
    }

    async fn set(
        &self,
        property: &'static str,
        value: serde_json::Value,
    ) -> Result<(), LinkError> {
        Ok(self.send(Command::SetProperty(property, value)).await?)
    }

    async fn cycle(&self, property: &'static str) -> Result<(), LinkError> {
        Ok(self.send(Command::Cycle(property)).await?)
    }

    async fn add(&self, property: &'static str, delta: f64) -> Result<(), LinkError> {
        Ok(self.send(Command::Add(property, delta)).await?)
    }

    async fn multiply(&self, property: &'static str, factor: f64) -> Result<(), LinkError> {
        Ok(self.send(Command::Multiply(property, factor)).await?)
    }

    async fn seek(&self, seconds: f64, absolute: bool) -> Result<(), LinkError> {
        Ok(self.send(Command::Seek { seconds, absolute }).await?)
    }

    async fn shaders(&self, paths: &[String]) -> Result<(), LinkError> {
        Ok(self.set_shaders(paths).await?)
    }

    async fn audio_filter(&self, add: bool, filter: &str) -> Result<(), LinkError> {
        Ok(self.send(Command::AudioFilter { add, filter: filter.to_owned() }).await?)
    }

    async fn restate_chapters(&self) -> Result<(), LinkError> {
        Ok(MpvSession::restate_chapters(self).await?)
    }

    async fn overlay(&self, overlay: Overlay) -> Result<(), LinkError> {
        let command = match overlay {
            Overlay::Starting { card, .. } => companion::state("status", &card),
            Overlay::Playing => companion::state("status", &companion::Status::playing()),
            Overlay::Episode(episode) => companion::state("episode", &episode),
            Overlay::Next { card: Some(next), .. } => companion::state("next", &next),
            Overlay::Next { card: None, .. } => companion::clear("next"),
            // The companion says nothing of a resume point: the terminal does.
            Overlay::Resumed { .. } => return Ok(()),
            Overlay::Menus(menus) => companion::state("menus", &menus),
            Overlay::Media(media) => companion::state("media", &media),
            Overlay::ThumbnailUrl(Some(url)) => companion::state("thumbnail-url", &url),
            Overlay::ThumbnailUrl(None) => companion::clear("thumbnail-url"),
            Overlay::SkipOffer { kind, to } => {
                companion::skip_offer(kind.label(), kind.label(), to)
            }
            Overlay::SkipCleared => companion::skip_clear(),
            Overlay::Skipped { kind, from, to } => {
                companion::skipped(kind.label(), kind.label(), from, to)
            }
            Overlay::Toast { text, level } => companion::toast(&text, level),
            Overlay::ChatLine { who, you, text, .. } => {
                companion::chat_line(&who, you, &text, &fold_words())
            }
            Overlay::Reaction { who, reaction } => companion::reaction(&who, glyph(reaction)),
            Overlay::ChatCleared => companion::chat_clear(),
            Overlay::Stall { seconds } => companion::stall(seconds),
            Overlay::StallCleared => companion::stall_clear(),
            // The companion's seek bar reads mpv itself, chapters and cache alike.
            Overlay::Timeline(_) | Overlay::Buffered(_) | Overlay::Stats(_) => return Ok(()),
            // The companion has no marking bar yet: the hosted players draw it.
            Overlay::SkipMark(_) | Overlay::SkipPrompt(_) | Overlay::SkipNotice(_) => {
                return Ok(());
            }
        };
        Ok(self.send(command).await?)
    }

    async fn stop(&self) -> Result<(), LinkError> {
        Ok(MpvSession::stop(self).await?)
    }

    async fn quit(&self) -> Result<(), LinkError> {
        Ok(MpvSession::quit(self).await?)
    }

    fn is_alive(&self) -> bool {
        MpvSession::is_alive(self)
    }

    fn has_quit(&self) -> bool {
        MpvSession::has_quit(self)
    }

    async fn diagnostics(&self) -> Option<String> {
        MpvSession::diagnostics(self).most_relevant().await
    }

    async fn shutdown(self: Box<Self>) {
        MpvSession::shutdown(*self).await;
    }
}

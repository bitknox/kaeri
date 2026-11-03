//! Settings: a front end's asks for a change, carried out on the config the engine owns; what
//! the player changes on its own; and a setting written to the file, named as the Settings
//! screen names it, with the work the write leaves the engine side.

use kaeri_core::{media::Translation, settings::SettingValue};
use kaeri_model::{
    Msg,
    event::Event,
    notice::Notice,
    settings::{self, SettingKey, redefines_sources},
    sources::only_reorders,
};

use crate::{
    data,
    outbox::{Internal, Outbox},
    services::Services,
    state::EngineState,
};

/// Step `key` through its values, toward `delta`'s sign, by the Settings screen's rules, and
/// write what that makes of it: [`set_setting`]. A setting no step changes — typed, read-only,
/// the order — is left alone, and nothing is said.
pub fn step_setting(
    key: SettingKey,
    delta: i64,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let config = &engine.config;
    let Some(value) = settings::step(config.effective(), config.saved(), key, delta) else {
        return;
    };
    set_setting(key, value, engine, services, tx);
}

/// Put `value` in effect for `key` and write it, then say what the file holds, as
/// [`Event::Settings`]. A value the setting does not take, or a setting the file is the one
/// place to change, is left alone, and nothing is said.
///
/// In effect before it is written, so the work reads it the moment it is asked for; put back
/// by [`ConfigState::write`](crate::config::ConfigState::write) if the write is refused.
pub fn set_setting(
    key: SettingKey,
    value: SettingValue,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Some((table, file_key)) = key.path() else { return };
    if !settings::put(engine.config.effective_mut(), key, &value) {
        tracing::debug!(?key, ?value, "not a value this setting takes; nothing written");
        return;
    }
    // Switching saving on is the moment the system's own records, from the week before, become
    // offerable: written down once, and kept while saving stays on.
    if key == SettingKey::SaveCrashReports
        && matches!(value, kaeri_core::settings::SettingValue::Bool(true))
        && !engine.config.saved().crash_reports.save
    {
        let now = crate::crash::now_ms();
        let _ = engine.config.write(
            &services.paths,
            &["crash_reports"],
            "enabled_at",
            kaeri_core::settings::SettingValue::Int(now),
        );
    }
    write_setting(table, file_key, value, engine, services, tx);
    // The TV picker's Chromecast group comes and goes with its row.
    #[cfg(feature = "cast")]
    if key == SettingKey::CastTvs {
        services.casts.set_show_tvs(engine.config.effective().cast.show_tvs);
    }
    // What the file now holds, put in effect: titles are named the new way from here.
    if key == SettingKey::Titles {
        let choice = engine.config.effective().meta.title_language;
        crate::title_language::chosen(choice, services, tx);
    }
}

/// Sub or dub for this session only: in effect at once, and the file left as it is, so the next
/// launch goes by the saved choice. The Settings row shows it beside that choice.
pub fn set_translation(translation: Translation, engine: &mut EngineState, tx: &Outbox) {
    engine.config.effective_mut().playback.translation = translation;
    let _ = tx.send(Event::Settings(engine.config.view()));
}

/// The terminal's Hide chat for watch parties, in effect at once and kept in the file. It has
/// no Settings row: `i` in Together turns it over.
pub fn save_hide_chat(
    hidden: bool,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    use kaeri_core::settings::SettingValue as V;
    if engine.config.effective().syncplay.hide_chat == hidden {
        return;
    }
    engine.config.effective_mut().syncplay.hide_chat = hidden;
    write_setting(&["syncplay"], "hide_chat", V::Bool(hidden), engine, services, tx);
}

/// Write a setting the config already has in effect, and say how that went: the settings as
/// the file then leaves them, the "saved" or "not saved" notice, and the work the write leaves
/// the engine side.
///
/// In line, here, rather than off the actor: one small file, and a settings change that
/// silently lost a race with the next one would be worse than a paused frame. The screen is
/// right whatever the disk does, since what it is told is what is in effect afterwards.
fn write_setting(
    table: &'static [&'static str],
    key: &'static str,
    value: SettingValue,
    engine: &mut EngineState,
    services: &Services,
    tx: &Outbox,
) {
    let Services { paths, registry, .. } = services;
    // Against the file's order, not the one in effect: the new order is in effect already.
    let reordered = (table, key) == (&["providers"][..], "order")
        && matches!(&value, kaeri_core::settings::SettingValue::Strings(order)
            if only_reorders(&engine.config.saved().providers.order, order));
    let written = engine.config.write(paths, table, key, value);
    // What the write left, so the Settings screen shows the file rather than its own hopes,
    // with a refused setting put back and named.
    let _ = tx.send(Event::Settings(settings::SettingsView {
        refused: written.as_ref().err().and_then(|_| SettingKey::at(table, key)),
        ..engine.config.view()
    }));
    match written {
        Ok(()) => {
            // A toast, named as the Settings screen names it. As a status it stayed
            // in the status line for the rest of the session, over the state it shows,
            // and it said the file's key — "saved commit_threshold". What has no row
            // there, the remembered speed and volume, is saved without a word.
            if let Some(label) = setting_label(table, key) {
                let _ = tx.send(Event::Toast(Notice::info(Msg::NoticeSaved {
                    label: label.to_owned(),
                })));
            }
            // Republish what the background tasks read, so the next download to
            // finish honours the value that was just written.
            let _ = tx.internal(Internal::SettingsSaved);
            if (table, key) == (&["playback"][..], "share_skip_times") {
                let share = engine.config.effective().playback.share_skip_times;
                crate::aniskip::sharing_changed(share, &services.http, &services.store);
            }
            // A setting that redefines what sources exist takes effect now rather
            // than at the next launch. The engine side's loop owns the session and
            // its route, so it does the work, once what was said above is in; this
            // only says that it is needed. An order that only moves sources is put in
            // effect without a rebuild.
            if reordered {
                let _ = tx.internal(Internal::SourcesReordered);
            } else if SettingKey::at(table, key).is_some_and(redefines_sources) {
                let _ = tx.internal(Internal::SourcesChanged);
            }
        }
        // The config's rules reaching the UI: a value they refuse fails validation here, so
        // no arrow key can write a config that would not load. And a file that did not load
        // at launch is refused by name and line.
        Err(e) => {
            let reason = match e {
                kaeri_core::Error::Config(reason) => reason,
                other => other.to_string(),
            };
            let _ = tx.send(Event::Toast(Notice::alert(Msg::NoticeNotSaved { reason })));
            // The Providers rows moved when the key was pressed and the chain did not,
            // so the chain's own rows put them back.
            if (table, key) == (&["providers"][..], "order") {
                let rows = data::provider_rows(registry, &services.stores);
                let _ = tx.send(Event::Providers(rows));
            }
        }
    }
}

/// What the player changed of the config on its own, from its menus or its keys, kept in effect
/// for the next playback and saved as any setting is, as the event that says so passes on its
/// way to the front end, which is told the settings after it. `tx` is asked for only when there
/// is something to say.
///
/// The speed and the volume are remembered whatever the config says, and saved only when it
/// asks for them to be: `persist_speed`, `persist_volume`. Sub or dub picked in the player is
/// for this session, as `t` is, and not saved.
///
/// The interface used to do this as the event reached it, by queueing the save in its one slot
/// for work, which an ending's prefetch, auto-next and an episode picked in the player fill too.
/// A save that found the slot taken was skipped without a word, and the next launch came back
/// without the change. Here nothing can take its place: the write is made in line, in its turn,
/// whatever the front end does with its events.
pub fn remember(
    event: &Event,
    engine: &mut EngineState,
    services: &Services,
    tx: impl FnOnce() -> Outbox,
) {
    use kaeri_core::settings::SettingValue as V;
    // A party joined puts its server first among the ones the form offers, and keeps it there
    // across launches.
    if let Event::PartyJoined(view) = event {
        let syncplay = &mut engine.config.effective_mut().syncplay;
        let recent = syncplay.used(&view.server);
        let moved = recent != syncplay.recent_servers;
        // The first name given is kept, so the next party does not ask again.
        let name = (syncplay.name.trim().is_empty() && !view.name.trim().is_empty())
            .then(|| view.name.clone());
        if !moved && name.is_none() {
            return;
        }
        let tx = tx();
        if moved {
            syncplay.recent_servers.clone_from(&recent);
            write_setting(
                &["syncplay"],
                "recent_servers",
                V::Strings(recent),
                engine,
                services,
                &tx,
            );
        }
        if let Some(name) = name {
            engine.config.effective_mut().syncplay.name.clone_from(&name);
            write_setting(&["syncplay"], "name", V::Str(name), engine, services, &tx);
        }
        return;
    }
    let playback = &mut engine.config.effective_mut().playback;
    let saved = match event {
        Event::PlaybackSpeed(speed) => {
            playback.persisted_speed = Some(*speed);
            playback.persist_speed.then_some(("persisted_speed", V::Float(*speed)))
        }
        Event::PlaybackVolume(volume) => {
            playback.persisted_volume = Some(*volume);
            playback.persist_volume.then_some(("persisted_volume", V::Float(*volume)))
        }
        Event::PlayerUpscaling(name) => {
            let Some(mode) = kaeri_core::config::Upscaling::from_name(name) else { return };
            playback.upscaling = mode;
            Some(("upscaling", V::Str(name.clone())))
        }
        Event::PlayerToneMapping(curve) => {
            playback.tone_mapping.clone_from(curve);
            Some(("tone_mapping", V::Str(curve.clone())))
        }
        Event::PlayerNightMode(on) => {
            playback.night_mode = *on;
            Some(("night_mode", V::Bool(*on)))
        }
        Event::PlayerAskWasOpening(on) => {
            playback.ask_was_opening = *on;
            Some(("ask_was_opening", V::Bool(*on)))
        }
        Event::PlayerShareSkipTimes(on) => {
            playback.share_skip_times = Some(*on);
            Some(("share_skip_times", V::Bool(*on)))
        }
        Event::PlayerTranslation(translation) => {
            playback.translation = *translation;
            None
        }
        Event::PlayerSubtitleStyle { setting, value } => {
            let Some((key, value)) = subtitle_style_value(setting, value) else { return };
            let tx = tx();
            write_setting(&["subtitles"], key, value, engine, services, &tx);
            return;
        }
        _ => return,
    };
    let tx = tx();
    match saved {
        Some((key, value)) => {
            write_setting(&["playback"], key, value, engine, services, &tx);
        }
        // Kept for this session only: in effect, and the file left as it is.
        None => {
            let _ = tx.send(Event::Settings(engine.config.view()));
        }
    }
}

/// A subtitle style row from the player's panel as a setting to write: only the rows the panel
/// has, and only values the setting accepts.
fn subtitle_style_value(setting: &str, value: &str) -> Option<(&'static str, SettingValue)> {
    use kaeri_core::config::{StyledSubtitles, SubtitleBackground, SubtitlesConfig};
    use kaeri_core::settings::SettingValue as V;
    Some(match setting {
        "size" => {
            let (low, high, _) = SubtitlesConfig::SIZES;
            ("size", V::Int(value.parse::<i64>().ok()?.clamp(i64::from(low), i64::from(high))))
        }
        "raise" => (
            "raise",
            V::Int(value.parse::<i64>().ok()?.clamp(0, i64::from(SubtitlesConfig::MAX_RAISE))),
        ),
        "background" => {
            SubtitleBackground::from_name(value)?;
            ("background", V::Str(value.to_owned()))
        }
        "styled" => {
            StyledSubtitles::from_name(value)?;
            ("styled", V::Str(value.to_owned()))
        }
        _ => return None,
    })
}

/// The name the Settings screen gives the setting stored at `table`.`key`, for saying it was
/// saved. The config key (`commit_threshold`) is the file's vocabulary, not the screen's.
///
/// `None` for what is stored without a row of its own, the remembered speed and volume: saved
/// in passing, and not something anyone asked to have confirmed.
///
/// Read off the setting's own key, which names it for every front end: the engine had a table
/// of its own, held to the Settings screen's rows by a test.
pub fn setting_label(table: &[&str], key: &str) -> Option<&'static str> {
    SettingKey::at(table, key).map(SettingKey::label)
}

#[cfg(test)]
mod subtitle_style_tests {
    use super::*;

    #[test]
    fn the_panels_rows_become_settings_in_range() {
        assert!(matches!(
            subtitle_style_value("size", "300"),
            Some(("size", SettingValue::Int(200)))
        ));
        assert!(matches!(
            subtitle_style_value("raise", "-4"),
            Some(("raise", SettingValue::Int(0)))
        ));
        assert!(subtitle_style_value("background", "each-line").is_some());
        assert!(subtitle_style_value("background", "glow").is_none());
        assert!(subtitle_style_value("styled", "strip").is_some());
        assert!(subtitle_style_value("font", "serif").is_none());
    }
}

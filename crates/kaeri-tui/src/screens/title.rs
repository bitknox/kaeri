//! The Title screen — the key visual as hero, then the title block — and the rows of the overlays
//! opened from it: the list status and rating pickers, the source picker, the watch order and the
//! recommendations.

use kaeri_i18n::t;
use kaeri_model::{
    catalog::{Entry, LibrarySegment},
    missing::{AiredAgo, CheckOutcome, MissingEpisode, MissingPhase, aired_ago},
    progress::TitleProgress,
};
use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

use crate::{
    app::{App, MissingStatus, push_detail, status_text},
    theme::{Role, glyph},
    widgets::{Cells, truncate, wrap, wrap_meta},
};

use super::{
    chrome::render_placeholder,
    draw_artwork, line,
    lists::{airing_fact_text, metadata_line},
    set_meter,
};

/// The Title screen: key visual as hero, then the title block.
pub(super) fn render_title(buf: &mut Buffer, app: &App, area: Rect) {
    let Some(entry) = app.target() else {
        render_placeholder(
            buf,
            app,
            area,
            &t::title_heading(),
            &t::title_nothing_selected_note(),
        );
        return;
    };

    let mut y = area.top();

    // Full-bleed banner: the most characteristic artefact of the subject, given the most
    // space. Unlike the cover plate this genuinely is wide, so it keeps the full width.
    // The lower clamp asks for three rows even when the stage has one, so it is capped by what
    // actually exists — a minimum is a preference, not a promise the terminal has to keep.
    let banner_height = (area.height / 4).clamp(3, 8).min(area.height);
    let banner_area = Rect { height: banner_height, y, ..area };
    // Banner first, cover as the fallback: not every title has a banner, and an empty hero
    // is worse than a portrait one.
    let hero = entry.banner_url.as_deref().or(entry.cover_url.as_deref());
    draw_artwork(buf, app, hero, banner_area);
    y += banner_height + 1;

    // Every line below stops at the stage's own bottom, not the buffer's: on a short terminal
    // the banner leaves only a few rows, and the metadata was drawn on into the status line.
    let fits = |y: u16| y < area.bottom();

    // Bold, not tracked. Letterspacing is for eyebrows and metadata; applied to a real
    // title it doubles the width and truncates the one thing the screen exists to show.
    if fits(y) {
        line(
            buf,
            area.left(),
            y,
            truncate(&entry.title, area.width as usize),
            app.palette.style(Role::Text).add_modifier(Modifier::BOLD),
        );
    }
    y += 1;
    if let Some(secondary) = entry.secondary.as_ref().filter(|_| fits(y)) {
        line(
            buf,
            area.left(),
            y,
            truncate(secondary, area.width as usize),
            app.palette.style(Role::TextDim),
        );
    }
    y += 2;

    // The facts wrap onto a second line rather than lose the studio to an ellipsis, and the
    // run's state follows them: a show on a break or stopped for good says so.
    let mut facts = wrap_meta(&metadata_line(entry), area.width as usize, 2);
    facts.extend(entry.airing.map(airing_fact_text));
    for fact in &facts {
        if fits(y) {
            line(
                buf,
                area.left(),
                y,
                truncate(fact, area.width as usize),
                app.palette.style(Role::TextDim),
            );
        }
        y += 1;
    }
    y += 1;

    if !entry.available_on.is_empty() && y < area.bottom() {
        let services = t::title_available_on(&entry.available_on.join("  ·  "));
        buf.set_string(
            area.left(),
            y,
            truncate(&services, area.width as usize),
            app.palette.style(Role::TextDim),
        );
        y += 2;
    }

    // The missing episode's status is worded first, so a short page gives up synopsis lines
    // before the status: it is what the page has to say right now.
    let status = app.missing_status();
    let status_lines = status
        .as_ref()
        .map(|s| missing_lines(app, s, area.width as usize, 4))
        .unwrap_or_default();
    let status_room = if status_lines.is_empty() { 0 } else { status_lines.len() + 1 };
    let budget = if app.synopsis_expanded { 12 } else { 5 };
    let room = (area.bottom().saturating_sub(y).saturating_sub(4) as usize)
        .saturating_sub(status_room);
    let lines = wrap(&entry.synopsis, area.width as usize, budget.min(room));
    for (i, line) in lines.iter().enumerate() {
        buf.set_string(area.left(), y + i as u16, line, app.palette.style(Role::TextDim));
    }
    y += lines.len() as u16 + 1;

    if !entry.genres.is_empty() && y < area.bottom() {
        buf.set_string(
            area.left(),
            y,
            truncate(&entry.genres.join("  ·  "), area.width as usize),
            app.palette.style(Role::TextDim),
        );
        y += 2;
    }

    // Progress, or the countdown to the next broadcast — whichever the viewer needs next.
    if y < area.bottom() {
        if let (Some(TitleProgress { next, .. }), Some(label)) =
            (entry.progress, progress_label(entry))
        {
            let mut x = area.left();
            if entry.progress_of().is_some() {
                set_meter(buf, x, y, &app.palette, entry.watched_fraction(), 20, Role::Text);
                x += 22;
            }
            // Watched to the end, there is no next episode to name: play says Watch again.
            let said = if crate::app::watched_all(entry) {
                label
            } else {
                [label, t::title_up_next(i64::from(next))].join("   ")
            };
            buf.set_string(x, y, said, app.palette.style(Role::TextDim));
        } else {
            // The same line the preview gives, so the same fact reads the same on both.
            let broadcast = broadcast_line(entry);
            if !broadcast.is_empty() {
                line(
                    buf,
                    area.left(),
                    y,
                    truncate(&broadcast, area.width as usize),
                    app.palette.style(Role::State),
                );
            }
        }
    }

    // A next episode that aired and no source lists: said a line under the meter.
    if !status_lines.is_empty() {
        let y = y + 2;
        let room = area.bottom().saturating_sub(y) as usize;
        for (i, words) in status_lines.iter().take(room).enumerate() {
            let mut x = area.left() + if i == 0 { 0 } else { STATUS_INDENT };
            for (n, (word, role)) in words.iter().enumerate() {
                if n > 0 {
                    x += 1;
                }
                buf.set_string(x, y + i as u16, word, app.palette.style(*role));
                x += word.cells() as u16;
            }
        }
    }
}

/// How far the status's later lines sit in, so they start under its first word, past the dot.
const STATUS_INDENT: u16 = 2;

/// The status under the meter, worded and wrapped to `width`, at most `room` lines: each line
/// its words, each word in its role. The dot and the first sentence in the state colour, the
/// rest in the dim one; a week on the dot is hollow and the sentence plain text.
fn missing_lines(
    app: &App,
    status: &MissingStatus,
    width: usize,
    room: usize,
) -> Vec<Vec<(String, Role)>> {
    let words = |text: &str, role: Role| {
        text.split_whitespace().map(|w| (w.to_owned(), role)).collect::<Vec<_>>()
    };
    let mut said = match status {
        MissingStatus::Checking => {
            let mut said = vec![(glyph::STATE_WAITING.to_string(), Role::State)];
            said.extend(words(&t::episode_missing_checking(), Role::State));
            said
        }
        MissingStatus::Says { missing, answer } => {
            let long_ago = missing.phase == MissingPhase::LongAgo;
            let (dot, head_role) = if long_ago {
                (glyph::STATE_OFF, Role::Text)
            } else {
                (glyph::STATE_READY, Role::State)
            };
            let mut said = vec![(dot.to_string(), head_role)];
            said.extend(words(&missing_head(app, missing), head_role));
            said.extend(words(&missing_tail(missing, *answer), Role::TextDim));
            said
        }
    };
    // Wrapped by words, as the synopsis is, the later lines in by the dot's width.
    let mut lines: Vec<Vec<(String, Role)>> = Vec::new();
    let mut used = 0;
    for word in said.drain(..) {
        let cells = word.0.cells();
        let later = lines.len() > 1;
        let fits = width.saturating_sub(if later { STATUS_INDENT as usize } else { 0 });
        match lines.last_mut() {
            Some(line) if used + 1 + cells <= fits => {
                line.push(word);
                used += 1 + cells;
            }
            _ => {
                if lines.len() == room {
                    break;
                }
                used = cells;
                lines.push(vec![word]);
            }
        }
    }
    lines
}

/// The status's first sentence: which episode, and when it aired. With no airing time known,
/// the words the page had before it could say more.
fn missing_head(app: &App, missing: &MissingEpisode) -> String {
    let episode = i64::from(missing.episode);
    let Some(at) = missing.aired_at else { return t::episode_unsourced() };
    match aired_ago(at, app.clock.now_epoch()) {
        AiredAgo::Minutes { minutes } => t::episode_missing_aired(
            episode,
            &t::time_ago(&t::time_minutes(i64::from(minutes))),
        ),
        AiredAgo::Hours { hours } => {
            t::episode_missing_aired(episode, &t::time_ago(&t::time_hours(i64::from(hours))))
        }
        AiredAgo::OnDate => {
            let date = app.clock.local(at).map(|d| d.format("%-d %b").to_string());
            t::episode_missing_aired_on(episode, &date.unwrap_or_default())
        }
    }
}

/// The rest of the status, by where the episode stands and what the last check the person asked
/// for came to. Kaeri's own looking is named only while it looks; offline then, the line says
/// it looks once back.
fn missing_tail(missing: &MissingEpisode, answer: Option<CheckOutcome>) -> String {
    let looks_again = || t::episode_missing_looks_again(i64::from(missing.every_minutes));
    let joined = |first: String| [first, looks_again()].join(" ");
    match (missing.phase, answer) {
        (MissingPhase::LongAgo, _) => t::episode_missing_none(),
        (MissingPhase::Undated, _) => String::new(),
        (MissingPhase::After, Some(CheckOutcome::NoAnswer)) => t::episode_missing_no_answer(),
        (MissingPhase::After, _) => t::episode_missing_still(),
        (MissingPhase::Quiet, Some(CheckOutcome::Offline)) => t::episode_missing_offline(),
        (MissingPhase::Quiet, Some(CheckOutcome::Missing)) => {
            joined(t::episode_missing_still())
        }
        (MissingPhase::Quiet, Some(CheckOutcome::NoAnswer)) => {
            joined(t::episode_missing_no_answer())
        }
        (MissingPhase::Quiet, Some(CheckOutcome::Found) | None) => {
            joined(t::episode_missing_not_yet())
        }
    }
}

/// The count beside the meter, in the language in use.
fn progress_label(entry: &Entry) -> Option<String> {
    let done = i64::from(entry.progress?.done);
    Some(match entry.progress_of() {
        Some((total, true)) => t::progress_episodes(done, i64::from(total)),
        Some((aired, false)) => t::title_progress_aired(done, i64::from(aired)),
        None => t::title_progress_watched(done),
    })
}

/// What most recently aired and what is next: the line the preview gives, so the same fact reads
/// the same on both.
fn broadcast_line(entry: &Entry) -> String {
    super::lists::broadcast_text(entry)
}

/// The list-status picker.
pub(super) fn status_rows(app: &App) -> Vec<(String, String)> {
    let current = app.target();
    LibrarySegment::ALL
        .iter()
        .map(|segment| {
            let hint = match (segment, current.and_then(|e| e.progress)) {
                (LibrarySegment::Completed, Some(_)) => t::title_status_completed_hint(),
                _ => String::new(),
            };
            (hint, status_text(*segment))
        })
        .collect()
}

pub(super) fn rate_rows() -> Vec<(String, String)> {
    // Ten down to one, the way a rating reads. The words carry the judgement so the
    // numbers do not have to be remembered as a scale.
    [
        t::title_rating_10(),
        t::title_rating_9(),
        t::title_rating_8(),
        t::title_rating_7(),
        t::title_rating_6(),
        t::title_rating_5(),
        t::title_rating_4(),
        t::title_rating_3(),
        t::title_rating_2(),
        t::title_rating_1(),
    ]
    .into_iter()
    .enumerate()
    .map(|(i, word)| (word, format!("{}", 10 - i)))
    .collect()
}

/// The watch order: prequels, parent story, sequels — the relation in the key column,
/// so the list reads as a path through the franchise rather than a bare list of names.
/// The source picker's rows: what each choice is, and which one is in force.
pub(super) fn source_provider_rows(app: &App) -> Vec<(String, String)> {
    app.provider_choices()
        .into_iter()
        .map(|(id, label)| {
            // The marker is the whole point of the list — without it the overlay says what the
            // options are but not which one this title is already on.
            let in_force = id.as_deref() == app.provider_preference.as_deref();
            (if in_force { t::source_picker_chosen() } else { String::new() }, label)
        })
        .collect()
}

/// The release picker's rows, with the one pinned marked.
pub(super) fn source_release_rows(app: &App) -> Vec<(String, String)> {
    let pinned = app
        .source_options
        .iter()
        .find(|option| Some(&option.provider_id) == app.release_of.as_ref())
        .and_then(|option| option.pinned_release.clone());
    app.release_choices()
        .into_iter()
        .map(|(id, label)| {
            (if id == pinned { t::source_picker_chosen() } else { String::new() }, label)
        })
        .collect()
}

pub(super) fn watch_order_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.target() else {
        return Vec::new();
    };
    crate::app::watch_order(detail)
        .into_iter()
        .map(|step| {
            let mut label = step.title;
            if let Some(format) = &step.format {
                push_detail(&mut label, format);
            }
            (step.place, label)
        })
        .collect()
}

pub(super) fn recommendation_rows(app: &App) -> Vec<(String, String)> {
    let Some(detail) = app.target() else {
        return Vec::new();
    };
    detail
        .recommended
        .iter()
        .map(|rec| {
            let mut label = rec.title.clone();
            if let Some(format) = &rec.format {
                push_detail(&mut label, format);
            }
            (String::new(), label)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_model::catalog::{ContentList, Entry};

    use crate::{
        layout,
        screens::{
            fixtures::{app_with, entry, text_of},
            render_to_buffer,
        },
    };

    #[test]
    fn the_title_screen_leads_with_the_key_visual() {
        // The hero is the artwork, not a headline — that is the thesis of the design.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Hoshikuzu")]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let buf = render_to_buffer(&mut app, 120, 30);
        let text = text_of(&buf);
        assert!(text.contains('░'), "banner plate missing");

        // The title is bold, and no longer letterspaced anywhere — measured on a real terminal,
        // tracking read as tiring rather than refined, and it was worst on digits.
        assert!(text.contains("Hoshikuzu"), "title missing");
        assert!(!text.contains("HOSHIKUZU"), "the hero title must not be letterspaced");

        // Metadata is still distinguished, by case and the dim role rather than by width.
        assert!(text.contains("28 episodes"), "metadata missing");

        // The plate occupies the top of the stage, above the title block.
        let plate_row = text.lines().position(|l| l.contains('░')).unwrap();
        let title_row = text.lines().position(|l| l.contains("Hoshikuzu")).unwrap();
        assert!(plate_row < title_row, "artwork must lead");
    }

    #[test]
    fn a_short_terminal_keeps_the_title_page_inside_the_stage() {
        // At ten rows the banner leaves the stage a few lines, and the metadata ran on into
        // the status rule and line under it.
        let mut app = app_with(ContentList::Entries(vec![entry(1, "Invented Show")]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let buf = render_to_buffer(&mut app, 80, 10);
        let stage = layout::compute(Rect::new(0, 0, 80, 10), app.nav.rail_width()).stage;
        let below: Vec<String> =
            text_of(&buf).lines().skip(stage.bottom() as usize).map(str::to_owned).collect();
        // The status line overwrites the front of a stray line and leaves its tail, the year.
        assert!(below.iter().all(|l| !l.contains("2023")), "{below:?}");
    }

    #[test]
    fn availability_badges_appear_when_a_title_has_them() {
        let mut app = app_with(ContentList::Entries(vec![Entry {
            available_on: vec!["Hikari Library".into(), "Nami TV".into()],
            ..entry(1, "Hoshikuzu")
        }]));
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let text = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(text.contains("Hikari Library"));
        assert!(text.contains("Nami TV"));
    }

    #[test]
    fn genres_read_the_same_in_the_preview_and_on_the_title() {
        let e =
            Entry { genres: vec!["Slice of Life".into(), "Sci-Fi".into()], ..entry(1, "Kite") };
        let mut app = app_with(ContentList::Entries(vec![e]));
        let preview = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(preview.contains("Slice of Life  ·  Sci-Fi"), "{preview}");
        assert!(!preview.contains("SLICE OF LIFE"), "{preview}");
        app.nav.focus_stage();
        app.handle(crate::keymap::Action::Open, 20);
        let title = text_of(&render_to_buffer(&mut app, 120, 34));
        assert!(title.contains("Slice of Life  ·  Sci-Fi"), "{title}");
    }
}

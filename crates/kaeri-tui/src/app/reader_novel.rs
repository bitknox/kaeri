//! A light novel in the reader: the engine's text blocks laid out in a column of the text width,
//! turned a screen at a time or scrolled a line at a time.
//!
//! The place is the engine's [`NovelPos`] of the first line on screen. A resize, a new width or
//! new spacing lays the text out again and puts the line that holds the place at the top, so
//! the sentence read last stays where it was. Screens are the pages the engine hears of: each
//! document of the book is its own run of screens, counted from the line at the top.

use std::time::{Duration, Instant};

use kaeri_core::media::Medium;
use kaeri_i18n::t;
use kaeri_model::{
    command::Command,
    reader::{
        NovelOpenView, NovelPos, ReaderAction, ReaderCommand, ReaderSettingKey, ReaderValue,
        SettingScope, TurnHow, novel_minutes_left,
    },
};
use ratatui::layout::Rect;

use super::{App, ReaderScreen, stage_of};
use crate::{
    keymap::{Action, said},
    novel::{Shape, TextLayout, lay_out},
};

/// The text width before the setting comes, in columns: about 66 characters a line reads well.
pub const TEXT_WIDTH: u16 = 66;
/// The narrowest text width `[` goes to.
pub const TEXT_WIDTH_MIN: u16 = 40;
/// `[` and `]` change the width by this many columns.
pub const TEXT_WIDTH_STEP: u16 = 4;
/// The columns the text keeps from each side of a window narrower than the text width, and
/// the room `]` leaves.
pub const TEXT_MARGIN: u16 = 2;
/// Line spacing from this amount up reads as Open, a blank line between paragraphs.
pub const OPEN_SPACING: f64 = 1.5;
/// What Tight and Open set the line spacing to. The engine starts a terminal on Open.
pub const TIGHT_AMOUNT: f64 = 1.4;
pub const OPEN_AMOUNT: f64 = 1.8;
/// A scrolled novel says where it settled this long after it stopped.
const REPORT_AFTER: Duration = Duration::from_millis(500);

/// A light novel open in the reader.
#[derive(Debug, Clone)]
pub struct NovelReading {
    pub open: NovelOpenView,
    /// The text at the shape it was last laid out for.
    pub layout: TextLayout,
    /// The first line on screen as the person left it: kept through a resize and a new width.
    pub place: NovelPos,
    /// The line at the top of the screen, in `layout`.
    pub top: usize,
    /// The page between chapters is up.
    pub end: bool,
    /// When a scrolled novel says where it settled.
    pub report: Option<Instant>,
    /// The place and screens last said to the engine.
    said: Option<(NovelPos, u32, u32)>,
    /// The line at the top follows `place`: false until the text is laid out after an open
    /// or a jump the engine asked for.
    placed: bool,
}

/// Where the text goes in a window: the column and the rows of the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NovelFrame {
    pub shape: Shape,
    /// The text column on the window.
    pub column: Rect,
}

/// What the status line says of a novel: the part, how far through it and the time left.
#[derive(Debug, Clone, PartialEq)]
pub struct NovelProgress {
    pub part: String,
    /// The part's title, when it has one beside its name.
    pub title: Option<String>,
    pub fraction: f64,
    pub minutes_left: Option<u32>,
}

impl NovelReading {
    /// The first place of the book's text: its first line.
    fn first_place(open: &NovelOpenView) -> NovelPos {
        open.text
            .iter()
            .find(|d| !d.blocks.is_empty())
            .and_then(|d| d.blocks.first().map(|b| NovelPos { spine: d.spine, ..b.at.clone() }))
            .unwrap_or(NovelPos { spine: 0, path: Vec::new(), offset: 0 })
    }

    /// The screen the line at the top is on in its document, and the document's screens.
    pub fn screens(layout: &TextLayout, top: usize, rows: usize) -> (u32, u32) {
        let rows = rows.max(1);
        let range = layout.docs.get(layout.doc_of(top)).cloned().unwrap_or(0..0);
        let before = top.saturating_sub(range.start);
        let after = range.end.saturating_sub(top).max(1);
        let page = before.div_ceil(rows);
        (page as u32, (page + after.div_ceil(rows)) as u32)
    }
}

impl App {
    /// The open chapter is a light novel.
    pub fn reads_novel(&self) -> bool {
        self.reader_here()
            .is_some_and(|r| r.novel.is_some() || r.medium() == Some(Medium::Novel))
    }

    /// The text column and stage for a window `w` by `h`, from the text width setting and the
    /// spacing settings.
    pub fn novel_frame(&self, (w, h): (u16, u16)) -> Option<NovelFrame> {
        let reader = self.reader_here()?;
        let stage = stage_of(Rect::new(0, 0, w, h), reader.bare);
        let wanted = reader
            .amount(ReaderSettingKey::TextWidth)
            .map_or(TEXT_WIDTH, |v| v.round().clamp(1.0, f64::from(u16::MAX)) as u16);
        let width = wanted.min(stage.width.saturating_sub(2 * TEXT_MARGIN)).max(1);
        let open =
            reader.amount(ReaderSettingKey::LineSpacing).is_none_or(|a| a >= OPEN_SPACING);
        let justify = reader.toggle(ReaderSettingKey::Justify).unwrap_or(false);
        let x = stage.x + (stage.width.saturating_sub(width)) / 2;
        Some(NovelFrame {
            shape: Shape { width, open, justify },
            column: Rect { x, width, ..stage },
        })
    }

    /// The novel's text laid out for the window: the one kept when it fits, else a new one, and
    /// the line at the top in it.
    pub fn novel_text(
        &self,
        frame: NovelFrame,
    ) -> Option<(std::borrow::Cow<'_, TextLayout>, usize)> {
        let novel = self.reader_here()?.novel.as_ref()?;
        if novel.layout.shape == frame.shape {
            return Some((std::borrow::Cow::Borrowed(&novel.layout), novel.top));
        }
        let layout = lay_out(&novel.open.text, frame.shape);
        let top = layout.line_of(&novel.open.text, &novel.place).unwrap_or(0);
        Some((std::borrow::Cow::Owned(layout), top))
    }

    /// The engine opened a light novel: its text, at the place reading stopped.
    pub(super) fn novel_open(&mut self, open: NovelOpenView) {
        let Some(reader) = self.reader.as_mut() else { return };
        if open.session != reader.session {
            return;
        }
        let place = open.restore.clone().unwrap_or_else(|| NovelReading::first_place(&open));
        reader.opening = None;
        reader.wait = None;
        reader.novel = Some(NovelReading {
            open,
            layout: TextLayout {
                shape: Shape { width: 0, open: false, justify: false },
                lines: Vec::new(),
                docs: Vec::new(),
            },
            place,
            top: 0,
            end: false,
            report: None,
            said: None,
            placed: false,
        });
        self.novel_fit();
    }

    /// Lay the text out again for the window and the settings when they changed, with the
    /// place's line at the top, and tell the engine when that changed the screens.
    pub(super) fn novel_fit(&mut self) {
        let Some(window) = self.window else { return };
        let Some(frame) = self.novel_frame(window) else { return };
        let rows = usize::from(frame.column.height);
        let Some(reader) = self.reader_mut() else { return };
        let Some(novel) = reader.novel.as_mut() else { return };
        // Laid out again, the line that holds the place goes to the top. Otherwise the top
        // stays where the keys put it, even on a blank line the place can't name.
        if novel.layout.shape != frame.shape || novel.layout.docs.len() != novel.open.text.len()
        {
            novel.layout = lay_out(&novel.open.text, frame.shape);
            novel.placed = false;
        }
        if !novel.placed {
            novel.top = novel.layout.line_of(&novel.open.text, &novel.place).unwrap_or(0);
            novel.placed = true;
        }
        // A scroll still moving says where it settled on the tick, not here.
        if !novel.end && novel.report.is_none() {
            let (page, pages) = NovelReading::screens(&novel.layout, novel.top, rows);
            if novel
                .said
                .as_ref()
                .is_none_or(|(pos, p, n)| *pos != novel.place || *p != page || *n != pages)
                && let Some(task) = self.novel_at(TurnHow::Placed)
            {
                self.commands.push(task);
            }
        }
    }

    /// The stage's rows for the text, in the window the last frame was drawn in.
    fn novel_rows(&self) -> usize {
        self.window
            .and_then(|w| self.novel_frame(w))
            .map_or(1, |f| usize::from(f.column.height).max(1))
    }

    /// Tell the engine the place and screens of the text in view.
    fn novel_at(&mut self, how: TurnHow) -> Option<Command> {
        let rows = self.novel_rows();
        let reader = self.reader_mut()?;
        let session = reader.session;
        let chapter = reader.view.as_ref().map(|v| v.chapter.id.clone())?;
        let novel = reader.novel.as_mut()?;
        let (page, pages) = NovelReading::screens(&novel.layout, novel.top, rows);
        novel.said = Some((novel.place.clone(), page, pages));
        novel.report = None;
        Some(Command::Reader(ReaderCommand::NovelAt {
            session,
            chapter,
            pos: novel.place.clone(),
            page,
            pages,
            how,
        }))
    }

    /// Put line `top` at the top and keep its place: said at once, or once a scroll settles.
    fn novel_move(&mut self, top: usize, how: TurnHow) -> Option<Command> {
        let now = self.clock.now_instant();
        let reader = self.reader_mut()?;
        let novel = reader.novel.as_mut()?;
        novel.end = false;
        novel.top = top.min(novel.layout.lines.len().saturating_sub(1));
        if let Some(place) = novel.layout.pos_of(&novel.open.text, novel.top) {
            novel.place = place;
        }
        if how == TurnHow::Scroll {
            novel.report = Some(now + REPORT_AFTER);
            return None;
        }
        self.novel_at(how)
    }

    /// The page between chapters: the engine hears the screen after the last, as the apps say
    /// it, and counts the chapter read.
    fn novel_end(&mut self) -> Option<Command> {
        let rows = self.novel_rows();
        let reader = self.reader_mut()?;
        let session = reader.session;
        let chapter = reader.view.as_ref().map(|v| v.chapter.id.clone())?;
        let novel = reader.novel.as_mut()?;
        novel.end = true;
        novel.report = None;
        let (_, pages) = NovelReading::screens(&novel.layout, novel.top, rows);
        Some(Command::Reader(ReaderCommand::NovelAt {
            session,
            chapter,
            pos: novel.place.clone(),
            page: pages,
            pages,
            how: TurnHow::Turn,
        }))
    }

    /// The reader's keys in a light novel. `None` leaves the key to the reader.
    pub(super) fn novel_key(
        &mut self,
        screen: ReaderScreen,
        action: Action,
    ) -> Option<Option<Command>> {
        use Action as A;
        let novel = self.reader_here()?.novel.as_ref()?;
        let scrolled = self.reader_here()?.novel_scrolled();
        if screen == ReaderScreen::End {
            return match action {
                A::ReaderPrevious | A::ReaderLeft | A::ReaderScrollUp => {
                    let top = novel.top;
                    Some(self.novel_move(top, TurnHow::Key))
                }
                A::ReaderNext | A::ReaderRight => {
                    let next = self.reader_here()?.end.next.clone();
                    Some(next.and_then(|n| self.open_chapter(n.id)))
                }
                _ => None,
            };
        }
        if screen != ReaderScreen::Pages {
            return None;
        }
        let rows = self.novel_rows();
        Some(match action {
            A::ReaderNext | A::ReaderRight => self.novel_forward(rows, scrolled),
            A::ReaderPrevious | A::ReaderLeft => self.novel_back(rows, scrolled),
            A::ReaderScrollDown if scrolled => self.novel_scroll(1, rows),
            A::ReaderScrollUp if scrolled => self.novel_scroll(-1, rows),
            A::ReaderScrollDown => self.novel_forward(rows, false),
            A::ReaderScrollUp => self.novel_back(rows, false),
            A::ReaderFirst => self.novel_first(),
            A::ReaderLast => self.novel_last(rows, scrolled),
            A::ReaderNarrower => self.novel_width(false),
            A::ReaderWider => self.novel_width(true),
            A::ReaderMode => self.novel_mode(),
            // Nothing to fit, zoom or put side by side in text.
            A::ReaderFit
            | A::ReaderZoom
            | A::ReaderZoomIn
            | A::ReaderZoomOut
            | A::ReaderPages => None,
            _ => return None,
        })
    }

    /// A screen on: the next screen of the document, the next document, or the page between
    /// chapters after the last. Scrolled, most of a screen.
    fn novel_forward(&mut self, rows: usize, scrolled: bool) -> Option<Command> {
        let novel = self.reader_here()?.novel.as_ref()?;
        let layout = &novel.layout;
        if scrolled {
            let max = layout.lines.len().saturating_sub(rows);
            if novel.top >= max {
                return self.novel_end();
            }
            let by = (rows * 9 / 10).max(1);
            let top = (novel.top + by).min(max);
            return self.novel_move(top, TurnHow::Scroll);
        }
        let doc = layout.doc_of(novel.top);
        let range = layout.docs.get(doc)?.clone();
        if novel.top + rows < range.end {
            let top = novel.top + rows;
            return self.novel_move(top, TurnHow::Key);
        }
        match layout.docs.iter().skip(doc + 1).find(|r| !r.is_empty()) {
            Some(next) => {
                let top = next.start;
                self.novel_move(top, TurnHow::Key)
            }
            None => self.novel_end(),
        }
    }

    /// A screen back: the screen before in the document, or the last screen of the document
    /// before. Scrolled, most of a screen.
    fn novel_back(&mut self, rows: usize, scrolled: bool) -> Option<Command> {
        let novel = self.reader_here()?.novel.as_ref()?;
        let layout = &novel.layout;
        if scrolled {
            if novel.top == 0 {
                return None;
            }
            let top = novel.top.saturating_sub((rows * 9 / 10).max(1));
            return self.novel_move(top, TurnHow::Scroll);
        }
        let doc = layout.doc_of(novel.top);
        let range = layout.docs.get(doc)?.clone();
        if novel.top > range.start {
            let top = novel.top.saturating_sub(rows).max(range.start);
            return self.novel_move(top, TurnHow::Key);
        }
        let before = layout.docs[..doc].iter().rev().find(|r| !r.is_empty())?.clone();
        let top = before.start + (before.len() - 1) / rows * rows;
        self.novel_move(top, TurnHow::Key)
    }

    /// `j` and `k` in a scrolled novel: a line.
    fn novel_scroll(&mut self, by: i64, rows: usize) -> Option<Command> {
        let novel = self.reader_here()?.novel.as_ref()?;
        let max = novel.layout.lines.len().saturating_sub(rows) as i64;
        let top = (novel.top as i64 + by).clamp(0, max.max(0)) as usize;
        if top == novel.top {
            return None;
        }
        self.novel_move(top, TurnHow::Scroll)
    }

    /// `g`: the start over the resumed notice offers, else the first line of the book.
    fn novel_first(&mut self) -> Option<Command> {
        let now = self.clock.now_instant();
        let reader = self.reader_mut()?;
        let offered = reader.notice.as_ref().is_some_and(|n| {
            n.action == Some(ReaderAction::StartOver) && n.until.is_none_or(|u| u > now)
        });
        if offered {
            reader.notice = None;
            return Some(Command::Reader(ReaderCommand::StartOver { session: reader.session }));
        }
        let first = reader.novel.as_ref()?.layout.docs.iter().find(|r| !r.is_empty())?.start;
        self.novel_move(first, TurnHow::Jump)
    }

    /// `G`: the last screen of the book.
    fn novel_last(&mut self, rows: usize, scrolled: bool) -> Option<Command> {
        let layout = &self.reader_here()?.novel.as_ref()?.layout;
        let top = if scrolled {
            layout.lines.len().saturating_sub(rows)
        } else {
            let last = layout.docs.iter().rev().find(|r| !r.is_empty())?;
            last.start + (last.len() - 1) / rows * rows
        };
        self.novel_move(top, TurnHow::Jump)
    }

    /// `[` and `]`: the text 4 columns narrower or wider, from 40 up to the window less 4,
    /// kept on this computer.
    pub(super) fn novel_width(&mut self, wider: bool) -> Option<Command> {
        let (w, _) = self.window?;
        let frame = self.novel_frame(self.window?)?;
        let most = w.saturating_sub(2 * TEXT_MARGIN);
        if most < TEXT_WIDTH_MIN {
            return None;
        }
        let now = frame.shape.width;
        let width = if wider {
            now.saturating_add(TEXT_WIDTH_STEP).min(most)
        } else {
            now.saturating_sub(TEXT_WIDTH_STEP).max(TEXT_WIDTH_MIN)
        };
        if width == now {
            return None;
        }
        let value = ReaderValue::Amount { value: f64::from(width) };
        self.set_reader_setting(ReaderSettingKey::TextWidth, SettingScope::Device, value)
    }

    /// `m` in a novel: paged or scrolled, the series' own choice, as the apps keep it.
    fn novel_mode(&mut self) -> Option<Command> {
        let scrolled = self.reader_here()?.novel_scrolled();
        let name = if scrolled { "paged" } else { "scrolled" }; // i18n-skip: setting values
        let value = ReaderValue::Choice { name: name.to_owned() };
        self.set_reader_setting(ReaderSettingKey::Mode, SettingScope::Series, value)
    }

    /// A setting changed in the reader: set here at once so the screen follows, then sent to be
    /// kept. The work goes with every change, so the settings that come back still carry the
    /// series' own choices. The text is laid out again where it changes it, and the surface
    /// sent again where it changes the stage.
    pub(super) fn set_reader_setting(
        &mut self,
        key: ReaderSettingKey,
        scope: SettingScope,
        value: ReaderValue,
    ) -> Option<Command> {
        let reader = self.reader_mut()?;
        if reader.session == 0 {
            return None;
        }
        reader.set_value(key, value.clone(), scope);
        let work = Some(reader.work.clone());
        let (session, medium) = (reader.session, reader.medium().unwrap_or(Medium::Manga));
        self.send_surface();
        self.novel_fit();
        Some(Command::Reader(ReaderCommand::SetSetting {
            session: Some(session),
            medium,
            work,
            key,
            scope,
            value,
        }))
    }

    /// On the idle tick: say where a scrolled novel settled, once it has been still a moment.
    pub(super) fn tick_novel(&mut self) {
        let now = self.clock.now_instant();
        let due = self
            .reader_here()
            .and_then(|r| r.novel.as_ref())
            .and_then(|n| n.report)
            .is_some_and(|due| due <= now);
        if due && let Some(task) = self.novel_at(TurnHow::Scroll) {
            self.commands.push(task);
        }
    }

    /// The part in view, how far into it the screen starts and the minutes left at the
    /// person's pace.
    pub fn novel_progress(&self) -> Option<NovelProgress> {
        let reader = self.reader_here()?;
        let novel = reader.novel.as_ref()?;
        let frame = self.novel_frame(self.window.unwrap_or((80, 24)))?;
        let (layout, top) = self.novel_text(frame)?;
        let rows = usize::from(frame.column.height).max(1);
        let doc = layout.doc_of(top);
        let range = layout.docs.get(doc).cloned().unwrap_or(0..0);
        let spine = novel.open.text.get(doc).map_or(0, |d| d.spine);
        let fraction = if novel.end || range.is_empty() {
            1.0
        } else {
            (top.saturating_sub(range.start)) as f64 / range.len() as f64
        };
        let (page, pages) = NovelReading::screens(&layout, top, rows);
        let words = novel.open.spine.get(spine as usize).map_or(0, |s| s.words);
        let minutes_left = (!novel.end)
            .then(|| novel_minutes_left(words, page, pages, novel.open.words_per_minute))
            .flatten();
        // The contents' entry for the document in view, or the chapter's own name.
        let toc = &novel.open.toc;
        let top_entry = toc.iter().rev().find(|e| e.spine <= spine && e.depth == 0);
        let deepest = toc.iter().rev().find(|e| e.spine <= spine);
        let (part, title) = match top_entry.or(deepest) {
            Some(entry) => {
                let title = deepest.filter(|d| d.label != entry.label).map(|d| d.label.clone());
                (entry.label.clone(), title)
            }
            None => {
                let row = reader.chapter()?;
                (row.label.clone(), row.title.clone())
            }
        };
        Some(NovelProgress { part, title, fraction, minutes_left })
    }

    /// The status line's words for a novel: "Part 3 · 34% · 6 min left", the time left only
    /// where there is room.
    pub fn novel_status_words(&self, roomy: bool) -> Option<String> {
        let progress = self.novel_progress()?;
        let percent = (progress.fraction * 100.0).floor() as i64;
        let mut words = format!("{} · {}", progress.part, t::tui_reader_novel_through(percent)); // i18n-skip: layout
        if roomy && let Some(minutes) = progress.minutes_left {
            words.push_str(&format!(" · {}", t::time_minutes_left(i64::from(minutes)))); // i18n-skip: layout
        }
        Some(words)
    }
}

impl App {
    /// A novel's status hints, as the board writes them: the turn keys or `j k scroll`, the
    /// width keys and settings where there is room, and the keys.
    pub(super) fn novel_hints(&self, width: usize) -> Vec<(String, &'static str)> {
        let Some(reader) = self.reader_here() else { return Vec::new() };
        let one = |action: Action| self.reader_key_name(action);
        let pair = |a: Action, b: Action| -> Option<String> {
            Some(format!("{} {}", self.reader_letter(a)?, self.reader_letter(b)?)) // i18n-skip: keys
        };
        let mut wanted: Vec<(Option<String>, &'static str)> = if reader.novel_scrolled() {
            vec![(
                pair(Action::ReaderScrollDown, Action::ReaderScrollUp),
                said(t::tui_reader_hint_scroll()),
            )]
        } else {
            let mut turns = vec![(one(Action::ReaderRight), said(t::tui_reader_hint_next()))];
            if width >= 72 {
                turns.push((one(Action::ReaderLeft), said(t::tui_reader_hint_back())));
            }
            turns
        };
        if width >= 72 {
            wanted.push((
                pair(Action::ReaderNarrower, Action::ReaderWider),
                said(t::tui_reader_hint_width()),
            ));
        }
        if width >= 100 {
            wanted.push((one(Action::ReaderSettings), said(t::tui_reader_hint_settings())));
        }
        wanted.push((one(Action::Help), said(t::keys_hint_keys())));
        wanted.into_iter().filter_map(|(key, label)| Some((key?, label))).collect()
    }

    /// The settings overlay's hints: change, move and close, and Use default on a series' own
    /// choice.
    pub(super) fn reader_settings_hints(&self) -> Vec<(String, &'static str)> {
        let items = self.reader_setting_items();
        let mut hints = vec![
            ("‹ ›".to_owned(), said(t::tui_reader_hint_change())), // i18n-skip: keys
            ("j k".to_owned(), said(t::tui_reader_hint_move())),   // i18n-skip: keys
        ];
        if items.get(self.cursors.overlay.selected).is_some_and(|i| i.overridden) {
            hints.push(("d".to_owned(), said(t::value_use_default()))); // i18n-skip: a key
        }
        hints.push(("esc".to_owned(), said(t::keys_hint_close()))); // i18n-skip: a key
        hints
    }
}

impl NovelReading {
    /// Go to `place`, as the engine asked: the top follows it once the text is laid out.
    pub fn go_to(&mut self, place: NovelPos) {
        self.place = place;
        self.end = false;
        self.placed = false;
    }
}

impl super::Reader {
    /// A novel scrolls rather than turning screens, as its Mode says.
    pub fn novel_scrolled(&self) -> bool {
        match self.choice(ReaderSettingKey::Mode) {
            Some(mode) => mode == "scrolled", // i18n-skip: a setting's value
            None => self.mode() == kaeri_model::reader::ReaderMode::Scrolled,
        }
    }
}

#[cfg(test)]
pub(in crate::app) mod tests {
    use std::time::Duration;

    use crossterm::event::KeyCode;
    use kaeri_core::books::{ChapterId, WorkId};
    use kaeri_model::{
        eink::NovelSupport,
        event::Event,
        reader::{
            NovelSpineItem, NovelTextDoc, NovelTocEntry, ReadDirection, ReaderEvent,
            ReaderMode, ReaderSettingRow, ReaderSettingsView, TextBlock, TextBlockKind,
            TextRun, WritingMode,
        },
    };

    use super::super::reader::tests::{SESSION, key, view};
    use super::*;
    use crate::{
        app::{FrameReport, fixtures::*},
        nav::Overlay,
    };

    fn block(kind: TextBlockKind, spine: u32, path: u32, text: &str) -> TextBlock {
        TextBlock {
            kind,
            at: NovelPos { spine, path: vec![path], offset: 0 },
            runs: vec![TextRun {
                text: text.to_owned(),
                offset: 0,
                path: vec![path],
                emphasis: false,
                strong: false,
                ruby: false,
            }],
        }
    }

    /// A made-up paragraph of about four lines at 66 columns.
    fn paragraph(n: u32) -> String {
        format!(
            "Lamp {n} hung by the door. The keeper of the street walked past it every evening, \
             counting the windows that still answered, and wrote the number in a small book \
             she kept for no one but herself and the rain."
        )
    }

    /// Two documents: a heading and twenty paragraphs, then ten more.
    pub(in crate::app) fn text() -> Vec<NovelTextDoc> {
        let mut first = vec![block(TextBlockKind::Heading { level: 1 }, 0, 0, "The Lamp Room")];
        first.extend((1..=20).map(|n| block(TextBlockKind::Paragraph, 0, n, &paragraph(n))));
        let second =
            (1..=10).map(|n| block(TextBlockKind::Paragraph, 1, n, &paragraph(20 + n)));
        vec![
            NovelTextDoc { spine: 0, blocks: first },
            NovelTextDoc { spine: 1, blocks: second.collect() },
        ]
    }

    pub(in crate::app) fn setting(
        key: ReaderSettingKey,
        value: ReaderValue,
    ) -> ReaderSettingRow {
        let scope = if key.syncs() { SettingScope::Medium } else { SettingScope::Device };
        ReaderSettingRow {
            key,
            value,
            scope,
            detected: false,
            overridden: false,
            choices: Vec::new(),
        }
    }

    fn settings(mode: &str) -> ReaderSettingsView {
        ReaderSettingsView {
            medium: Medium::Novel,
            work: Some(WorkId::new("a:1")),
            rows: vec![
                setting(ReaderSettingKey::Mode, ReaderValue::Choice { name: mode.to_owned() }),
                setting(ReaderSettingKey::TextWidth, ReaderValue::Amount { value: 66.0 }),
                setting(ReaderSettingKey::LineSpacing, ReaderValue::Amount { value: 1.4 }),
                setting(ReaderSettingKey::Justify, ReaderValue::Toggle { on: false }),
            ],
            tap_zones: None,
        }
    }

    pub(in crate::app) fn open_view(restore: Option<NovelPos>) -> NovelOpenView {
        let spine = |n: u32| NovelSpineItem {
            url: format!("kaeri-book://s7/text/{n}.xhtml"),
            linear: true,
            words: 2000,
        };
        NovelOpenView {
            session: SESSION,
            chapter: ChapterId("c14".into()),
            entry: "kaeri-book://s7/text/0.xhtml".into(),
            writing_mode: WritingMode::Horizontal,
            progression: ReadDirection::LeftToRight,
            restore,
            fixed_layout: false,
            support: NovelSupport::Full,
            spine: vec![spine(0), spine(1)],
            toc: vec![
                NovelTocEntry {
                    label: "Part 3".into(),
                    url: String::new(),
                    spine: 0,
                    depth: 0,
                },
                NovelTocEntry {
                    label: "The Lamp Room".into(),
                    url: String::new(),
                    spine: 0,
                    depth: 1,
                },
            ],
            style: String::new(),
            count_key: String::new(),
            counted: Vec::new(),
            render_dir: None,
            words_per_minute: 230,
            text: text(),
        }
    }

    /// A light novel open in a window `w` by `h`, paged or scrolled, at `restore`.
    pub(in crate::app) fn novel(w: u16, h: u16, mode: &str, restore: Option<NovelPos>) -> App {
        kaeri_i18n::set_language("en");
        let engine = crate::image::ImageEngine::halfblocks();
        let mut a =
            App::with_images(SettingsView::default(), Palette::dark(), Keymap::new(), engine);
        a.clock = crate::clock::Clock::fixed(1_793_000_000, crate::clock::Zone::utc());
        a.took_frame(FrameReport { window: Some((w, h)), ..FrameReport::default() });
        a.open_reader(WorkId::new("a:1"), Some(ChapterId("c14".into())));
        let mut opened = view(ReadDirection::LeftToRight);
        opened.medium = Medium::Novel;
        opened.mode = if mode == "scrolled" { ReaderMode::Scrolled } else { ReaderMode::Paged };
        opened.settings = Some(settings(mode));
        a.apply(Event::Reader(ReaderEvent::Opened(Box::new(opened))));
        a.apply(Event::Reader(ReaderEvent::NovelOpen(open_view(restore))));
        a
    }

    fn press(a: &mut App, code: KeyCode) -> Option<Command> {
        a.press(key(code), 20)
    }

    fn reading(a: &App) -> &NovelReading {
        a.reader_here().and_then(|r| r.novel.as_ref()).expect("a novel open")
    }

    /// The NovelAt reports among `commands`, as (pos, page, pages, how).
    fn reports(commands: &[Command]) -> Vec<(NovelPos, u32, u32, TurnHow)> {
        commands
            .iter()
            .filter_map(|c| match c {
                Command::Reader(ReaderCommand::NovelAt { pos, page, pages, how, .. }) => {
                    Some((pos.clone(), *page, *pages, *how))
                }
                _ => None,
            })
            .collect()
    }

    fn at(a: &mut App, code: KeyCode) -> Vec<(NovelPos, u32, u32, TurnHow)> {
        let task = press(a, code);
        let mut commands = a.take_commands();
        commands.extend(task);
        reports(&commands)
    }

    fn pos(spine: u32, path: u32) -> NovelPos {
        NovelPos { spine, path: vec![path], offset: 0 }
    }

    #[test]
    fn a_novel_opens_at_the_kept_place_and_says_so() {
        let mut a = novel(120, 34, "paged", Some(pos(0, 12)));
        assert_eq!(a.reader_screen(), Some(ReaderScreen::Pages));
        let said = reports(&a.take_commands());
        assert_eq!(said.len(), 1, "{said:?}");
        let (place, page, pages, how) = said[0].clone();
        assert_eq!(place, pos(0, 12));
        assert_eq!(how, TurnHow::Placed);
        assert!(page > 0 && page < pages, "{page} of {pages}");
        let novel = reading(&a);
        assert_eq!(novel.layout.pos_of(&novel.open.text, novel.top), Some(pos(0, 12)));
    }

    #[test]
    fn paged_keys_turn_a_screen_and_space_always_goes_on() {
        let mut a = novel(120, 34, "paged", None);
        a.take_commands();
        let rows = 30;
        let said = at(&mut a, KeyCode::Right);
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!((said[0].1, said[0].3), (1, TurnHow::Key));
        assert_eq!(reading(&a).top, rows);
        at(&mut a, KeyCode::Char(' '));
        assert_eq!(reading(&a).top, 2 * rows);
        let said = at(&mut a, KeyCode::Left);
        assert_eq!((said[0].1, said[0].3), (1, TurnHow::Key));
        assert_eq!(reading(&a).top, rows);
        at(&mut a, KeyCode::Left);
        assert!(at(&mut a, KeyCode::Left).is_empty(), "nothing before the first screen");
        assert_eq!(reading(&a).top, 0);
    }

    #[test]
    fn the_last_screen_of_a_document_turns_to_the_next_then_to_the_end_page() {
        let mut a = novel(120, 34, "paged", None);
        a.take_commands();
        let second = reading(&a).layout.docs[1].clone();
        // On through the first document into the second, which starts a screen of its own.
        while reading(&a).top < second.start {
            at(&mut a, KeyCode::Right);
        }
        assert_eq!(reading(&a).top, second.start);
        let said = at(&mut a, KeyCode::Char('G'));
        let last = said.last().cloned().expect("G says where it went");
        assert_eq!(last.3, TurnHow::Jump);
        assert_eq!(last.1 + 1, last.2, "the last screen");
        let said = at(&mut a, KeyCode::Right);
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].1, said[0].2, "the end page is the screen after the last");
        assert_eq!(said[0].3, TurnHow::Turn);
        assert_eq!(a.reader_screen(), Some(ReaderScreen::End));
        at(&mut a, KeyCode::Left);
        assert_eq!(a.reader_screen(), Some(ReaderScreen::Pages));
        assert!(reading(&a).top >= second.start);
    }

    #[test]
    fn scrolled_j_and_k_move_a_line_and_say_where_they_settled() {
        let mut a = novel(120, 34, "scrolled", None);
        a.take_commands();
        assert!(at(&mut a, KeyCode::Char('j')).is_empty(), "not while it moves");
        at(&mut a, KeyCode::Char('j'));
        at(&mut a, KeyCode::Char('j'));
        at(&mut a, KeyCode::Char('k'));
        assert_eq!(reading(&a).top, 2);
        a.took_frame(FrameReport { window: Some((120, 34)), ..FrameReport::default() });
        a.tick_toasts();
        assert!(reports(&a.take_commands()).is_empty(), "nor on the next frame");
        a.clock.advance(Duration::from_millis(500));
        a.tick_toasts();
        let said = reports(&a.take_commands());
        assert_eq!(said.len(), 1, "{said:?}");
        assert_eq!(said[0].3, TurnHow::Scroll);
        let novel = reading(&a);
        assert_eq!(Some(said[0].0.clone()), novel.layout.pos_of(&novel.open.text, 2));
        // Space runs on most of a screen, across the documents.
        at(&mut a, KeyCode::Char(' '));
        assert_eq!(reading(&a).top, 2 + 27);
    }

    #[test]
    fn m_switches_a_novel_between_paged_and_scrolled_for_the_series() {
        let mut a = novel(120, 34, "paged", None);
        a.take_commands();
        let task = press(&mut a, KeyCode::Char('m'));
        let Some(Command::Reader(ReaderCommand::SetSetting {
            key, scope, value, work, ..
        })) = task
        else {
            panic!("{task:?}")
        };
        assert_eq!((key, scope), (ReaderSettingKey::Mode, SettingScope::Series));
        assert_eq!(value, ReaderValue::Choice { name: "scrolled".into() });
        assert_eq!(work, Some(WorkId::new("a:1")));
        assert!(a.reader_here().unwrap().novel_scrolled());
    }

    fn width_set(task: Option<Command>) -> Option<f64> {
        match task? {
            Command::Reader(ReaderCommand::SetSetting {
                key: ReaderSettingKey::TextWidth,
                scope: SettingScope::Device,
                value: ReaderValue::Amount { value },
                ..
            }) => Some(value),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn brackets_change_the_width_by_four_between_40_and_the_window() {
        let mut a = novel(120, 34, "paged", None);
        assert_eq!(width_set(press(&mut a, KeyCode::Char(']'))), Some(70.0));
        assert_eq!(reading(&a).layout.shape.width, 70);
        for _ in 0..20 {
            press(&mut a, KeyCode::Char(']'));
        }
        assert_eq!(reading(&a).layout.shape.width, 116, "the window less 4");
        assert_eq!(width_set(press(&mut a, KeyCode::Char(']'))), None);
        for _ in 0..40 {
            press(&mut a, KeyCode::Char('['));
        }
        assert_eq!(reading(&a).layout.shape.width, 40);
        assert_eq!(width_set(press(&mut a, KeyCode::Char('['))), None);
    }

    #[test]
    fn a_narrow_window_takes_the_stage_less_two_each_side() {
        let a = novel(60, 20, "paged", None);
        let frame = a.novel_frame((60, 20)).unwrap();
        assert_eq!((frame.column.x, frame.column.width), (2, 56));
        assert_eq!(a.novel_status_words(false).as_deref(), Some("Part 3 · 0%"));
    }

    #[test]
    fn the_place_survives_a_resize_and_a_new_width() {
        let mut a = novel(120, 34, "paged", None);
        for _ in 0..2 {
            press(&mut a, KeyCode::Right);
        }
        let place = reading(&a).place.clone();
        a.take_commands();
        a.took_frame(FrameReport { window: Some((80, 24)), ..FrameReport::default() });
        let novel = reading(&a);
        assert_eq!(novel.place, place, "the place is kept");
        assert_eq!(novel.layout.line_of(&novel.open.text, &place), Some(novel.top));
        let said = reports(&a.take_commands());
        assert_eq!(said.len(), 1, "the new screens are said: {said:?}");
        assert_eq!((said[0].0.clone(), said[0].3), (place.clone(), TurnHow::Placed));
        press(&mut a, KeyCode::Char('['));
        let novel = reading(&a);
        assert_eq!(novel.place, place);
        assert_eq!(novel.layout.line_of(&novel.open.text, &place), Some(novel.top));
    }

    #[test]
    fn the_status_names_the_part_how_far_and_the_time_left() {
        let mut a = novel(120, 34, "paged", None);
        let words = a.novel_status_words(true).unwrap();
        assert!(words.starts_with("Part 3 · "), "{words}");
        assert!(words.ends_with(" min left"), "{words}");
        press(&mut a, KeyCode::Right);
        let later = a.novel_progress().unwrap();
        assert_eq!(later.title.as_deref(), Some("The Lamp Room"));
        assert!(later.fraction > 0.3 && later.fraction < 0.5, "{later:?}");
    }

    #[test]
    fn the_settings_overlay_changes_rows_at_once() {
        let mut a = novel(120, 34, "paged", None);
        a.take_commands();
        let asked = press(&mut a, KeyCode::Char(','));
        assert!(matches!(asked, Some(Command::Reader(ReaderCommand::LoadSettings { .. }))));
        assert_eq!(a.nav.overlay(), Some(&Overlay::ReaderSettings));
        let items = a.reader_setting_items();
        let keys: Vec<_> = items.iter().map(|i| i.key).collect();
        assert_eq!(
            keys,
            [
                ReaderSettingKey::Mode,
                ReaderSettingKey::TextWidth,
                ReaderSettingKey::LineSpacing,
                ReaderSettingKey::Justify
            ]
        );
        press(&mut a, KeyCode::Char('j'));
        let task = press(&mut a, KeyCode::Left);
        assert_eq!(width_set(task), Some(62.0));
        assert_eq!(reading(&a).layout.shape.width, 62, "laid out at once");
        press(&mut a, KeyCode::Down);
        let task = press(&mut a, KeyCode::Right);
        let Some(Command::Reader(ReaderCommand::SetSetting { key, value, scope, .. })) = task
        else {
            panic!("{task:?}")
        };
        assert_eq!((key, scope), (ReaderSettingKey::LineSpacing, SettingScope::Device));
        assert_eq!(value, ReaderValue::Amount { value: OPEN_AMOUNT });
        assert!(reading(&a).layout.shape.open);
        press(&mut a, KeyCode::Down);
        let task = press(&mut a, KeyCode::Enter);
        assert!(
            matches!(
                task,
                Some(Command::Reader(ReaderCommand::SetSetting {
                    key: ReaderSettingKey::Justify,
                    value: ReaderValue::Toggle { on: true },
                    ..
                }))
            ),
            "{task:?}"
        );
        press(&mut a, KeyCode::Esc);
        assert_eq!(a.nav.overlay(), None);
        assert!(a.reader_here().is_some(), "still reading");
    }

    #[test]
    fn a_novels_rows_share_one_heading_and_mode_is_still_kept_for_the_series() {
        let mut a = novel(120, 34, "paged", None);
        press(&mut a, KeyCode::Char(','));
        let items = a.reader_setting_items();
        assert!(
            items.iter().all(|i| i.place == crate::app::SettingPlace::Here),
            "under the novels on this computer, as the board groups them: {items:?}"
        );
        let task = press(&mut a, KeyCode::Right);
        let Some(Command::Reader(ReaderCommand::SetSetting {
            key, value, scope, work, ..
        })) = task
        else {
            panic!("{task:?}")
        };
        assert_eq!((key, scope), (ReaderSettingKey::Mode, SettingScope::Series));
        assert_eq!(value, ReaderValue::Choice { name: "scrolled".into() });
        assert_eq!(work, Some(WorkId::new("a:1")));
    }
}

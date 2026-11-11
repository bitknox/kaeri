//! The page between chapters, as the reader board and the Apple reader lay it out: what was
//! read and where it went, the next chapter with its facts and its first page, and Continue. With
//! no next chapter it says why, and the series page is the action. The column is 440 px,
//! left-aligned, centred in the window, and narrower in a narrower window, its buttons going one
//! under the other when they don't fit side by side; at a strip's foot it follows the last image.

use std::rc::Rc;

use adw::prelude::*;
use gtk::gdk;
use kaeri_i18n::t;
use kaeri_model::{reader::NoNext, reading::ChapterRow};

use crate::motion;

/// The column's width on a desktop window.
const COLUMN: i32 = 500;
/// The next chapter's first page, at the board's 1:1.42.
const PREVIEW_WIDTH: i32 = 180;

/// What the end page shows, from the reader's state.
pub struct EndFacts {
    /// The chapter just finished: its number, or its label when it has none.
    pub chapter: String,
    /// `None` until the chapter counts as read; then the trackers it went to.
    pub read: Option<Vec<String>>,
    pub next: Option<ChapterRow>,
    pub why: Option<NoNext>,
    pub source: Option<String>,
    pub preview: Option<gdk::Texture>,
    pub novel: bool,
    /// "1 credit page skipped", for credit pages skipped at the chapter's end and not shown.
    pub credits: Option<String>,
    /// When `next` is an extra: the first whole chapter after it, for "Skip to chapter 17".
    pub skip_to: Option<ChapterRow>,
}

pub struct EndPage {
    /// The paged reader's "end" child: the column centred, scrolling when the window is short.
    pub scroll: gtk::ScrolledWindow,
    holder: gtk::Box,
    /// The column, at most 440 px and never wider than the window.
    column: adw::Clamp,
    top: gtk::Label,
    read_veil: gtk::Overlay,
    read: gtk::Label,
    credit_row: gtk::Box,
    credit_text: gtk::Label,
    /// Show, beside the credit line: the reader connects it.
    pub credit_show: gtk::Button,
    next_box: gtk::Box,
    next_title: gtk::Label,
    facts: gtk::Label,
    preview_frame: gtk::Box,
    preview: gtk::Picture,
    plate: gtk::Label,
    none_box: gtk::Box,
    none_title: gtk::Label,
    none_body: gtk::Label,
    pub continue_button: gtk::Button,
    /// Past the extras, between Continue and Back to the series, when the next chapter is one.
    skip: gtk::Button,
    back: gtk::Button,
}

fn label(class: &str) -> gtk::Label {
    let label = gtk::Label::new(None);
    label.set_xalign(0.0);
    label.set_wrap(true);
    label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    label.add_css_class(class);
    label
}

impl EndPage {
    pub fn new(go_on: Rc<dyn Fn()>, skip: Rc<dyn Fn()>, back: Rc<dyn Fn()>) -> Rc<Self> {
        let top = label("reader-end-top");
        let read = label("reader-end-read");
        let read_veil = gtk::Overlay::new();
        read_veil.set_child(Some(&read));
        read_veil.set_margin_top(4);
        // Under what was updated: the credit pages skipped at the end, with Show beside them.
        let (credit_row, credit_text, credit_show) = super::credits::show_line();
        credit_row.set_margin_top(14);
        credit_row.set_visible(false);

        let next_label = label("reader-end-read");
        next_label.set_label(&t::reader_end_next());
        let next_title = label("reader-end-title");
        next_title.add_css_class("show-title");
        next_title.set_margin_top(4);
        let facts = label("reader-end-facts");
        facts.set_margin_top(4);
        let preview = gtk::Picture::new();
        preview.set_content_fit(gtk::ContentFit::Cover);
        preview.set_can_shrink(true);
        preview.set_hexpand(true);
        preview.set_vexpand(true);
        // With no first page to show, the plate keeps its place with the chapter's number.
        let plate = gtk::Label::new(None);
        plate.add_css_class("reader-end-plate");
        plate.add_css_class("numeric");
        plate.set_halign(gtk::Align::Center);
        plate.set_valign(gtk::Align::Center);
        plate.set_wrap(true);
        plate.set_justify(gtk::Justification::Center);
        let preview_stack = gtk::Overlay::new();
        preview_stack.set_child(Some(&preview));
        preview_stack.add_overlay(&plate);
        preview_stack.set_vexpand(true);
        let preview_frame = gtk::Box::new(gtk::Orientation::Vertical, 0);
        preview_frame.add_css_class("reader-end-preview");
        preview_frame.set_overflow(gtk::Overflow::Hidden);
        preview_frame
            .set_size_request(PREVIEW_WIDTH, (PREVIEW_WIDTH as f32 * 1.42).round() as i32);
        preview_frame.set_halign(gtk::Align::Start);
        preview_frame.set_margin_top(18);
        preview_frame.append(&preview_stack);
        preview_frame.set_accessible_role(gtk::AccessibleRole::Presentation);
        let next_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        next_box.set_margin_top(22);
        next_box.append(&next_label);
        next_box.append(&next_title);
        next_box.append(&facts);
        next_box.append(&preview_frame);

        let none_title = label("reader-end-none");
        none_title.add_css_class("show-title");
        let none_body = label("reader-end-body");
        let none_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
        none_box.set_margin_top(22);
        none_box.append(&none_title);
        none_box.append(&none_body);

        let continue_button = gtk::Button::with_label(&t::linux_reader_continue());
        continue_button.add_css_class("pill");
        continue_button.add_css_class("suggested-action");
        continue_button.connect_clicked(move |_| go_on());
        let skip_button = gtk::Button::new();
        skip_button.add_css_class("pill");
        skip_button.set_visible(false);
        skip_button.connect_clicked(move |_| skip());
        let back_button = gtk::Button::with_label(&t::linux_reader_back_to_series());
        back_button.add_css_class("pill");
        back_button.connect_clicked(move |_| back());
        let buttons = adw::WrapBox::builder().child_spacing(10).line_spacing(10).build();
        buttons.set_margin_top(22);
        buttons.set_halign(gtk::Align::Start);
        buttons.append(&continue_button);
        buttons.append(&skip_button);
        buttons.append(&back_button);

        let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
        body.add_css_class("reader-end");
        body.append(&top);
        body.append(&read_veil);
        body.append(&credit_row);
        body.append(&next_box);
        body.append(&none_box);
        body.append(&buttons);
        let column = adw::Clamp::builder()
            .maximum_size(COLUMN)
            .tightening_threshold(COLUMN)
            .child(&body)
            .build();

        // From the top, as the board draws every end state, so the words stay put whether a
        // next chapter follows or not.
        let holder = gtk::Box::new(gtk::Orientation::Vertical, 0);
        holder.set_valign(gtk::Align::Start);
        holder.set_margin_top(96);
        holder.set_margin_bottom(96);
        holder.set_margin_start(24);
        holder.set_margin_end(24);
        holder.append(&column);
        let scroll = gtk::ScrolledWindow::builder()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .child(&holder)
            .build();
        Rc::new(Self {
            scroll,
            holder,
            column,
            top,
            read_veil,
            read,
            credit_row,
            credit_text,
            credit_show,
            next_box,
            next_title,
            facts,
            preview_frame,
            preview,
            plate,
            none_box,
            none_title,
            none_body,
            continue_button,
            skip: skip_button,
            back: back_button,
        })
    }

    /// Put the column in the paged reader's end child.
    pub fn in_page(&self) {
        if self.column.parent().as_ref() != Some(self.holder.upcast_ref()) {
            self.column.unparent();
            self.holder.append(&self.column);
        }
    }

    /// Put the column at the foot of a strip, in `foot`.
    pub fn at_foot(&self, foot: &gtk::Box) {
        if self.column.parent().as_ref() != Some(foot.upcast_ref()) {
            self.column.unparent();
            foot.append(&self.column);
        }
    }

    /// Let go of the next chapter's preview, as the reader closes.
    pub fn forget(&self) {
        self.preview.set_paintable(gdk::Paintable::NONE);
    }

    /// Show `facts`. A change of the read line crossfades in place; the rest is set as it is.
    pub fn fill(&self, facts: &EndFacts) {
        self.top.set_label(&t::reader_end_of_chapter_number(&facts.chapter));
        let read = read_line(facts.read.as_deref(), crate::widgets::is_offline());
        if self.read.label() != read {
            if self.read.label().is_empty() || !self.read.is_mapped() {
                self.read.set_label(&read);
            } else {
                let line = self.read.clone();
                motion::ghost(&self.read_veil, &self.read, false, move || {
                    line.set_label(&read)
                });
            }
        }
        self.credit_row.set_visible(facts.credits.is_some());
        self.credit_text.set_label(facts.credits.as_deref().unwrap_or_default());
        // Show is named by its line, so it says what it shows: "1 credit page skipped, button".
        if let Some(words) = facts.credits.as_deref() {
            self.credit_show.update_property(&[gtk::accessible::Property::Label(words)]);
        }
        match &facts.next {
            Some(next) => {
                self.next_box.set_visible(true);
                self.none_box.set_visible(false);
                self.next_title.set_label(next.title.as_deref().unwrap_or(&next.label));
                let line = next_facts(next, facts.source.as_deref());
                self.facts.set_label(&line);
                self.facts.set_visible(!line.is_empty());
                self.preview_frame.set_visible(!facts.novel);
                self.preview.set_paintable(facts.preview.as_ref());
                self.plate.set_label(
                    &next.number.as_deref().map_or(next.label.clone(), t::reader_chapter_line),
                );
                self.plate.set_visible(facts.preview.is_none());
                self.continue_button.set_visible(true);
                self.skip.set_visible(facts.skip_to.is_some());
                if let Some(skip) = &facts.skip_to {
                    self.skip.set_label(&t::reader_end_skip_to(
                        skip.number.as_deref().unwrap_or(&skip.label),
                    ));
                    // What stays behind: "Chapter 16.5 stays unread."
                    let extra = next.number.as_deref().unwrap_or(&next.label);
                    self.skip.update_property(&[gtk::accessible::Property::Description(
                        &t::reader_end_skip_hint(extra),
                    )]);
                }
                self.back.remove_css_class("suggested-action");
            }
            None => {
                self.next_box.set_visible(false);
                let why = facts.why.unwrap_or(NoNext::CaughtUp);
                self.none_title.set_label(&why_title(why));
                let body = why_body(why);
                self.none_body.set_label(body.as_deref().unwrap_or(""));
                self.none_body.set_visible(body.is_some());
                self.none_box.set_visible(true);
                self.continue_button.set_visible(false);
                self.skip.set_visible(false);
                self.back.add_css_class("suggested-action");
            }
        }
    }
}

/// "Read · AniList and MangaUpdates updated", Read alone when no tracker took it, and a line
/// saying it has not counted yet before the read rule fired. Offline the trackers only hear of
/// it once the device is back online, and the line says so.
fn read_line(read: Option<&[String]>, offline: bool) -> String {
    match read {
        None => t::linux_reader_end_not_read(),
        Some([]) => t::reader_end_read(),
        Some(names) => {
            let names: Vec<&str> = names.iter().map(|n| crate::say::tracker_name(n)).collect();
            if offline {
                t::reader_end_tracker_offline(&joined(&names))
            } else {
                t::reader_end_tracker(&joined(&names))
            }
        }
    }
}

/// Names joined as a sentence: "A", "A and B", "A, B and C".
fn joined(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [rest @ .., last] => t::linux_reader_names_and(&rest.join(", "), last),
    }
}

/// "Chapter 13 · 34 pages · From books.example": the chapter's number when it has a title of
/// its own, its length, and the source it came from.
fn next_facts(next: &ChapterRow, source: Option<&str>) -> String {
    let chapter =
        next.title.as_ref().map(|_| next.number.clone().unwrap_or(next.label.clone()));
    let pages = next.pages.filter(|p| *p > 0).map(|p| t::reader_end_pages(i64::from(p)));
    let facts = match (chapter, pages) {
        (Some(chapter), Some(pages)) => Some(t::reader_end_facts(&chapter, &pages)),
        (Some(chapter), None) => Some(t::reader_chapter_line(&chapter)),
        (None, pages) => pages,
    };
    match (facts, source) {
        (Some(facts), Some(source)) => t::reader_divider_facts_source(&facts, source),
        (None, Some(source)) => t::reader_end_from(source),
        (facts, None) => facts.unwrap_or_default(),
    }
}

fn why_title(why: NoNext) -> String {
    match why {
        NoNext::NotOutYet => t::linux_reader_end_not_out(),
        NoNext::CaughtUp => t::reader_end_caught_up(),
        NoNext::Finished => t::linux_reader_end_finished(),
        NoNext::NoSourceHasIt => t::linux_reader_end_no_source(),
        NoNext::Offline => t::linux_reader_end_offline(),
    }
}

fn why_body(why: NoNext) -> Option<String> {
    match why {
        NoNext::NotOutYet => Some(t::linux_reader_end_not_out_body()),
        NoNext::CaughtUp => Some(t::linux_reader_end_caught_up_body()),
        NoNext::Finished => Some(t::linux_reader_end_finished_body()),
        NoNext::NoSourceHasIt => None,
        NoNext::Offline => Some(t::linux_reader_end_offline_body()),
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use adw::prelude::*;

    use super::{COLUMN, EndPage, joined, read_line};
    use crate::gtk_test::{pump, run};

    /// The end page asks for no more width than its widest part: never the column's 440 px or
    /// both buttons side by side. At the normal text size it fits the narrowest window, and at
    /// twice the size its buttons go one under the other; on a wide window the column keeps its
    /// 440 px.
    #[test]
    fn the_end_page_fits_a_narrow_window_at_any_text_size() {
        run(|| {
            let min_width =
                |widget: &gtk::Widget| widget.measure(gtk::Orientation::Horizontal, -1).0;
            let end = EndPage::new(Rc::new(|| {}), Rc::new(|| {}), Rc::new(|| {}));
            let window = gtk::Window::builder().default_width(1280).default_height(800).build();
            window.set_child(Some(&end.scroll));
            window.present();
            pump(150);
            let normal = min_width(end.scroll.upcast_ref());
            let wide = end.column.child().map(|body| body.width());

            let settings = gtk::Settings::default().expect("a display");
            let dpi = settings.gtk_xft_dpi();
            settings.set_gtk_xft_dpi(2 * 96 * 1024);
            pump(150);
            let large = min_width(end.scroll.upcast_ref());
            let widest = [end.continue_button.upcast_ref(), end.back.upcast_ref()]
                .into_iter()
                .map(min_width)
                .max()
                .unwrap_or(0);
            settings.set_gtk_xft_dpi(dpi);
            window.destroy();

            assert!(normal <= 360, "the end page needs {normal} px at the normal text size");
            // The column's margins on each side.
            assert!(
                large <= widest + 48,
                "{large} px at twice the size, its widest part {widest}"
            );
            assert_eq!(wide, Some(COLUMN));
        });
    }

    #[test]
    fn names_join_as_a_sentence() {
        assert_eq!(joined(&[]), "");
        assert_eq!(joined(&["Alpha"]), "Alpha");
        assert_eq!(joined(&["Alpha", "Beta"]), "Alpha and Beta");
        assert_eq!(joined(&["Alpha", "Beta", "Gamma"]), "Alpha, Beta and Gamma");
    }

    /// Offline the trackers only hear of the read later, and the line says so.
    #[test]
    fn the_read_line_waits_for_the_network_offline() {
        let names = ["Alpha".to_owned()];
        let online = read_line(Some(&names), false);
        let offline = read_line(Some(&names), true);
        assert_eq!(online, kaeri_i18n::t::reader_end_tracker("Alpha"));
        assert_eq!(offline, kaeri_i18n::t::reader_end_tracker_offline("Alpha"));
        assert_ne!(online, offline);
        // With no tracker there is nothing to wait for.
        assert_eq!(read_line(Some(&[]), true), read_line(Some(&[]), false));
    }
}

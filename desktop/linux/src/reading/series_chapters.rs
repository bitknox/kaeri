//! The series page's chapters, drawn as the page scrolls to them. A series can list a thousand
//! chapters and more, and a row is a dozen widgets, so the page draws the first rows and adds
//! more as the bottom comes near. A new listing with the same chapters in the same places
//! changes only the rows that changed, so reading a chapter and coming back redraws one row,
//! not the list.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use adw::prelude::*;
use gtk::glib;
use kaeri_core::books::WorkId;
use kaeri_i18n::t;
use kaeri_model::reading::{ChapterRow, VolumeGroup};

use super::series::{Order, chapter_row, chapters_head};
use crate::widgets;

/// Rows drawn at a time: a tall window's worth, twice over.
pub const CHUNK: usize = 40;

/// One step of drawing the list, in order. Read volumes in a row share one list, each folded
/// into one line; any other volume has its heading, when there is more than one, and its list.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Fold(usize),
    Heading(String),
    List,
    Row(usize, usize),
}

/// What a step drew: a chapter's row or a folded volume, with the list it sits in.
enum Drawn {
    Row(gtk::ListBoxRow, gtk::ListBox),
    Fold(adw::ExpanderRow, gtk::ListBox),
    Other,
}

#[derive(Default)]
struct Listing {
    volumes: Vec<VolumeGroup>,
    failure: Option<String>,
    order: Option<Order>,
    steps: Vec<Step>,
    /// What each step drawn so far drew, by step.
    drawn: Vec<Drawn>,
    /// The rows drawn so far, a folded volume counting as one.
    rows: usize,
    folded: Option<gtk::ListBox>,
    list: Option<gtk::ListBox>,
}

pub(super) struct SeriesChapters {
    column: gtk::Box,
    work: WorkId,
    listing: RefCell<Listing>,
    /// More rows are on their way, after what is drawing now.
    growing: Cell<bool>,
}

impl SeriesChapters {
    /// The list in `column`, growing as `scroll` nears its end.
    pub(super) fn new(
        column: &gtk::Box,
        scroll: &gtk::ScrolledWindow,
        work: &WorkId,
    ) -> Rc<Self> {
        let this = Rc::new(Self {
            column: column.clone(),
            work: work.clone(),
            listing: RefCell::default(),
            growing: Cell::new(false),
        });
        let adjustment = scroll.vadjustment();
        let near_end = {
            let this = Rc::downgrade(&this);
            move |adjustment: &gtk::Adjustment| {
                if let Some(this) = this.upgrade() {
                    this.grow_if_near(adjustment);
                }
            }
        };
        adjustment.connect_value_changed(near_end.clone());
        adjustment.connect_changed(near_end);
        this
    }

    /// Show the engine's listing in `order`. The chapters in the same places as before change
    /// only the rows whose chapter changed; anything else draws the list again, as far as it
    /// was drawn.
    pub(super) fn show(&self, volumes: &[VolumeGroup], failure: Option<&str>, order: Order) {
        // The engine lists oldest first; newest first turns the volumes and their chapters round.
        let volumes: Vec<VolumeGroup> = if order.newest {
            volumes
                .iter()
                .rev()
                .map(|group| {
                    let mut group = group.clone();
                    group.chapters.reverse();
                    group
                })
                .collect()
        } else {
            volumes.to_vec()
        };
        let steps = steps(&volumes);
        let failure = failure.map(str::to_owned);
        let same = {
            let old = self.listing.borrow();
            old.order == Some(order)
                && old.failure == failure
                && old.steps == steps
                && same_chapters(&old.volumes, &volumes)
        };
        if same {
            self.update(volumes);
            return;
        }
        let rows = self.listing.borrow().rows.max(CHUNK);
        super::clear(&self.column);
        self.column.append(&chapters_head(order));
        if let Some(failure) = &failure {
            self.column.append(&widgets::haze(failure, "caption"));
        }
        self.listing.replace(Listing {
            volumes,
            failure,
            order: Some(order),
            steps,
            ..Listing::default()
        });
        self.draw_to(rows);
    }

    /// Draw more once the bottom of what is drawn is within a screen of showing. A row that
    /// takes the keyboard while the list changes scrolls to it: the list is busy then, and the
    /// next scroll asks again.
    fn grow_if_near(self: &Rc<Self>, adjustment: &gtk::Adjustment) {
        let more = self.listing.try_borrow().is_ok_and(|listing| {
            listing.order.is_some() && listing.drawn.len() < listing.steps.len()
        });
        if self.growing.get() || !more {
            return;
        }
        let page = adjustment.page_size();
        if adjustment.value() + page * 2.0 < adjustment.upper() {
            return;
        }
        // After the layout under way, not inside it.
        self.growing.set(true);
        let this = Rc::downgrade(self);
        glib::idle_add_local_once(move || {
            let Some(this) = this.upgrade() else { return };
            this.growing.set(false);
            let rows = this.listing.borrow().rows + CHUNK;
            this.draw_to(rows);
        });
    }

    /// Draw steps until `rows` rows are drawn, or the list is.
    fn draw_to(&self, rows: usize) {
        let mut listing = self.listing.borrow_mut();
        while listing.rows < rows && listing.drawn.len() < listing.steps.len() {
            let step = listing.steps[listing.drawn.len()].clone();
            let drawn = self.draw_step(&mut listing, &step);
            listing.drawn.push(drawn);
        }
    }

    fn draw_step(&self, listing: &mut Listing, step: &Step) -> Drawn {
        match step {
            Step::Fold(group) => {
                let newest = listing.order.is_some_and(|o| o.newest);
                let fold = fold(&self.work, &listing.volumes[*group], newest);
                let list = listing
                    .folded
                    .get_or_insert_with(|| {
                        let list = widgets::boxed_list();
                        self.column.append(&list);
                        list
                    })
                    .clone();
                list.append(&fold);
                listing.rows += 1;
                Drawn::Fold(fold, list)
            }
            Step::Heading(name) => {
                listing.folded = None;
                let heading = widgets::heading(name);
                heading.remove_css_class("title-4");
                heading.add_css_class("heading");
                self.column.append(&heading);
                Drawn::Other
            }
            Step::List => {
                listing.folded = None;
                let list = widgets::boxed_list();
                self.column.append(&list);
                listing.list = Some(list);
                Drawn::Other
            }
            Step::Row(group, index) => {
                let chapter = &listing.volumes[*group].chapters[*index];
                let row = chapter_row(&self.work, chapter);
                let Some(list) = listing.list.clone() else { return Drawn::Other };
                list.append(&row);
                listing.rows += 1;
                Drawn::Row(row, list)
            }
        }
    }

    /// The same chapters in the same places: the rows drawn whose chapter changed are drawn
    /// again where they are, and the keyboard stays on a row that had it.
    fn update(&self, volumes: Vec<VolumeGroup>) {
        let mut listing = self.listing.borrow_mut();
        let old = std::mem::replace(&mut listing.volumes, volumes);
        let newest = listing.order.is_some_and(|o| o.newest);
        let Listing { volumes, steps, drawn, .. } = &mut *listing;
        for (step, drawn) in steps.iter().zip(drawn.iter_mut()) {
            match (step, drawn) {
                (Step::Row(g, i), Drawn::Row(row, list)) => {
                    let chapter = &volumes[*g].chapters[*i];
                    if old[*g].chapters[*i] == *chapter {
                        continue;
                    }
                    let fresh = chapter_row(&self.work, chapter);
                    swap(list, row, &fresh);
                    *row = fresh;
                }
                (Step::Fold(g), Drawn::Fold(fold, list)) => {
                    if old[*g] == volumes[*g] {
                        continue;
                    }
                    let fresh = self::fold(&self.work, &volumes[*g], newest);
                    fresh.set_expanded(fold.is_expanded());
                    swap(list, fold, &fresh);
                    *fold = fresh;
                }
                _ => {}
            }
        }
    }

    /// The rows drawn so far, a folded volume counting as one.
    #[cfg(test)]
    fn rows(&self) -> usize {
        self.listing.borrow().rows
    }
}

/// Put `fresh` in `old`'s place in `list`, with the keyboard if `old` had it.
fn swap(
    list: &gtk::ListBox,
    old: &impl IsA<gtk::ListBoxRow>,
    fresh: &impl IsA<gtk::ListBoxRow>,
) {
    let (old, fresh) = (old.as_ref(), fresh.as_ref());
    let focused = old.has_focus() || old.focus_child().is_some();
    let at = old.index();
    list.remove(old);
    list.insert(fresh, at);
    if focused {
        fresh.grab_focus();
    }
}

/// The steps that draw `volumes`, in the order they show.
fn steps(volumes: &[VolumeGroup]) -> Vec<Step> {
    let mut steps = Vec::new();
    for (g, group) in volumes.iter().enumerate() {
        if group.all_read && volumes.len() > 1 {
            steps.push(Step::Fold(g));
            continue;
        }
        if volumes.len() > 1 {
            let name =
                group.volume.as_deref().map_or_else(t::series_no_volume, t::series_volume);
            steps.push(Step::Heading(name));
        }
        steps.push(Step::List);
        steps.extend((0..group.chapters.len()).map(|i| Step::Row(g, i)));
    }
    steps
}

/// The same chapters, by id, in each volume.
fn same_chapters(old: &[VolumeGroup], new: &[VolumeGroup]) -> bool {
    old.len() == new.len()
        && old.iter().zip(new).all(|(a, b)| {
            a.chapters.len() == b.chapters.len()
                && a.chapters.iter().zip(&b.chapters).all(|(a, b)| a.id == b.id)
        })
}

/// A read volume folded into one line: "Volume 9 · chapters 77 to 84 · read". Its chapters
/// are drawn the first time it opens.
fn fold(work: &WorkId, group: &VolumeGroup, newest: bool) -> adw::ExpanderRow {
    let name = group.volume.as_deref().map_or_else(t::series_no_volume, t::series_volume);
    let mut line = Vec::new();
    let (first, last) = (group.chapters.first(), group.chapters.last());
    // Newest first lists a volume's chapters high to low; the range reads low to high.
    let (low, high) = if newest { (last, first) } else { (first, last) };
    if let (Some(low), Some(high)) = (low, high) {
        let number = |c: &ChapterRow| c.number.clone().unwrap_or_else(|| c.label.clone());
        line.push(t::series_chapter_range(&number(low), &number(high)));
    }
    line.push(t::series_volume_read());
    let fold = adw::ExpanderRow::builder()
        .title(glib::markup_escape_text(&name))
        .subtitle(glib::markup_escape_text(&line.join(" \u{b7} ")))
        .build();
    let chapters = group.chapters.clone();
    let work = work.clone();
    let filled = Cell::new(false);
    fold.connect_expanded_notify(move |fold| {
        if fold.is_expanded() && !filled.replace(true) {
            for chapter in &chapters {
                fold.add_row(&chapter_row(&work, chapter));
            }
        }
    });
    fold
}

#[cfg(test)]
mod tests {
    use kaeri_core::books::ChapterId;
    use kaeri_model::reading::ChapterReadState;

    use super::*;
    use crate::gtk_test::{pump, run};

    fn chapter(n: usize) -> ChapterRow {
        ChapterRow {
            id: ChapterId(format!("c{n}")),
            label: format!("Chapter {n}"), // i18n-skip
            state: ChapterReadState::Unread,
            number: Some(n.to_string()),
            title: None,
            volume: None,
            published: None,
            pages: None,
            new_chapter: false,
            download: None,
            versions: 1,
            bookmarked: false,
            version_label: None,
            read_at: None,
            line: Vec::new(),
            gone_from_source: false,
            parts: 0,
            missing_parts: Vec::new(),
            part_groups: Vec::new(),
            other_group: false,
            group_language: None,
            other_source: None,
        }
    }

    fn one_volume(count: usize) -> Vec<VolumeGroup> {
        vec![VolumeGroup {
            volume: None,
            title: None,
            chapters: (1..=count).map(chapter).collect(),
            all_read: false,
        }]
    }

    /// The chapter rows in `widget`, in order, by their action target's chapter.
    fn rows(widget: &gtk::Widget) -> Vec<gtk::ListBoxRow> {
        let mut found = Vec::new();
        let mut child = widget.first_child();
        while let Some(next) = child {
            if let Some(row) = next.downcast_ref::<gtk::ListBoxRow>()
                && row.action_name().is_some()
            {
                found.push(row.clone());
            }
            found.extend(rows(&next));
            child = next.next_sibling();
        }
        found
    }

    fn scene() -> (gtk::Window, gtk::ScrolledWindow, gtk::Box, Rc<SeriesChapters>) {
        let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
        let scroll = gtk::ScrolledWindow::builder().child(&column).build();
        let window = gtk::Window::builder().default_width(600).default_height(500).build();
        window.set_child(Some(&scroll));
        let list = SeriesChapters::new(&column, &scroll, &WorkId("w".into()));
        (window, scroll, column, list)
    }

    const ORDER: Order = Order { newest: false, novel: false };

    #[test]
    fn a_long_series_draws_a_chunk_and_more_near_the_end() {
        run(|| {
            let (window, scroll, column, list) = scene();
            list.show(&one_volume(1200), None, ORDER);
            assert_eq!(rows(column.upcast_ref()).len(), CHUNK);
            window.present();
            pump(200);
            let adjustment = scroll.vadjustment();
            adjustment.set_value(adjustment.upper());
            pump(200);
            assert!(list.rows() > CHUNK, "{}", list.rows());
            assert!(list.rows() < 1200, "{}", list.rows());
            window.destroy();
        });
    }

    #[test]
    fn a_changed_chapter_redraws_its_row_alone() {
        run(|| {
            let (window, _scroll, column, list) = scene();
            let mut volumes = one_volume(1200);
            list.show(&volumes, None, ORDER);
            let before = rows(column.upcast_ref());
            volumes[0].chapters[3].state = ChapterReadState::Read;
            list.show(&volumes, None, ORDER);
            let after = rows(column.upcast_ref());
            assert_eq!(before.len(), after.len());
            for (i, (a, b)) in before.iter().zip(&after).enumerate() {
                assert_eq!(a == b, i != 3, "row {i}");
            }
            window.destroy();
        });
    }

    #[test]
    fn another_order_draws_again_as_far_as_before() {
        run(|| {
            let (window, _scroll, column, list) = scene();
            let volumes = one_volume(300);
            list.show(&volumes, None, ORDER);
            list.draw_to(CHUNK * 2);
            list.show(&volumes, None, Order { newest: true, novel: false });
            let drawn = rows(column.upcast_ref());
            assert_eq!(drawn.len(), CHUNK * 2);
            let first =
                drawn[0].action_target_value().and_then(|v| v.get::<(String, String)>());
            assert_eq!(first.map(|(_, c)| c), Some("c300".to_owned()));
            window.destroy();
        });
    }

    #[test]
    fn a_folded_volume_draws_its_chapters_when_it_opens() {
        run(|| {
            let (window, _scroll, column, list) = scene();
            let mut volumes = one_volume(30);
            volumes[0].all_read = true;
            volumes[0].volume = Some("1".into());
            volumes.push(VolumeGroup {
                volume: Some("2".into()),
                title: None,
                chapters: (31..=40).map(chapter).collect(),
                all_read: false,
            });
            list.show(&volumes, None, ORDER);
            assert_eq!(rows(column.upcast_ref()).len(), 10);
            let fold = list.listing.borrow().drawn.iter().find_map(|d| match d {
                Drawn::Fold(fold, _) => Some(fold.clone()),
                _ => None,
            });
            let fold = fold.expect("a folded volume");
            fold.set_expanded(true);
            assert_eq!(rows(column.upcast_ref()).len(), 40);
            window.destroy();
        });
    }
}

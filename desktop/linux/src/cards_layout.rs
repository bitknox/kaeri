//! Cards in rows: as many columns of one width as fit, left-aligned with a gap across and one
//! down (the same unless set apart), the room left over staying at the end of each row. GtkFlowBox spreads that room between its
//! children, so a grid of covers would drift apart as the window widens.

use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct CardsLayout {
        /// Each card's width.
        pub card: Cell<i32>,
        /// The gap between cards across.
        pub gap: Cell<i32>,
        /// The gap between rows of cards.
        pub row_gap: Cell<i32>,
        /// Only this many rows are shown, each full, when above 0: placeholders that fill whole
        /// rows at any width. The cards past them are kept out of sight.
        pub full_rows: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for CardsLayout {
        const NAME: &'static str = "KaeriCardsLayout"; // i18n-skip
        type Type = super::CardsLayout;
        type ParentType = gtk::LayoutManager;
    }

    impl ObjectImpl for CardsLayout {}

    impl LayoutManagerImpl for CardsLayout {
        fn request_mode(&self, _widget: &gtk::Widget) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(
            &self,
            widget: &gtk::Widget,
            orientation: gtk::Orientation,
            for_size: i32,
        ) -> (i32, i32, i32, i32) {
            let (card, gap, row_gap) = (self.card.get(), self.gap.get(), self.row_gap.get());
            if orientation == gtk::Orientation::Horizontal {
                return (card, card * 4 + gap * 3, -1, -1);
            }
            let columns = if for_size < 0 { 4 } else { self.columns(for_size) };
            let rows = self.rows(widget, columns);
            let height = rows.iter().sum::<i32>() + row_gap * (rows.len() as i32 - 1).max(0);
            (height, height, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, _height: i32, _baseline: i32) {
            let (card, gap, row_gap) = (self.card.get(), self.gap.get(), self.row_gap.get());
            let columns = self.columns(width);
            let rows = self.rows(widget, columns);
            let shown = self.shown(columns);
            let mut child = widget.first_child();
            let (mut at, mut y) = (0, 0);
            while let Some(c) = child {
                child = c.next_sibling();
                if !c.should_layout() {
                    continue;
                }
                c.set_child_visible(at < shown);
                if at >= shown {
                    // Out of sight, laid over the first card at its size.
                    c.size_allocate(&gtk::Allocation::new(0, 0, card, rows[0]), -1);
                    at += 1;
                    continue;
                }
                let (column, row) = (at % columns, (at / columns) as usize);
                if column == 0 && row > 0 {
                    y += rows[row - 1] + row_gap;
                }
                let x = column * (card + gap);
                c.size_allocate(&gtk::Allocation::new(x, y, card, rows[row]), -1);
                at += 1;
            }
        }
    }

    impl CardsLayout {
        /// How many cards fit in `width`.
        pub fn columns(&self, width: i32) -> i32 {
            let (card, gap) = (self.card.get(), self.gap.get());
            ((width + gap) / (card + gap).max(1)).max(1)
        }

        /// How many cards show with `columns` across.
        pub fn shown(&self, columns: i32) -> i32 {
            match self.full_rows.get() {
                0 => i32::MAX,
                rows => columns.saturating_mul(rows),
            }
        }

        /// Each row's height: the tallest card in it.
        fn rows(&self, widget: &gtk::Widget, columns: i32) -> Vec<i32> {
            let shown = self.shown(columns);
            let mut rows = Vec::new();
            let mut child = widget.first_child();
            let mut at = 0;
            while let Some(c) = child {
                child = c.next_sibling();
                if !c.should_layout() {
                    continue;
                }
                if at >= shown {
                    break;
                }
                let (min, ..) = c.measure(gtk::Orientation::Vertical, self.card.get());
                if at % columns == 0 {
                    rows.push(min);
                } else if let Some(last) = rows.last_mut() {
                    *last = (*last).max(min);
                }
                at += 1;
            }
            rows
        }
    }
}

glib::wrapper! {
    pub struct CardsLayout(ObjectSubclass<imp::CardsLayout>) @extends gtk::LayoutManager;
}

impl CardsLayout {
    /// Cards `card` px wide, `gap` px apart.
    pub fn new(card: i32, gap: i32) -> Self {
        Self::apart(card, gap, gap)
    }

    /// Cards `card` px wide, `gap` px apart across and `row_gap` px down.
    pub fn apart(card: i32, gap: i32, row_gap: i32) -> Self {
        let layout: Self = glib::Object::new();
        layout.imp().card.set(card);
        layout.imp().gap.set(gap);
        layout.imp().row_gap.set(row_gap);
        layout
    }

    /// As [`Self::apart`], showing only `rows` full rows of the cards it is given.
    pub fn full_rows(card: i32, gap: i32, row_gap: i32, rows: i32) -> Self {
        let layout = Self::apart(card, gap, row_gap);
        layout.imp().full_rows.set(rows);
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_fit_the_width_with_their_gaps() {
        let imp = imp::CardsLayout::default();
        imp.card.set(144);
        imp.gap.set(16);
        assert_eq!(imp.columns(144), 1);
        assert_eq!(imp.columns(303), 1);
        assert_eq!(imp.columns(304), 2);
        assert_eq!(imp.columns(10), 1);
    }

    #[test]
    fn full_rows_show_whole_rows_only() {
        let imp = imp::CardsLayout::default();
        assert_eq!(imp.shown(5), i32::MAX);
        imp.full_rows.set(2);
        assert_eq!(imp.shown(5), 10);
        assert_eq!(imp.shown(7), 14);
    }
}

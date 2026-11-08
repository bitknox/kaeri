//! Cards in rows: as many columns of one width as fit, left-aligned with one gap both ways, the
//! room left over staying at the end of each row. GtkFlowBox spreads that room between its
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
        /// The gap between cards, across and down.
        pub gap: Cell<i32>,
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
            let (card, gap) = (self.card.get(), self.gap.get());
            if orientation == gtk::Orientation::Horizontal {
                return (card, card * 4 + gap * 3, -1, -1);
            }
            let columns = if for_size < 0 { 4 } else { self.columns(for_size) };
            let rows = self.rows(widget, columns);
            let height = rows.iter().sum::<i32>() + gap * (rows.len() as i32 - 1).max(0);
            (height, height, -1, -1)
        }

        fn allocate(&self, widget: &gtk::Widget, width: i32, _height: i32, _baseline: i32) {
            let (card, gap) = (self.card.get(), self.gap.get());
            let columns = self.columns(width);
            let rows = self.rows(widget, columns);
            let mut child = widget.first_child();
            let (mut at, mut y) = (0, 0);
            while let Some(c) = child {
                child = c.next_sibling();
                if !c.should_layout() {
                    continue;
                }
                let (column, row) = (at % columns, (at / columns) as usize);
                if column == 0 && row > 0 {
                    y += rows[row - 1] + gap;
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

        /// Each row's height: the tallest card in it.
        fn rows(&self, widget: &gtk::Widget, columns: i32) -> Vec<i32> {
            let mut rows = Vec::new();
            let mut child = widget.first_child();
            let mut at = 0;
            while let Some(c) = child {
                child = c.next_sibling();
                if !c.should_layout() {
                    continue;
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
        let layout: Self = glib::Object::new();
        layout.imp().card.set(card);
        layout.imp().gap.set(gap);
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
}

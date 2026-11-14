//! Spreads and leaves: which pages share a screen, and how a spread is shown on a screen that
//! holds one page.
//!
//! The rules, from the e-ink and reader boards: a page wider than [`SPREAD_ASPECT`] is a
//! spread. On a two-page screen the cover stands alone, spreads stand alone, and two pages pair
//! when their aspects are within [`PAIR_TOLERANCE`]. On a one-page screen a spread is split
//! into halves, rotated, or shown whole, as set. The engine maps these leaves onto the model's
//! `Leaf` records, so no app repeats the rule.

use crate::PixelRect;

/// Width over height above which a page is a spread.
pub const SPREAD_ASPECT: f32 = 1.16;

/// Two pages pair on a two-page screen when their aspects differ by this much or less.
pub const PAIR_TOLERANCE: f32 = 0.1;

/// Width over height taken for a page whose size is not known (a side of 0, as for a page whose
/// header could not be read): a usual comic page, 5 by 7.
pub const UNKNOWN_ASPECT: f32 = 5.0 / 7.0;

/// Whether a page of this size is a spread.
pub fn is_spread(width: u32, height: u32) -> bool {
    height > 0 && width as f32 > height as f32 * SPREAD_ASPECT
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Direction {
    #[default]
    LeftToRight,
    RightToLeft,
}

/// How many pages a screen shows side by side.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Screen {
    #[default]
    Single,
    Double,
}

/// What a one-page screen does with a spread.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpreadOnSingle {
    /// Two leaves, one half each, in reading order.
    #[default]
    Split,
    /// One leaf, turned a quarter so the spread fills the screen's length.
    Rotate,
    /// One leaf, the spread fitted as it is.
    Whole,
}

/// What a page is, as far as layout is concerned. Mirrors the model's `PageKind`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PageKind {
    #[default]
    Story,
    Spread,
    Cover,
    /// Listed by the book but not shown (ComicInfo's deleted pages). Never in a leaf.
    Skip,
    /// One screen of a vertical strip; see [`crate::strip`].
    StripCut,
}

/// One page as the layout sees it: its size (after any crop) and what the book says it is.
/// The book's word is [`PageKind::Story`] unless it marks a cover or a skipped page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageShape {
    pub width: u32,
    pub height: u32,
    pub kind: PageKind,
}

impl PageShape {
    pub fn story(width: u32, height: u32) -> Self {
        PageShape { width, height, kind: PageKind::Story }
    }

    /// Width over height, [`UNKNOWN_ASPECT`] for a page with a side of 0. Always finite and
    /// above 0, so places worked out from it are finite.
    fn aspect(&self) -> f32 {
        if self.width == 0 || self.height == 0 {
            return UNKNOWN_ASPECT;
        }
        PixelRect::full(self.width, self.height).aspect()
    }
}

/// A rectangle in a leaf, 0 to 1 on each axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnitRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UnitRect {
    pub const FULL: UnitRect = UnitRect { x: 0.0, y: 0.0, width: 1.0, height: 1.0 };
}

/// One page, or half of one, in a leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LeafPart {
    /// The page's index in the input.
    pub page: u32,
    pub kind: PageKind,
    /// The half of a split spread this part shows, in the page's pixels; `None` for all of it.
    pub source: Option<PixelRect>,
    /// Where the part sits in the leaf.
    pub place: UnitRect,
    /// The part is a spread turned a quarter to fill a one-page screen.
    pub rotate: bool,
}

/// What one screen shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Leaf {
    pub parts: Vec<LeafPart>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LayoutOptions {
    pub screen: Screen,
    pub direction: Direction,
    pub spread_on_single: SpreadOnSingle,
    /// With no page marked as the cover, the first shown page is taken as one.
    pub first_is_cover: bool,
}

impl Default for LayoutOptions {
    fn default() -> Self {
        LayoutOptions {
            screen: Screen::Single,
            direction: Direction::LeftToRight,
            spread_on_single: SpreadOnSingle::Split,
            first_is_cover: true,
        }
    }
}

/// Lays the pages out into leaves, in reading order.
pub fn lay_out(pages: &[PageShape], options: LayoutOptions) -> Vec<Leaf> {
    let marked_cover = pages.iter().any(|p| p.kind == PageKind::Cover);
    let first_shown = pages.iter().position(|p| p.kind != PageKind::Skip);
    let kind_of = |i: usize| {
        let p = &pages[i];
        if p.kind == PageKind::Skip || p.kind == PageKind::Cover {
            p.kind
        } else if !marked_cover && options.first_is_cover && Some(i) == first_shown {
            PageKind::Cover
        } else if is_spread(p.width, p.height) {
            PageKind::Spread
        } else {
            PageKind::Story
        }
    };

    let mut leaves = Vec::new();
    let mut i = 0;
    while i < pages.len() {
        let kind = kind_of(i);
        let page = i as u32;
        i += 1;
        let alone = |part: LeafPart| Leaf { parts: vec![part] };
        let whole =
            |kind, rotate| LeafPart { page, kind, source: None, place: UnitRect::FULL, rotate };
        match (kind, options.screen) {
            (PageKind::Skip, _) => {}
            (PageKind::Spread, Screen::Single) => match options.spread_on_single {
                SpreadOnSingle::Split => {
                    let p = &pages[page as usize];
                    let left = p.width / 2;
                    let halves = [
                        PixelRect::new(0, 0, left, p.height),
                        PixelRect::new(left, 0, p.width - left, p.height),
                    ];
                    let order = match options.direction {
                        Direction::LeftToRight => halves,
                        Direction::RightToLeft => [halves[1], halves[0]],
                    };
                    for half in order {
                        leaves.push(alone(LeafPart {
                            source: Some(half),
                            ..whole(PageKind::Spread, false)
                        }));
                    }
                }
                SpreadOnSingle::Rotate => leaves.push(alone(whole(PageKind::Spread, true))),
                SpreadOnSingle::Whole => leaves.push(alone(whole(PageKind::Spread, false))),
            },
            (PageKind::Story, Screen::Double) => {
                let next = (i < pages.len() && kind_of(i) == PageKind::Story).then_some(i);
                match next.filter(|&n| {
                    (pages[n].aspect() - pages[page as usize].aspect()).abs() <= PAIR_TOLERANCE
                }) {
                    Some(n) => {
                        i += 1;
                        leaves.push(pair(
                            page,
                            &pages[page as usize],
                            n as u32,
                            &pages[n],
                            options.direction,
                        ));
                    }
                    None => leaves.push(alone(whole(PageKind::Story, false))),
                }
            }
            (kind, _) => leaves.push(alone(whole(kind, false))),
        }
    }
    leaves
}

/// Two pages side by side at one height, the first in reading order on the reading side. Both
/// aspects are finite and above 0, so the shares are finite and between 0 and 1.
fn pair(a: u32, sa: &PageShape, b: u32, sb: &PageShape, direction: Direction) -> Leaf {
    let (wa, wb) = (sa.aspect(), sb.aspect());
    let share_a = wa / (wa + wb);
    let part = |page, x, width| LeafPart {
        page,
        kind: PageKind::Story,
        source: None,
        place: UnitRect { x, y: 0.0, width, height: 1.0 },
        rotate: false,
    };
    let parts = match direction {
        Direction::LeftToRight => vec![part(a, 0.0, share_a), part(b, share_a, 1.0 - share_a)],
        Direction::RightToLeft => {
            vec![part(a, 1.0 - share_a, share_a), part(b, 0.0, 1.0 - share_a)]
        }
    };
    Leaf { parts }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: PageShape = PageShape { width: 1000, height: 1400, kind: PageKind::Story };
    const SPREAD: PageShape = PageShape { width: 2000, height: 1400, kind: PageKind::Story };

    fn pages_of(leaves: &[Leaf]) -> Vec<Vec<u32>> {
        leaves.iter().map(|l| l.parts.iter().map(|p| p.page).collect()).collect()
    }

    #[test]
    fn spread_threshold() {
        assert!(!is_spread(1160, 1000));
        assert!(is_spread(1161, 1000));
        assert!(!is_spread(10, 0));
    }

    #[test]
    fn single_screen_splits_spreads_in_reading_order() {
        let pages = [PAGE, SPREAD, PAGE];
        let rtl =
            LayoutOptions { direction: Direction::RightToLeft, ..LayoutOptions::default() };
        let leaves = lay_out(&pages, rtl);
        assert_eq!(pages_of(&leaves), vec![vec![0], vec![1], vec![1], vec![2]]);
        assert_eq!(leaves[0].parts[0].kind, PageKind::Cover);
        assert_eq!(leaves[1].parts[0].source, Some(PixelRect::new(1000, 0, 1000, 1400)));
        assert_eq!(leaves[2].parts[0].source, Some(PixelRect::new(0, 0, 1000, 1400)));

        let ltr = lay_out(&pages, LayoutOptions::default());
        assert_eq!(ltr[1].parts[0].source, Some(PixelRect::new(0, 0, 1000, 1400)));
    }

    #[test]
    fn single_screen_rotates_or_keeps_spreads_whole_as_set() {
        let pages = [PAGE, SPREAD];
        for (mode, rotate) in [(SpreadOnSingle::Rotate, true), (SpreadOnSingle::Whole, false)] {
            let o = LayoutOptions { spread_on_single: mode, ..LayoutOptions::default() };
            let leaves = lay_out(&pages, o);
            assert_eq!(leaves.len(), 2);
            assert_eq!(leaves[1].parts[0].rotate, rotate);
            assert_eq!(leaves[1].parts[0].source, None);
        }
    }

    #[test]
    fn double_screen_pairs_close_aspects_with_the_cover_and_spreads_alone() {
        let narrow = PageShape::story(800, 1400); // 0.57 against 0.71: too far to pair
        let pages = [PAGE, PAGE, PAGE, SPREAD, PAGE, narrow, PAGE, PAGE];
        let o = LayoutOptions { screen: Screen::Double, ..LayoutOptions::default() };
        let leaves = lay_out(&pages, o);
        assert_eq!(
            pages_of(&leaves),
            vec![vec![0], vec![1, 2], vec![3], vec![4], vec![5], vec![6, 7]]
        );
        assert_eq!(leaves[2].parts[0].kind, PageKind::Spread);
        let pair = &leaves[1].parts;
        assert_eq!((pair[0].place.x, pair[0].place.width), (0.0, 0.5));
        assert_eq!((pair[1].place.x, pair[1].place.width), (0.5, 0.5));
    }

    #[test]
    fn right_to_left_pairs_put_the_first_page_on_the_right() {
        let o = LayoutOptions {
            screen: Screen::Double,
            direction: Direction::RightToLeft,
            first_is_cover: false,
            ..LayoutOptions::default()
        };
        let leaves = lay_out(&[PAGE, PAGE], o);
        assert_eq!(leaves.len(), 1);
        let p = &leaves[0].parts;
        assert_eq!((p[0].page, p[0].place.x), (0, 0.5));
        assert_eq!((p[1].page, p[1].place.x), (1, 0.0));
    }

    #[test]
    fn pages_of_unknown_size_pair_as_usual_pages() {
        let unknown = PageShape::story(0, 0);
        let o = LayoutOptions {
            screen: Screen::Double,
            first_is_cover: false,
            ..LayoutOptions::default()
        };
        for pages in [[unknown, unknown], [unknown, PAGE], [PageShape::story(700, 0), PAGE]] {
            let leaves = lay_out(&pages, o);
            assert_eq!(pages_of(&leaves), vec![vec![0, 1]], "{pages:?}");
            for p in &leaves[0].parts {
                assert!(p.place.width > 0.0 && p.place.width < 1.0, "{pages:?}: {p:?}");
            }
        }
    }

    #[test]
    fn skipped_pages_vanish_and_a_marked_cover_wins_over_the_first() {
        let skip = PageShape { kind: PageKind::Skip, ..PAGE };
        let cover = PageShape { kind: PageKind::Cover, ..PAGE };
        let o = LayoutOptions { screen: Screen::Double, ..LayoutOptions::default() };
        let leaves = lay_out(&[skip, PAGE, cover, PAGE, PAGE], o);
        assert_eq!(pages_of(&leaves), vec![vec![1], vec![2], vec![3, 4]]);
        assert_eq!(leaves[0].parts[0].kind, PageKind::Story);
        assert_eq!(leaves[1].parts[0].kind, PageKind::Cover);
    }
}

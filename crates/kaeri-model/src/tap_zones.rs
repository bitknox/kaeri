//! Tap zones: what a tap or a click on a reader page asks for.
//!
//! One model for every front end. A layout is a grid of three columns by three rows, each cell
//! one of three logical zones: [`TapAction::Next`], [`TapAction::Previous`] and
//! [`TapAction::Controls`]. Layouts are drawn for a book read left to right; Next always sits on
//! the side the book reads towards, so a book read right to left mirrors the columns by itself.
//! Pages that move downwards (vertical paging, strips, scrolled novels) never mirror for the
//! direction. The two flips are for hands only: [`TapZoneSettings::flip_sides`] mirrors the
//! columns after the direction, [`TapZoneSettings::flip_ends`] the rows.
//!
//! The outer bands are a third of the screen, or a quarter at [`ZoneSize::Small`]. On e-ink,
//! Sides at Standard keeps the approved 40/30/30 split, wider on the side Next is on.
//!
//! Paged reading (paged manga, vertical paging, paged novels) and scrolling (strips, scrolled
//! novels) each have their own choice. E-ink has one choice for all reading. While a screen
//! reader runs, or a page is zoomed, the zones are off and every tap asks for the controls.
//!
//! A front end asks [`tap_action`] which action a point asks for, and draws the guide and the
//! setting's tiles from [`tap_map`]'s cells, so a layout looks the same on every screen.

use crate::reader::{ReadDirection, ReaderMode};
use kaeri_i18n::t;

/// How far a tap in a scrolling zone moves: 90% of the visible height, the step of Space and
/// Page Down, so a line of the last view stays in sight.
pub const SCROLL_STEP: f64 = 0.9;

/// After a strip comes to rest by itself, how long a tap still counts as the one that stopped
/// it, in milliseconds. While the strip moves, a touch only stops it.
pub const SETTLE_MS: u32 = 120;

/// A tap zone layout, as the person chooses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ZoneLayout {
    /// Sides when pages turn sideways, L shape when they move downwards. The default for
    /// paged reading on a phone or a tablet.
    #[default]
    Auto,
    /// Previous page on one side, next page on the other, controls in the middle column.
    Sides,
    /// Previous page along the top and down the trailing side, next page along the bottom and
    /// on the reading side.
    LShape,
    /// Controls along the top, a narrow column for previous page, next page everywhere else.
    WideNext,
    /// Previous page across the top, next page across the bottom, controls between.
    TopBottom,
    /// Next page down both sides, previous page in the bottom middle.
    BothNext,
    /// Taps only show and hide the controls.
    Off,
}

impl ZoneLayout {
    /// The choices for paged reading, in the order the setting shows them.
    pub const PAGED: &[Self] = &[
        Self::Auto,
        Self::Sides,
        Self::LShape,
        Self::WideNext,
        Self::TopBottom,
        Self::BothNext,
        Self::Off,
    ];

    /// The choices for scrolling, Off first since it is the default.
    pub const SCROLLING: &[Self] = &[
        Self::Off,
        Self::Sides,
        Self::LShape,
        Self::WideNext,
        Self::TopBottom,
        Self::BothNext,
    ];

    /// The value kept in the config.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Sides => "sides",
            Self::LShape => "l_shape",
            Self::WideNext => "wide_next",
            Self::TopBottom => "top_bottom",
            Self::BothNext => "both_next",
            Self::Off => "off",
        }
    }

    /// A kept value. The choices of the one setting before tap zones, Edges and Halves, read as
    /// Sides.
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "edges" | "halves" => Some(Self::Sides),
            _ => Self::PAGED.iter().copied().find(|l| l.name() == name),
        }
    }

    /// The value of the one setting before tap zones (`tap_layout`): Off stays Off, and
    /// everything else, Edges and Halves (kept as `sides`) included, becomes Sides.
    pub fn from_earlier(name: &str) -> Self {
        if name == "off" { Self::Off } else { Self::Sides }
    }

    /// The layout's name, for its tile and its radio row.
    pub fn label(self) -> String {
        match self {
            Self::Auto => t::reader_tap_auto(),
            Self::Sides => t::reader_tap_sides(),
            Self::LShape => t::reader_tap_l_shape(),
            Self::WideNext => t::reader_tap_wide_next(),
            Self::TopBottom => t::reader_tap_top_bottom(),
            Self::BothNext => t::reader_tap_both_next(),
            Self::Off => t::reader_tap_off(),
        }
    }

    /// Auto made concrete for pages that move as `flow` says. Every other layout is itself.
    pub const fn resolve(self, flow: TapFlow) -> Self {
        match (self, flow) {
            (Self::Auto, TapFlow::Sideways) => Self::Sides,
            (Self::Auto, _) => Self::LShape,
            (other, _) => other,
        }
    }

    /// The grid drawn for a book read left to right, row by row from the top. Auto is drawn
    /// as Sides; resolve it first.
    const fn grid(self) -> Grid {
        use TapAction::{Controls as C, Next as N, Previous as P};
        match self {
            Self::Auto | Self::Sides => [[P, C, N], [P, C, N], [P, C, N]],
            Self::LShape => [[P, P, P], [P, C, N], [N, N, N]],
            Self::WideNext => [[C, C, C], [P, N, N], [P, N, N]],
            Self::TopBottom => [[P, P, P], [C, C, C], [N, N, N]],
            Self::BothNext => [[N, C, N], [N, C, N], [N, P, N]],
            Self::Off => [[C, C, C], [C, C, C], [C, C, C]],
        }
    }
}

/// How wide the outer bands are.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum ZoneSize {
    /// A third of the screen.
    #[default]
    Standard,
    /// A quarter, which leaves half the screen for the controls in Sides.
    Small,
}

impl ZoneSize {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Small => "small",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        [Self::Standard, Self::Small].into_iter().find(|s| s.name() == name)
    }

    /// The outer band, as a fraction of the screen.
    pub const fn band(self) -> f64 {
        match self {
            Self::Standard => 1.0 / 3.0,
            Self::Small => 0.25,
        }
    }
}

/// What a tap asks for. In scrolling, Next scrolls down by [`SCROLL_STEP`] and Previous up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TapAction {
    Next,
    Previous,
    /// Show or hide the controls.
    Controls,
}

/// Which way the open reading moves, which decides the mirroring, the arrows and Auto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TapFlow {
    /// Paged, turning left or right.
    Sideways,
    /// Vertical paging: pages turn downwards.
    Downward,
    /// A strip or a scrolled novel.
    Scrolling,
}

/// The way an arrow on a zone points: the way the page moves, whatever the flips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum TapArrow {
    Left,
    Right,
    Up,
    Down,
}

/// What is open, and how: what the zones are worked out for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TapReading {
    pub mode: ReaderMode,
    /// The book's direction: the session's, or a novel's progression.
    pub direction: ReadDirection,
    /// The page is zoomed in: one finger pans and a tap shows the controls.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub zoomed: bool,
    /// VoiceOver, TalkBack, Narrator or Orca runs: the zones are off, and the page offers its
    /// moves as actions instead.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub screen_reader: bool,
}

impl TapReading {
    pub const fn new(mode: ReaderMode, direction: ReadDirection) -> Self {
        Self { mode, direction, zoomed: false, screen_reader: false }
    }

    pub const fn flow(&self) -> TapFlow {
        match self.mode {
            ReaderMode::Paged if matches!(self.direction, ReadDirection::TopToBottom) => {
                TapFlow::Downward
            }
            ReaderMode::Paged => TapFlow::Sideways,
            ReaderMode::Vertical => TapFlow::Downward,
            ReaderMode::Strip | ReaderMode::Scrolled => TapFlow::Scrolling,
        }
    }

    const fn right_to_left(&self) -> bool {
        matches!(self.direction, ReadDirection::RightToLeft)
    }

    /// The zones are off: every tap asks for the controls and no guide shows.
    const fn zones_off(&self) -> bool {
        self.zoomed || self.screen_reader
    }
}

/// The person's tap zone settings on this device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TapZoneSettings {
    /// The layout for paged reading. On e-ink, the one layout for all reading.
    pub paged: ZoneLayout,
    /// The layout for strips and scrolled novels. Not offered on e-ink.
    pub scrolling: ZoneLayout,
    pub size: ZoneSize,
    /// Flip left and right: mirrors the columns after the reading direction is applied.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub flip_sides: bool,
    /// Flip top and bottom: mirrors the rows.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub flip_ends: bool,
    /// The screen is e-ink: one choice for all reading, and Sides at Standard is 40/30/30.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub eink: bool,
    /// E-ink: swipes turn pages. Off leaves taps, page buttons and keys.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub swipes_turn: bool,
    /// The guide last seen in paged reading, as [`TapMap::guide`] named it; empty for never.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub guide_seen_paged: String,
    /// The guide last seen in scrolling.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub guide_seen_scrolling: String,
}

impl Default for TapZoneSettings {
    /// A phone's or a tablet's: Auto for paged reading, Off for scrolling, thirds, no flips.
    fn default() -> Self {
        Self {
            paged: ZoneLayout::Auto,
            scrolling: ZoneLayout::Off,
            size: ZoneSize::Standard,
            flip_sides: false,
            flip_ends: false,
            eink: false,
            swipes_turn: true,
            guide_seen_paged: String::new(),
            guide_seen_scrolling: String::new(),
        }
    }
}

/// One of the nine cells of a layout, in fractions of the screen.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TapCell {
    /// 0 to 2 from the top.
    pub row: u32,
    /// 0 to 2 from the left.
    pub column: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub action: TapAction,
    /// The zone's name: Next page, Previous page, Scroll down, Scroll up or Controls.
    pub name: String,
    /// The way the page moves, on a Next or Previous cell.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub arrow: Option<TapArrow>,
    /// The cell that carries the zone's name and arrow in the guide and on the tile: the
    /// largest of its kind, and in Next on both sides, both sides.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub labelled: bool,
}

/// A layout worked out for what is open: the guide, or one tile of the setting.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct TapMap {
    /// The layout as chosen, Auto included.
    pub choice: ZoneLayout,
    /// The layout drawn: Auto made concrete.
    pub layout: ZoneLayout,
    pub flow: TapFlow,
    /// Nine cells, row by row from the top.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub cells: Vec<TapCell>,
    /// The layout in words for the open book, the tile's accessibility value: "Next page on
    /// the left side, previous page on the right side, controls in the middle".
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub about: String,
    /// What the guide shows, as one value: send it back with `ReaderCommand::TapGuideSeen`
    /// once the guide was seen. It changes with the layout, the size, the flips, the flow and,
    /// for sideways pages, the direction, so the guide shows again after any of them changes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub guide: String,
}

type Grid = [[TapAction; 3]; 3];

/// A layout's cells and band edges, as drawn.
struct Geometry {
    layout: ZoneLayout,
    grid: Grid,
    xs: [f64; 4],
    ys: [f64; 4],
}

impl Geometry {
    fn at(&self, x: f64, y: f64) -> TapAction {
        let band = |edges: &[f64; 4], v: f64| {
            if v < edges[1] {
                0
            } else if v < edges[2] {
                1
            } else {
                2
            }
        };
        self.grid[band(&self.ys, y)][band(&self.xs, x)]
    }
}

/// Whether the guide belongs to paged reading or to scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuideKind {
    Paged,
    Scrolling,
}

impl GuideKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Paged => "paged",
            Self::Scrolling => "scrolling",
        }
    }

    /// The kind a [`TapMap::guide`] value names.
    pub fn of(guide: &str) -> Option<Self> {
        match guide.split(':').next()? {
            "paged" => Some(Self::Paged),
            "scrolling" => Some(Self::Scrolling),
            _ => None,
        }
    }
}

impl TapZoneSettings {
    /// Which of the two choices `reading` follows. E-ink has one choice for all reading.
    pub const fn kind(&self, reading: &TapReading) -> GuideKind {
        if !self.eink && matches!(reading.flow(), TapFlow::Scrolling) {
            GuideKind::Scrolling
        } else {
            GuideKind::Paged
        }
    }

    /// The layout chosen for `reading`, Auto included.
    pub const fn choice(&self, reading: &TapReading) -> ZoneLayout {
        match self.kind(reading) {
            GuideKind::Paged => self.paged,
            GuideKind::Scrolling => self.scrolling,
        }
    }

    fn geometry(&self, reading: &TapReading, choice: ZoneLayout) -> Geometry {
        let flow = reading.flow();
        let layout = choice.resolve(flow);
        let mut grid = layout.grid();
        let mirror =
            (reading.right_to_left() && matches!(flow, TapFlow::Sideways)) != self.flip_sides;
        if mirror {
            for row in &mut grid {
                row.reverse();
            }
        }
        if self.flip_ends {
            grid.reverse();
        }
        let band = self.size.band();
        let mut columns = [band, 1.0 - 2.0 * band, band];
        let rows = columns;
        if self.eink && layout == ZoneLayout::Sides && self.size == ZoneSize::Standard {
            columns =
                if grid[1][0] == TapAction::Next { [0.4, 0.3, 0.3] } else { [0.3, 0.3, 0.4] };
        }
        let edges = |w: [f64; 3]| [0.0, w[0], w[0] + w[1], 1.0];
        Geometry { layout, grid, xs: edges(columns), ys: edges(rows) }
    }

    /// The action a tap at (`x`, `y`) asks for, each a fraction of the reader's width and
    /// height from its top left corner. A zoomed page and a running screen reader turn the
    /// zones off.
    pub fn action(&self, reading: &TapReading, x: f64, y: f64) -> TapAction {
        if reading.zones_off() || x.is_nan() || y.is_nan() {
            return TapAction::Controls;
        }
        self.geometry(reading, self.choice(reading)).at(x.clamp(0.0, 1.0), y.clamp(0.0, 1.0))
    }

    /// The layout in effect for `reading`, or `layout` in its place (for the setting's
    /// tiles), worked out for the open book.
    pub fn map(&self, reading: &TapReading, layout: Option<ZoneLayout>) -> TapMap {
        let choice = layout.unwrap_or_else(|| self.choice(reading));
        let geometry = self.geometry(reading, choice);
        let flow = reading.flow();
        let scrolling = flow == TapFlow::Scrolling;
        let arrows = match flow {
            TapFlow::Sideways if reading.right_to_left() => (TapArrow::Left, TapArrow::Right),
            TapFlow::Sideways => (TapArrow::Right, TapArrow::Left),
            _ => (TapArrow::Down, TapArrow::Up),
        };
        let mut cells = Vec::with_capacity(9);
        for (row, actions) in geometry.grid.iter().enumerate() {
            for (column, &action) in actions.iter().enumerate() {
                let (name, arrow) = match action {
                    TapAction::Next if scrolling => {
                        (t::reader_zone_scroll_down(), Some(arrows.0))
                    }
                    TapAction::Next => (t::reader_zone_next(), Some(arrows.0)),
                    TapAction::Previous if scrolling => {
                        (t::reader_zone_scroll_up(), Some(arrows.1))
                    }
                    TapAction::Previous => (t::reader_zone_previous(), Some(arrows.1)),
                    TapAction::Controls => (t::reader_zone_controls(), None),
                };
                cells.push(TapCell {
                    row: row as u32,
                    column: column as u32,
                    x: geometry.xs[column],
                    y: geometry.ys[row],
                    width: geometry.xs[column + 1] - geometry.xs[column],
                    height: geometry.ys[row + 1] - geometry.ys[row],
                    action,
                    name,
                    arrow,
                    labelled: false,
                });
            }
        }
        label(&mut cells, geometry.layout);
        TapMap {
            choice,
            layout: geometry.layout,
            flow,
            about: about(geometry.layout, &geometry.grid, scrolling),
            guide: self.guide_key(reading, geometry.layout),
            cells,
        }
    }

    fn guide_key(&self, reading: &TapReading, layout: ZoneLayout) -> String {
        let flips = match (self.flip_sides, self.flip_ends) {
            (false, false) => "none",
            (true, false) => "sides",
            (false, true) => "ends",
            (true, true) => "both",
        };
        let flow = match reading.flow() {
            TapFlow::Sideways if reading.right_to_left() => "rtl",
            TapFlow::Sideways => "ltr",
            TapFlow::Downward => "down",
            TapFlow::Scrolling => "scroll",
        };
        format!(
            "{}:{}:{}:{flips}:{flow}",
            self.kind(reading).name(),
            layout.name(),
            self.size.name()
        )
    }

    /// Whether the guide shows when the reader opens on `reading`: the first time on this
    /// device for paged reading and for scrolling, and again after the layout, the size, the
    /// flips or the direction changed. Never for Off, over a zoomed page or while a screen
    /// reader runs. Show zones in the setting shows it whatever this says.
    pub fn guide_due(&self, reading: &TapReading) -> bool {
        if reading.zones_off() {
            return false;
        }
        let layout = self.choice(reading).resolve(reading.flow());
        if layout == ZoneLayout::Off {
            return false;
        }
        let seen = match self.kind(reading) {
            GuideKind::Paged => &self.guide_seen_paged,
            GuideKind::Scrolling => &self.guide_seen_scrolling,
        };
        *seen != self.guide_key(reading, layout)
    }
}

/// Marks the cells that carry a name: the largest of each kind, the middle row first and then
/// the middle column on a tie, and in Next on both sides the Next cell across from it too.
fn label(cells: &mut [TapCell], layout: ZoneLayout) {
    let score = |c: &TapCell| {
        (c.width * c.height * 100.0).round()
            + if c.row == 1 { 0.5 } else { 0.0 }
            + if c.column == 1 { 0.25 } else { 0.0 }
    };
    let mut best: [Option<usize>; 3] = [None; 3];
    for (i, cell) in cells.iter().enumerate() {
        let slot = &mut best[cell.action as usize];
        if slot.is_none_or(|b| score(cell) > score(&cells[b]) + 0.01) {
            *slot = Some(i);
        }
    }
    for i in best.into_iter().flatten() {
        cells[i].labelled = true;
    }
    if layout == ZoneLayout::BothNext
        && let Some(next) = best[TapAction::Next as usize]
    {
        let row = cells[next].row;
        for cell in cells.iter_mut() {
            if cell.action == TapAction::Next && cell.row == row {
                cell.labelled = true;
            }
        }
    }
}

/// The layout in words, for the grid as drawn.
fn about(layout: ZoneLayout, grid: &Grid, scrolling: bool) -> String {
    use TapAction::{Controls as C, Next as N, Previous as P};
    let next_left = grid[1][0] == N;
    let next_low = grid[2][1] == N;
    match (layout, scrolling) {
        (ZoneLayout::Auto | ZoneLayout::Sides, false) if next_left => {
            t::reader_tap_sides_left_about()
        }
        (ZoneLayout::Auto | ZoneLayout::Sides, false) => t::reader_tap_sides_right_about(),
        (ZoneLayout::Auto | ZoneLayout::Sides, true) if next_left => {
            t::reader_tap_sides_left_scroll_about()
        }
        (ZoneLayout::Auto | ZoneLayout::Sides, true) => {
            t::reader_tap_sides_right_scroll_about()
        }
        (ZoneLayout::LShape, false) => match (next_low, next_left) {
            (true, false) => t::reader_tap_l_shape_bottom_right_about(),
            (true, true) => t::reader_tap_l_shape_bottom_left_about(),
            (false, false) => t::reader_tap_l_shape_top_right_about(),
            (false, true) => t::reader_tap_l_shape_top_left_about(),
        },
        (ZoneLayout::LShape, true) => match (next_low, next_left) {
            (true, false) => t::reader_tap_l_shape_bottom_right_scroll_about(),
            (true, true) => t::reader_tap_l_shape_bottom_left_scroll_about(),
            (false, false) => t::reader_tap_l_shape_top_right_scroll_about(),
            (false, true) => t::reader_tap_l_shape_top_left_scroll_about(),
        },
        (ZoneLayout::WideNext, _) => {
            let controls_top = grid[0][1] == C;
            let previous_left = grid[1][0] == P;
            match (controls_top, previous_left, scrolling) {
                (true, true, false) => t::reader_tap_wide_next_top_left_about(),
                (true, false, false) => t::reader_tap_wide_next_top_right_about(),
                (false, true, false) => t::reader_tap_wide_next_bottom_left_about(),
                (false, false, false) => t::reader_tap_wide_next_bottom_right_about(),
                (true, true, true) => t::reader_tap_wide_next_top_left_scroll_about(),
                (true, false, true) => t::reader_tap_wide_next_top_right_scroll_about(),
                (false, true, true) => t::reader_tap_wide_next_bottom_left_scroll_about(),
                (false, false, true) => t::reader_tap_wide_next_bottom_right_scroll_about(),
            }
        }
        (ZoneLayout::TopBottom, false) if next_low => t::reader_tap_top_bottom_about(),
        (ZoneLayout::TopBottom, false) => t::reader_tap_top_bottom_flipped_about(),
        (ZoneLayout::TopBottom, true) if next_low => t::reader_tap_top_bottom_scroll_about(),
        (ZoneLayout::TopBottom, true) => t::reader_tap_top_bottom_flipped_scroll_about(),
        (ZoneLayout::BothNext, false) if grid[2][1] == P => t::reader_tap_both_next_about(),
        (ZoneLayout::BothNext, false) => t::reader_tap_both_next_flipped_about(),
        (ZoneLayout::BothNext, true) if grid[2][1] == P => {
            t::reader_tap_both_next_scroll_about()
        }
        (ZoneLayout::BothNext, true) => t::reader_tap_both_next_flipped_scroll_about(),
        (ZoneLayout::Off, _) => t::reader_tap_off_about(),
    }
}

/// [`TapZoneSettings::action`], for a front end: the action a tap at (`x`, `y`) asks for.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn tap_action(settings: TapZoneSettings, reading: TapReading, x: f64, y: f64) -> TapAction {
    settings.action(&reading, x, y)
}

/// [`TapZoneSettings::map`], for a front end: the cells of the layout in effect, or of
/// `layout` for one of the setting's tiles.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn tap_map(
    settings: TapZoneSettings,
    reading: TapReading,
    layout: Option<ZoneLayout>,
) -> TapMap {
    settings.map(&reading, layout)
}

/// [`TapZoneSettings::guide_due`], for a front end.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub fn tap_guide_due(settings: TapZoneSettings, reading: TapReading) -> bool {
    settings.guide_due(&reading)
}

/// [`SCROLL_STEP`], for a front end.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub const fn tap_scroll_step() -> f64 {
    SCROLL_STEP
}

/// [`SETTLE_MS`], for a front end.
#[cfg_attr(feature = "ffi", uniffi::export)]
pub const fn tap_settle_ms() -> u32 {
    SETTLE_MS
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The board's layouts, drawn left to right: P previous, C controls, N next.
    fn drawn(layout: ZoneLayout) -> [&'static str; 3] {
        match layout {
            ZoneLayout::Auto => unreachable!("resolved first"),
            ZoneLayout::Sides => ["PCN", "PCN", "PCN"],
            ZoneLayout::LShape => ["PPP", "PCN", "NNN"],
            ZoneLayout::WideNext => ["CCC", "PNN", "PNN"],
            ZoneLayout::TopBottom => ["PPP", "CCC", "NNN"],
            ZoneLayout::BothNext => ["NCN", "NCN", "NPN"],
            ZoneLayout::Off => ["CCC", "CCC", "CCC"],
        }
    }

    fn letter(action: TapAction) -> char {
        match action {
            TapAction::Next => 'N',
            TapAction::Previous => 'P',
            TapAction::Controls => 'C',
        }
    }

    const READINGS: [(ReaderMode, ReadDirection); 7] = [
        (ReaderMode::Paged, ReadDirection::LeftToRight),
        (ReaderMode::Paged, ReadDirection::RightToLeft),
        (ReaderMode::Paged, ReadDirection::TopToBottom),
        (ReaderMode::Vertical, ReadDirection::RightToLeft),
        (ReaderMode::Strip, ReadDirection::RightToLeft),
        (ReaderMode::Strip, ReadDirection::LeftToRight),
        (ReaderMode::Scrolled, ReadDirection::RightToLeft),
    ];

    /// Every combination of what a hit test depends on.
    fn every() -> Vec<(ZoneLayout, TapReading, TapZoneSettings)> {
        let mut out = Vec::new();
        for &layout in ZoneLayout::PAGED {
            for (mode, direction) in READINGS {
                for flip_sides in [false, true] {
                    for flip_ends in [false, true] {
                        for size in [ZoneSize::Standard, ZoneSize::Small] {
                            for eink in [false, true] {
                                let settings = TapZoneSettings {
                                    paged: layout,
                                    scrolling: layout,
                                    size,
                                    flip_sides,
                                    flip_ends,
                                    eink,
                                    ..TapZoneSettings::default()
                                };
                                out.push((layout, TapReading::new(mode, direction), settings));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// The board's rules, written out apart from the engine's: Auto is Sides sideways and L
    /// shape otherwise; columns mirror for right to left only on sideways pages, and again for
    /// Flip left and right; rows mirror for Flip top and bottom; bands are a third or a quarter,
    /// and e-ink Sides at Standard is 40/30/30, wider on Next's side.
    fn expected(
        layout: ZoneLayout,
        reading: &TapReading,
        s: &TapZoneSettings,
    ) -> ([[char; 3]; 3], [f64; 4], [f64; 4]) {
        let sideways = reading.mode == ReaderMode::Paged
            && reading.direction != ReadDirection::TopToBottom;
        let layout = match layout {
            ZoneLayout::Auto if sideways => ZoneLayout::Sides,
            ZoneLayout::Auto => ZoneLayout::LShape,
            other => other,
        };
        let mut grid = drawn(layout).map(|row| {
            let c: Vec<char> = row.chars().collect();
            [c[0], c[1], c[2]]
        });
        let rtl = reading.direction == ReadDirection::RightToLeft;
        if (rtl && sideways) ^ s.flip_sides {
            grid.iter_mut().for_each(|r| r.reverse());
        }
        if s.flip_ends {
            grid.reverse();
        }
        let b = if s.size == ZoneSize::Small { 0.25 } else { 1.0 / 3.0 };
        let ys = [0.0, b, 1.0 - b, 1.0];
        let xs = if s.eink && layout == ZoneLayout::Sides && s.size == ZoneSize::Standard {
            if grid[1][0] == 'N' { [0.0, 0.4, 0.7, 1.0] } else { [0.0, 0.3, 0.6, 1.0] }
        } else {
            ys
        };
        (grid, xs, ys)
    }

    #[test]
    fn every_layout_direction_flip_and_size_hits_the_boards_grid() {
        let combos = every();
        assert_eq!(combos.len(), 7 * 7 * 2 * 2 * 2 * 2);
        for (layout, reading, settings) in combos {
            let (grid, xs, ys) = expected(layout, &reading, &settings);
            let say = format!("{layout:?} {reading:?} {settings:?}");
            for r in 0..3 {
                for c in 0..3 {
                    let want = grid[r][c];
                    // The middle of the cell, and just inside each of its corners.
                    let inside = 1e-6;
                    let points = [
                        ((xs[c] + xs[c + 1]) / 2.0, (ys[r] + ys[r + 1]) / 2.0),
                        (xs[c] + inside, ys[r] + inside),
                        (xs[c + 1] - inside, ys[r + 1] - inside),
                    ];
                    for (x, y) in points {
                        let got = letter(settings.action(&reading, x, y));
                        assert_eq!(got, want, "{say} at ({x}, {y})");
                        assert_eq!(letter(tap_action(settings.clone(), reading, x, y)), want);
                    }
                }
            }
            // The cells the guide draws are the cells the hit test uses.
            let map = settings.map(&reading, None);
            assert_eq!(map.cells.len(), 9, "{say}");
            for cell in &map.cells {
                let (r, c) = (cell.row as usize, cell.column as usize);
                assert_eq!(letter(cell.action), grid[r][c], "{say} cell {r},{c}");
                assert!(
                    (cell.x - xs[c]).abs() < 1e-9 && (cell.y - ys[r]).abs() < 1e-9,
                    "{say}"
                );
                assert!((cell.width - (xs[c + 1] - xs[c])).abs() < 1e-9, "{say}");
                assert!((cell.height - (ys[r + 1] - ys[r])).abs() < 1e-9, "{say}");
            }
        }
    }

    #[test]
    fn the_layouts_read_as_the_board_draws_them() {
        let phone = TapZoneSettings::default();
        let rtl = TapReading::new(ReaderMode::Paged, ReadDirection::RightToLeft);
        let ltr = TapReading::new(ReaderMode::Paged, ReadDirection::LeftToRight);
        let strip = TapReading::new(ReaderMode::Strip, ReadDirection::RightToLeft);
        let vertical = TapReading::new(ReaderMode::Vertical, ReadDirection::RightToLeft);

        // Auto is Sides for sideways pages: next page on the side the book reads towards.
        assert_eq!(phone.action(&rtl, 0.1, 0.5), TapAction::Next);
        assert_eq!(phone.action(&rtl, 0.9, 0.5), TapAction::Previous);
        assert_eq!(phone.action(&ltr, 0.9, 0.5), TapAction::Next);
        assert_eq!(phone.action(&ltr, 0.5, 0.5), TapAction::Controls);
        // Auto is L shape for vertical paging, which never mirrors for the direction.
        assert_eq!(phone.action(&vertical, 0.1, 0.9), TapAction::Next);
        assert_eq!(phone.action(&vertical, 0.9, 0.5), TapAction::Next);
        assert_eq!(phone.action(&vertical, 0.1, 0.5), TapAction::Previous);
        // Scrolling is Off by default: a tap anywhere shows the controls.
        for (x, y) in [(0.05, 0.05), (0.5, 0.5), (0.95, 0.95)] {
            assert_eq!(phone.action(&strip, x, y), TapAction::Controls);
        }
        // A scrolling layout of its own, which a strip follows and paged reading does not.
        let scroll = TapZoneSettings { scrolling: ZoneLayout::TopBottom, ..phone.clone() };
        assert_eq!(scroll.action(&strip, 0.5, 0.9), TapAction::Next);
        assert_eq!(scroll.action(&strip, 0.5, 0.1), TapAction::Previous);
        assert_eq!(scroll.action(&rtl, 0.5, 0.9), TapAction::Controls);
        // E-ink has one choice for all reading: a strip follows the paged layout there.
        let eink = TapZoneSettings { paged: ZoneLayout::Sides, eink: true, ..scroll };
        assert_eq!(eink.action(&strip, 0.9, 0.5), TapAction::Next);
        assert_eq!(eink.choice(&strip), ZoneLayout::Sides);
        // E-ink Sides is 40/30/30, wider at the reading side.
        assert_eq!(eink.action(&rtl, 0.39, 0.5), TapAction::Next);
        assert_eq!(eink.action(&rtl, 0.41, 0.5), TapAction::Controls);
        assert_eq!(eink.action(&rtl, 0.71, 0.5), TapAction::Previous);
        // Small: quarters, half the screen for the controls in Sides.
        let small = TapZoneSettings { size: ZoneSize::Small, ..phone.clone() };
        assert_eq!(small.action(&ltr, 0.26, 0.5), TapAction::Controls);
        assert_eq!(small.action(&ltr, 0.76, 0.5), TapAction::Next);
        // Flip left and right is the left-handed switch; with right to left it is drawn as
        // left to right.
        let left_hand = TapZoneSettings { flip_sides: true, ..phone };
        assert_eq!(left_hand.action(&ltr, 0.1, 0.5), TapAction::Next);
        assert_eq!(left_hand.action(&rtl, 0.9, 0.5), TapAction::Next);
        // Points outside the reader count at its edge.
        assert_eq!(left_hand.action(&ltr, -3.0, 0.5), TapAction::Next);
        assert_eq!(left_hand.action(&ltr, f64::NAN, 0.5), TapAction::Controls);
    }

    #[test]
    fn a_zoomed_page_and_a_screen_reader_turn_the_zones_off() {
        let settings = TapZoneSettings::default();
        let open = TapReading::new(ReaderMode::Paged, ReadDirection::LeftToRight);
        assert!(settings.guide_due(&open));
        for reading in
            [TapReading { zoomed: true, ..open }, TapReading { screen_reader: true, ..open }]
        {
            for x in [0.05, 0.5, 0.95] {
                assert_eq!(settings.action(&reading, x, 0.5), TapAction::Controls);
            }
            assert!(!settings.guide_due(&reading), "{reading:?}");
        }
    }

    #[test]
    fn the_guide_shows_until_seen_and_again_after_a_change() {
        let mut s = TapZoneSettings::default();
        let rtl = TapReading::new(ReaderMode::Paged, ReadDirection::RightToLeft);
        let ltr = TapReading::new(ReaderMode::Paged, ReadDirection::LeftToRight);
        let strip = TapReading::new(ReaderMode::Strip, ReadDirection::RightToLeft);
        let strip_ltr = TapReading::new(ReaderMode::Strip, ReadDirection::LeftToRight);

        assert!(s.guide_due(&rtl));
        s.guide_seen_paged = s.map(&rtl, None).guide;
        assert!(!s.guide_due(&rtl));
        assert_eq!(GuideKind::of(&s.guide_seen_paged), Some(GuideKind::Paged));
        // The direction changes what the guide shows.
        assert!(s.guide_due(&ltr));
        // Never for Off: scrolling is Off by default.
        assert!(!s.guide_due(&strip));
        s.scrolling = ZoneLayout::LShape;
        assert!(s.guide_due(&strip), "paged and scrolling are seen apart");
        s.guide_seen_scrolling = s.map(&strip, None).guide;
        assert_eq!(GuideKind::of(&s.guide_seen_scrolling), Some(GuideKind::Scrolling));
        assert!(!s.guide_due(&strip) && !s.guide_due(&rtl));
        // A strip has no direction to show.
        assert!(!s.guide_due(&strip_ltr));
        // The layout, the size and each flip each bring it back.
        for changed in [
            TapZoneSettings { paged: ZoneLayout::TopBottom, ..s.clone() },
            TapZoneSettings { size: ZoneSize::Small, ..s.clone() },
            TapZoneSettings { flip_sides: true, ..s.clone() },
            TapZoneSettings { flip_ends: true, ..s.clone() },
        ] {
            assert!(changed.guide_due(&rtl), "{changed:?}");
        }
        // Auto chosen and Sides chosen draw the same guide.
        let sides = TapZoneSettings { paged: ZoneLayout::Sides, ..s.clone() };
        assert!(!sides.guide_due(&rtl));
        assert_eq!(GuideKind::of("sideways"), None);
    }

    #[test]
    fn each_kind_is_named_once_and_the_arrows_point_the_way_pages_move() {
        let rtl = TapReading::new(ReaderMode::Paged, ReadDirection::RightToLeft);
        let strip = TapReading::new(ReaderMode::Strip, ReadDirection::RightToLeft);
        for (layout, reading, settings) in every() {
            let map = settings.map(&reading, Some(layout));
            let named = |a| map.cells.iter().filter(|c| c.labelled && c.action == a).count();
            let has = |a| map.cells.iter().any(|c| c.action == a);
            let resolved = map.layout;
            let nexts = if resolved == ZoneLayout::BothNext { 2 } else { 1 };
            assert_eq!(named(TapAction::Next), if has(TapAction::Next) { nexts } else { 0 });
            assert_eq!(named(TapAction::Previous), usize::from(has(TapAction::Previous)));
            assert_eq!(named(TapAction::Controls), usize::from(has(TapAction::Controls)));
            for cell in &map.cells {
                assert_eq!(cell.arrow.is_some(), cell.action != TapAction::Controls);
                assert!(!cell.name.is_empty());
            }
            let flow = reading.flow();
            let next = map.cells.iter().find(|c| c.action == TapAction::Next).map(|c| c.arrow);
            let want = match flow {
                TapFlow::Sideways if reading.direction == ReadDirection::RightToLeft => {
                    TapArrow::Left
                }
                TapFlow::Sideways => TapArrow::Right,
                _ => TapArrow::Down,
            };
            if let Some(arrow) = next {
                assert_eq!(arrow, Some(want), "flips never turn the arrows");
            }
            assert_ne!(resolved, ZoneLayout::Auto);
        }
        // The guide names the zones by what they do: pages turn, strips scroll.
        let settings = TapZoneSettings { scrolling: ZoneLayout::Sides, ..Default::default() };
        let names = |reading| -> BTreeSet<String> {
            settings.map(&reading, None).cells.into_iter().map(|c| c.name).collect()
        };
        assert_eq!(
            names(rtl),
            BTreeSet::from(["Next page".into(), "Previous page".into(), "Controls".into()])
        );
        assert_eq!(
            names(strip),
            BTreeSet::from(["Scroll down".into(), "Scroll up".into(), "Controls".into()])
        );
        // In Wide next the name sits in the middle of the large next page area.
        let wide = settings.map(&rtl, Some(ZoneLayout::WideNext));
        let label = wide.cells.iter().find(|c| c.labelled && c.action == TapAction::Next);
        assert_eq!(label.map(|c| (c.row, c.column)), Some((1, 1)));
        // In Next on both sides, both sides are named, in the middle row.
        let both = settings.map(&rtl, Some(ZoneLayout::BothNext));
        let named: Vec<_> = both
            .cells
            .iter()
            .filter(|c| c.labelled && c.action == TapAction::Next)
            .map(|c| (c.row, c.column))
            .collect();
        assert_eq!(named, [(1, 0), (1, 2)]);
    }

    #[test]
    fn every_drawn_variant_has_its_own_words() {
        let mut said = BTreeSet::new();
        for (layout, reading, settings) in every() {
            let map = settings.map(&reading, Some(layout));
            assert!(!map.about.is_empty());
            said.insert(map.about);
        }
        // Sides 2, L shape 4, Wide next 4, Top and bottom 2, Next on both sides 2, each paged
        // and scrolling, and Off.
        assert_eq!(said.len(), 14 * 2 + 1, "{said:#?}");
        let rtl = TapReading::new(ReaderMode::Paged, ReadDirection::RightToLeft);
        assert_eq!(
            TapZoneSettings::default().map(&rtl, None).about,
            "Next page on the left side, previous page on the right side, controls in the middle"
        );
    }

    #[test]
    fn kept_names_read_back_and_the_earlier_choices_become_sides() {
        for &layout in ZoneLayout::PAGED {
            assert_eq!(ZoneLayout::parse(layout.name()), Some(layout));
            assert!(!layout.label().is_empty());
        }
        assert_eq!(ZoneLayout::parse("edges"), Some(ZoneLayout::Sides));
        assert_eq!(ZoneLayout::parse("halves"), Some(ZoneLayout::Sides));
        assert_eq!(ZoneLayout::parse("diagonal"), None);
        for (earlier, now) in [
            ("edges", ZoneLayout::Sides),
            ("sides", ZoneLayout::Sides),
            ("off", ZoneLayout::Off),
            ("anything", ZoneLayout::Sides),
        ] {
            assert_eq!(ZoneLayout::from_earlier(earlier), now, "{earlier}");
        }
        assert!(!ZoneLayout::SCROLLING.contains(&ZoneLayout::Auto));
        assert_eq!(ZoneLayout::SCROLLING[0], ZoneLayout::Off);
        for size in [ZoneSize::Standard, ZoneSize::Small] {
            assert_eq!(ZoneSize::parse(size.name()), Some(size));
        }
        assert_eq!(tap_scroll_step(), 0.9);
        assert_eq!(tap_settle_ms(), 120);
    }
}

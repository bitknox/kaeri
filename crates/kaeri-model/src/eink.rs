//! E-ink screens: what Android says about the device, what the engine makes of it, and the
//! refresh rule each paint follows.
//!
//! The policy crosses once as data and [`eink_paint`] decides each paint on the app's thread, so a
//! page turn or a list page never waits on a round trip through the engine. The app's refresh
//! controller turns a [`Refresh`] into the vendor's call; with no hook it draws normally.
//!
//! Out of the box the device decides: the policy asks for no full refresh, so the panel follows
//! its own configuration (the vendor's per-app mode, the firmware's defaults). Full refreshes are
//! the person's choice in the Refresh rows, and Clean screen asks for one by hand.

/// What Android says about the device and its screen, sent once at start. Named for its
/// screen so the command that carries it keeps the name `DeviceFacts`.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct ScreenFacts {
    pub manufacturer: String,
    pub brand: String,
    pub model: String,
    pub device: String,
    pub product: String,
    pub hardware: String,
    pub sdk: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: u32,
    pub refresh_hz: f32,
    /// `ro.eink.support`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub eink_support: Option<String>,
    /// `ro.onyx.devicename`.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub onyx_name: Option<String>,
    /// `Display.isWideColorGamut()`: colour on the vendor's own devices.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub wide_colour_gamut: bool,
    /// The refresh hooks that resolved: "onyx", "ntx", "rockchip", and "onyx-modes" when the
    /// Onyx calls that set a view's update mode resolved.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub hooks: Vec<String>,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub page_keys: bool,
    /// The system WebView's major version.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub webview: Option<u32>,
}

/// What the engine makes of the device.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EinkProfile {
    pub eink: EinkMatch,
    pub panel: EinkPanel,
    pub ppi: u32,
    /// From the device's identity, not from which hook resolves.
    pub family: VendorFamily,
    /// The family's refresh hook resolved: the Refresh rows and Clean screen show.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub refresh_hook: bool,
    /// The family's own update modes can be set for a view: Screen mode offers [`PANEL_MODES`].
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub panel_modes: bool,
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub page_buttons: bool,
    /// False: pictures stay in colour and the amber accent waits for a yes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub colour_sure: bool,
    pub novels: NovelSupport,
    /// The refresh rule with this device's Refresh settings; `None` on a screen that is not
    /// e-ink. Sent again whenever one of those settings changes.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub policy: Option<EinkPolicy>,
}

/// Whether the screen is e-ink. Unsure asks "Is this an e-ink screen?" first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EinkMatch {
    Sure,
    Unsure,
    No,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EinkPanel {
    Grey,
    Kaleido3,
    Gallery3,
    UnknownColour,
    Lcd,
}

impl EinkPanel {
    pub const fn colour(self) -> bool {
        matches!(self, Self::Kaleido3 | Self::Gallery3 | Self::UnknownColour | Self::Lcd)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum VendorFamily {
    Onyx,
    Rockchip,
    Ntx,
    Other,
}

/// How well the system web view reads novels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum NovelSupport {
    /// WebView 102 and up.
    #[default]
    Full,
    /// 83 to 101: opens, with known flaws.
    KnownFlaws,
    /// Below 83: novels are refused.
    Refused,
}

impl NovelSupport {
    /// For a system WebView of major version `major`.
    pub const fn of_webview(major: u32) -> Self {
        if major >= 102 {
            Self::Full
        } else if major >= 83 {
            Self::KnownFlaws
        } else {
            Self::Refused
        }
    }
}

/// Screen mode's choices where the vendor's update modes can be set: the device's own setting
/// first, then the panel's modes from cleanest to quickest.
pub const PANEL_MODES: &[&str] = &["device", "hd", "regal", "balanced", "fast"];

/// Screen mode's choices on Gallery 3: its colour update speeds.
pub const GALLERY_SPEEDS: &[&str] = &["standard", "fast", "best"];

/// Gallery 3's colour update speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum GallerySpeed {
    #[default]
    Standard,
    Fast,
    Best,
}

/// The refresh settings, crossed once and again when one changes. Each full refresh is opt-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EinkPolicy {
    pub panel: EinkPanel,
    /// A full refresh on every image page. Off by default.
    pub image_full: bool,
    /// A full refresh every this many text pages; 0 never, the default.
    pub text_every: u8,
    /// The same on the Dark look.
    pub text_every_dark: u8,
    /// A full refresh every this many list pages; 0 never, the default.
    pub list_every: u8,
    pub gallery_speed: GallerySpeed,
    /// The look is dark.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub dark: bool,
}

impl EinkPolicy {
    /// The defaults for `panel`: no full refresh, so the device's own configuration decides how
    /// the panel clears.
    pub const fn defaults(panel: EinkPanel) -> Self {
        Self {
            panel,
            image_full: false,
            text_every: 0,
            text_every_dark: 0,
            list_every: 0,
            gallery_speed: GallerySpeed::Standard,
            dark: false,
        }
    }

    /// The person turned on a full refresh for some kind of page. Only then does closing the
    /// chrome or reaching a chapter's end clear the panel too.
    pub const fn any_full(self) -> bool {
        self.image_full
            || self.text_every > 0
            || self.text_every_dark > 0
            || self.list_every > 0
    }
}

/// Partial refreshes since the last full one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EinkCounter {
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub partials: u32,
}

/// What the app is about to paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum EinkPaint {
    ImagePage {
        colour: bool,
    },
    TextPage,
    ListPage {
        covers: bool,
    },
    ChromeOpen,
    ChromeClose,
    SheetChange,
    EndPage,
    Press,
    Waiting,
    /// Hold page down: Clean screen.
    Clean,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum Refresh {
    Full,
    Partial,
    Fast,
    Region,
    ColourStandard,
    ColourFast,
    ColourBest,
    BlackWhite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EinkDecision {
    pub refresh: Refresh,
    pub counter: EinkCounter,
}

/// The refresh for one paint, and the counter to keep for the next.
pub fn eink_paint(policy: EinkPolicy, counter: EinkCounter, paint: EinkPaint) -> EinkDecision {
    let full = || EinkDecision { refresh: Refresh::Full, counter: EinkCounter { partials: 0 } };
    let every = |n: u8, refresh: Refresh| {
        let partials = counter.partials + 1;
        if n > 0 && partials >= u32::from(n) {
            full()
        } else {
            EinkDecision { refresh, counter: EinkCounter { partials } }
        }
    };
    let keep = |refresh| EinkDecision { refresh, counter };
    match paint {
        EinkPaint::ImagePage { colour } if policy.panel == EinkPanel::Gallery3 => {
            let refresh = match (colour, policy.gallery_speed) {
                (false, _) => Refresh::BlackWhite,
                (true, GallerySpeed::Standard) => Refresh::ColourStandard,
                (true, GallerySpeed::Fast) => Refresh::ColourFast,
                (true, GallerySpeed::Best) => Refresh::ColourBest,
            };
            EinkDecision { refresh, counter: EinkCounter { partials: 0 } }
        }
        EinkPaint::ImagePage { .. } if policy.image_full => full(),
        EinkPaint::ImagePage { .. } => every(policy.text_every, Refresh::Partial),
        EinkPaint::TextPage => {
            let n = if policy.dark { policy.text_every_dark } else { policy.text_every };
            every(n, Refresh::Partial)
        }
        EinkPaint::ListPage { covers } => {
            every(policy.list_every, if covers { Refresh::Partial } else { Refresh::Fast })
        }
        EinkPaint::Clean => full(),
        EinkPaint::ChromeClose | EinkPaint::EndPage if policy.any_full() => full(),
        EinkPaint::ChromeClose | EinkPaint::EndPage => keep(Refresh::Partial),
        EinkPaint::SheetChange => keep(Refresh::Region),
        EinkPaint::ChromeOpen | EinkPaint::Press | EinkPaint::Waiting => keep(Refresh::Fast),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(policy: EinkPolicy, paints: &[EinkPaint]) -> Vec<Refresh> {
        let mut counter = EinkCounter::default();
        paints
            .iter()
            .map(|paint| {
                let decision = eink_paint(policy, counter, *paint);
                counter = decision.counter;
                decision.refresh
            })
            .collect()
    }

    /// A person who turned every full refresh on: every image page, every sixth text page
    /// (fourth on Dark), every third list page.
    fn opted_in(panel: EinkPanel) -> EinkPolicy {
        EinkPolicy {
            image_full: true,
            text_every: 6,
            text_every_dark: 4,
            list_every: 3,
            ..EinkPolicy::defaults(panel)
        }
    }

    #[test]
    fn out_of_the_box_the_device_decides_and_only_clean_screen_is_full() {
        for panel in [EinkPanel::Grey, EinkPanel::Kaleido3, EinkPanel::UnknownColour] {
            let policy = EinkPolicy::defaults(panel);
            assert!(!policy.any_full());
            let paints = [
                EinkPaint::ImagePage { colour: true },
                EinkPaint::TextPage,
                EinkPaint::ListPage { covers: true },
                EinkPaint::ListPage { covers: false },
                EinkPaint::ChromeOpen,
                EinkPaint::ChromeClose,
                EinkPaint::EndPage,
                EinkPaint::SheetChange,
            ];
            let many: Vec<EinkPaint> = paints.iter().copied().cycle().take(80).collect();
            assert!(!run(policy, &many).contains(&Refresh::Full), "{panel:?}");
            let dark = EinkPolicy { dark: true, ..policy };
            assert!(!run(dark, &[EinkPaint::TextPage; 20]).contains(&Refresh::Full));
            assert_eq!(run(policy, &[EinkPaint::Clean]), [Refresh::Full]);
        }
    }

    #[test]
    fn text_pages_go_full_every_sixth_and_every_fourth_on_dark_when_asked() {
        let text = [EinkPaint::TextPage; 12];
        let grey = opted_in(EinkPanel::Grey);
        let fulls: Vec<usize> = run(grey, &text)
            .iter()
            .enumerate()
            .filter(|(_, r)| **r == Refresh::Full)
            .map(|(i, _)| i + 1)
            .collect();
        assert_eq!(fulls, [6, 12]);
        let dark = EinkPolicy { dark: true, ..grey };
        let fulls = run(dark, &text).iter().filter(|r| **r == Refresh::Full).count();
        assert_eq!(fulls, 3);
    }

    #[test]
    fn images_chrome_and_lists_follow_the_rows_turned_on() {
        let grey = opted_in(EinkPanel::Grey);
        assert_eq!(
            run(grey, &[EinkPaint::TextPage, EinkPaint::ImagePage { colour: false }]),
            [Refresh::Partial, Refresh::Full]
        );
        assert_eq!(
            run(grey, &[EinkPaint::ChromeOpen, EinkPaint::ChromeClose, EinkPaint::EndPage]),
            [Refresh::Fast, Refresh::Full, Refresh::Full]
        );
        let lists =
            run(opted_in(EinkPanel::Kaleido3), &[EinkPaint::ListPage { covers: true }; 3]);
        assert_eq!(lists, [Refresh::Partial, Refresh::Partial, Refresh::Full]);
        // Only lists turned on: closing the chrome clears too, picture pages stay partial.
        let lists_only = EinkPolicy { list_every: 5, ..EinkPolicy::defaults(EinkPanel::Grey) };
        assert_eq!(
            run(lists_only, &[EinkPaint::ImagePage { colour: false }, EinkPaint::ChromeClose]),
            [Refresh::Partial, Refresh::Full]
        );
        let gallery = EinkPolicy::defaults(EinkPanel::Gallery3);
        assert_eq!(
            run(
                gallery,
                &[
                    EinkPaint::ImagePage { colour: false },
                    EinkPaint::ImagePage { colour: true }
                ]
            ),
            [Refresh::BlackWhite, Refresh::ColourStandard]
        );
    }

    #[test]
    fn webview_versions_map_to_novel_support() {
        assert_eq!(NovelSupport::of_webview(120), NovelSupport::Full);
        assert_eq!(NovelSupport::of_webview(102), NovelSupport::Full);
        assert_eq!(NovelSupport::of_webview(90), NovelSupport::KnownFlaws);
        assert_eq!(NovelSupport::of_webview(82), NovelSupport::Refused);
    }
}

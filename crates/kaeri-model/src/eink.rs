//! E-ink screens: what Android says about the device, what the engine makes of it, and the
//! refresh rule each paint follows.
//!
//! The policy crosses once as data and [`eink_paint`] decides each paint on the app's thread, so a
//! page turn or a list page never waits on a round trip through the engine. The app's refresh
//! controller turns a [`Refresh`] into the vendor's call; with no hook it draws normally.

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
    /// The refresh hooks that resolved: "onyx", "ntx", "rockchip".
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

/// Gallery 3's colour update speed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum GallerySpeed {
    #[default]
    Standard,
    Fast,
    Best,
}

/// The refresh settings, crossed once and again when one changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct EinkPolicy {
    pub panel: EinkPanel,
    /// A full refresh on every image page.
    pub image_full: bool,
    /// A full refresh every this many text pages; 0 never.
    pub text_every: u8,
    /// The same on the Dark look.
    pub text_every_dark: u8,
    /// A full refresh every this many list pages; 0 never.
    pub list_every: u8,
    pub gallery_speed: GallerySpeed,
    /// The look is dark.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub dark: bool,
}

impl EinkPolicy {
    /// The e-ink board's defaults for `panel`: full on every image page, a full every sixth text
    /// page (fourth on Dark), every fifth list page (third on Kaleido).
    pub const fn defaults(panel: EinkPanel) -> Self {
        Self {
            panel,
            image_full: true,
            text_every: 6,
            text_every_dark: 4,
            list_every: if matches!(panel, EinkPanel::Kaleido3) { 3 } else { 5 },
            gallery_speed: GallerySpeed::Standard,
            dark: false,
        }
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
        EinkPaint::ChromeClose | EinkPaint::EndPage | EinkPaint::Clean => full(),
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

    #[test]
    fn text_pages_go_full_every_sixth_and_every_fourth_on_dark() {
        let text = [EinkPaint::TextPage; 12];
        let grey = EinkPolicy::defaults(EinkPanel::Grey);
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
    fn images_chrome_and_lists_follow_the_board() {
        let grey = EinkPolicy::defaults(EinkPanel::Grey);
        assert_eq!(
            run(grey, &[EinkPaint::TextPage, EinkPaint::ImagePage { colour: false }]),
            [Refresh::Partial, Refresh::Full]
        );
        assert_eq!(
            run(grey, &[EinkPaint::ChromeOpen, EinkPaint::ChromeClose, EinkPaint::EndPage]),
            [Refresh::Fast, Refresh::Full, Refresh::Full]
        );
        let kaleido = EinkPolicy::defaults(EinkPanel::Kaleido3);
        let lists = run(kaleido, &[EinkPaint::ListPage { covers: true }; 3]);
        assert_eq!(lists, [Refresh::Partial, Refresh::Partial, Refresh::Full]);
        assert_eq!(EinkPolicy::defaults(EinkPanel::Grey).list_every, 5);
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

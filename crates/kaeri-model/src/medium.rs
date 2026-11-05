//! Watch, read, or both: what this device's Kaeri is for, and the build it runs in.
//!
//! One answer per device, kept in its config, and the account's answer as a synced register. Every
//! place reads it from [`MediumView`]: Home rows, Search's scope, notifications, Accounts, store
//! lists, settings and widgets. Turning a medium off keeps its data.
//!
//! While reading ships dark the engine says reading is not available, never asks, and treats
//! every answer as Watch.

/// What Kaeri is for on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum KaeriIsFor {
    Watch,
    Read,
    Both,
}

impl KaeriIsFor {
    /// The name the config and the `medium_account` register hold: `watch`, `read`, `both`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Watch => "watch",
            Self::Read => "read",
            Self::Both => "both",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "watch" => Some(Self::Watch),
            "read" => Some(Self::Read),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    pub const fn watches(self) -> bool {
        matches!(self, Self::Watch | Self::Both)
    }

    pub const fn reads(self) -> bool {
        matches!(self, Self::Read | Self::Both)
    }
}

/// Which build of Kaeri runs: the full app, the e-reader build, or a TV.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum AppEdition {
    #[default]
    Full,
    /// The Android e-reader build: reading only, never asks.
    ReadOnly,
    /// A TV: watching only. It stores and relays reading data and shows none of it.
    WatchOnly,
}

impl AppEdition {
    /// The answer the edition fixes, for a build that never asks.
    pub const fn fixed(self) -> Option<KaeriIsFor> {
        match self {
            Self::Full => None,
            Self::ReadOnly => Some(KaeriIsFor::Read),
            Self::WatchOnly => Some(KaeriIsFor::Watch),
        }
    }
}

/// A question about the medium the app shows once.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MediumAsk {
    /// First run's page 2 of 4, with no Skip and nothing chosen.
    FirstRun,
    /// One sheet over Home for someone updating from a version without reading, Watch chosen.
    Updated,
    /// This device joined sync and the others answer differently: "{devices} are set to
    /// {answer}", their answer chosen. `devices` are the other devices' names.
    Joined { devices: Vec<String>, answer: KaeriIsFor },
}

/// The medium as every place reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct MediumView {
    /// This device's answer, in effect: Watch while reading is not available, else the edition's
    /// own when it fixes one.
    pub answer: KaeriIsFor,
    pub edition: AppEdition,
    /// The edition fixes the answer: never asked, and Settings has no row for it.
    pub fixed: bool,
    /// Reading is in this build. False while reading ships dark.
    pub reading_available: bool,
    /// A question to show once, `None` when there is none.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub ask: Option<MediumAsk>,
    /// The answer synced from the account, when one exists.
    #[cfg_attr(feature = "ffi", uniffi(default))]
    pub account: Option<KaeriIsFor>,
}

impl MediumView {
    /// The answer in effect for `stored` (this device's saved answer, if any), as the edition and
    /// reading's availability allow.
    pub fn effective(
        stored: Option<KaeriIsFor>,
        edition: AppEdition,
        reading_available: bool,
    ) -> KaeriIsFor {
        if !reading_available {
            return KaeriIsFor::Watch;
        }
        if let Some(fixed) = edition.fixed() {
            return fixed;
        }
        stored.unwrap_or(KaeriIsFor::Watch)
    }

    /// Whether reading shows anywhere on this device.
    pub const fn reads(&self) -> bool {
        self.reading_available && self.answer.reads()
    }
}

/// Watch, read or both. Answered with [`crate::event::Event::Medium`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum MediumCommand {
    /// Send the view.
    Load,
    /// Choose in Settings: written to this device and to the account's register. With
    /// `delete_downloads` the downloads of the medium turned off are deleted; its other data
    /// stays.
    Choose { answer: KaeriIsFor, delete_downloads: bool },
    /// Answer a question the view asked: first run, the update sheet or a join.
    Answer { ask: MediumAsk, answer: KaeriIsFor },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_edition_and_dark_reading_decide_before_the_stored_answer() {
        use KaeriIsFor::*;
        assert_eq!(MediumView::effective(Some(Read), AppEdition::Full, true), Read);
        assert_eq!(MediumView::effective(None, AppEdition::Full, true), Watch);
        assert_eq!(MediumView::effective(Some(Both), AppEdition::Full, false), Watch);
        assert_eq!(MediumView::effective(Some(Both), AppEdition::WatchOnly, true), Watch);
        assert_eq!(MediumView::effective(None, AppEdition::ReadOnly, true), Read);
        assert_eq!(MediumView::effective(None, AppEdition::ReadOnly, false), Watch);
        for answer in [Watch, Read, Both] {
            assert_eq!(KaeriIsFor::parse(answer.as_str()), Some(answer));
        }
        assert_eq!(KaeriIsFor::parse("listen"), None);
        assert!(Both.reads() && Both.watches() && !Read.watches());
    }
}

//! Private watching and reading: what a front end is told of it.
//!
//! Private mode lasts until it is turned off or the app quits, on this device only. While it is
//! on, nothing watched or read is written to the history, sent to a tracker or synced, and the
//! place in an episode or chapter is held in memory until the app quits. A show or series can be
//! always private on this device, which keeps it out the same way whether the mode is on or not.
//!
//! The engine says the mode with [`Event::Private`](crate::event::Event::Private) at launch and
//! on every change, and a title's flag with
//! [`Event::AlwaysPrivate`](crate::event::Event::AlwaysPrivate). Views of a title carry what was
//! true when they were sent, as an `Option<PrivateMode>`; a front end draws the line under the
//! main button from [`PrivateMode::shown`], which puts the mode as it is now beside it.

use kaeri_core::{books::WorkId, ids::AnilistId};

/// Why a title is private: the mode, on for the session, or the title's own flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PrivateMode {
    /// Private mode is on: "Private · Not saved or synced".
    Session,
    /// The title is always private on this device: "Always private · Not saved or synced".
    Always,
}

impl PrivateMode {
    /// What a title page shows, from what its view said and whether the mode is on now: the
    /// title's own flag first, then the mode.
    pub fn shown(view: Option<Self>, on: bool) -> Option<Self> {
        match view {
            Some(Self::Always) => Some(Self::Always),
            _ if on => Some(Self::Session),
            _ => None,
        }
    }
}

/// A title that can be always private: a show, or a series to read.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum PrivateTitle {
    Show { id: AnilistId },
    Series { work: WorkId },
}

impl PrivateTitle {
    /// How the device's database keeps it: `show:<id>` or `series:<work>`.
    pub fn key(&self) -> String {
        match self {
            Self::Show { id } => format!("show:{}", id.get()),
            Self::Series { work } => format!("series:{}", work.as_str()),
        }
    }

    /// The title `key` names, `None` for a key this version does not read.
    pub fn from_key(key: &str) -> Option<Self> {
        match key.split_once(':')? {
            ("show", id) => Some(Self::Show { id: AnilistId::new(id.parse().ok()?) }),
            ("series", work) if !work.is_empty() => {
                Some(Self::Series { work: WorkId::new(work) })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_keeps_its_key() {
        for title in [
            PrivateTitle::Show { id: AnilistId::new(4242) },
            PrivateTitle::Series { work: WorkId::new("a:30001") },
            PrivateTitle::Series { work: WorkId::new("l:00ff") },
        ] {
            assert_eq!(PrivateTitle::from_key(&title.key()), Some(title));
        }
        assert_eq!(PrivateTitle::from_key("show:x"), None);
        assert_eq!(PrivateTitle::from_key("series:"), None);
        assert_eq!(PrivateTitle::from_key("film:1"), None);
    }

    #[test]
    fn the_titles_own_flag_comes_before_the_mode() {
        use PrivateMode::{Always, Session};
        assert_eq!(PrivateMode::shown(Some(Always), false), Some(Always));
        assert_eq!(PrivateMode::shown(Some(Always), true), Some(Always));
        assert_eq!(PrivateMode::shown(Some(Session), false), None, "turned off since");
        assert_eq!(PrivateMode::shown(None, true), Some(Session), "turned on since");
        assert_eq!(PrivateMode::shown(None, false), None);
    }
}

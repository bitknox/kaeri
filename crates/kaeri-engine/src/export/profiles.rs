//! The e-readers Send to e-reader prepares files for, as data: a screen, whether it shows
//! colour, and what it opens. A few device classes are built in; the person's own come from the
//! config (`reading.export_profiles`), and one with the same id replaces the built-in.

use kaeri_core::config::ReadingConfig;
use kaeri_model::reading::{ExportFormat, ExportTarget};
use kaeri_pages::Panel;

/// One target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub panel: Panel,
    pub formats: Vec<ExportFormat>,
    /// Files are split under the Kindle limit.
    pub split: bool,
    /// The largest file the target takes, checked before a send. `None` when it takes any size.
    pub max_file_bytes: Option<u64>,
}

/// The largest file Send to Kindle takes.
pub const KINDLE_SEND_BYTES: u64 = 200_000_000;
/// The largest file a FAT32 card or drive holds, which is what most e-readers use.
pub const FAT32_FILE_BYTES: u64 = 4_294_967_295;

impl Profile {
    pub fn colour(&self) -> bool {
        self.panel.shows_colour()
    }

    /// What the apps list.
    pub fn target(&self) -> ExportTarget {
        ExportTarget {
            id: self.id.clone(),
            name: self.name.clone(),
            width_px: self.width,
            height_px: self.height,
            colour: self.colour(),
            formats: self.formats.clone(),
            manga_formats: self.accepts(false),
            novel_formats: self.accepts(true),
        }
    }

    /// The formats this target is written in: for a novel (`text`) the EPUB kinds, for pages
    /// the fixed-layout and comic kinds. What the share sheet offers, and the only formats a
    /// send is prepared in.
    pub fn accepts(&self, text: bool) -> Vec<ExportFormat> {
        use ExportFormat::{Cbz, Epub, FixedEpub, Kepub};
        let kinds: &[ExportFormat] =
            if text { &[Epub, Kepub] } else { &[FixedEpub, Kepub, Cbz] };
        self.formats.iter().copied().filter(|f| kinds.contains(f)).collect()
    }
}

fn builtin(
    id: &str,
    name: String,
    (width, height): (u32, u32),
    panel: Panel,
    formats: &[ExportFormat],
    split: bool,
) -> Profile {
    let max_file_bytes = match id {
        "kindle" => Some(KINDLE_SEND_BYTES),
        "tablet" => None,
        _ => Some(FAT32_FILE_BYTES),
    };
    Profile {
        id: id.into(),
        name,
        width,
        height,
        panel,
        formats: formats.to_vec(),
        split,
        max_file_bytes,
    }
}

/// The device classes every install has.
pub fn builtin_profiles() -> Vec<Profile> {
    use ExportFormat::{Cbz, Epub, FixedEpub, Kepub};
    let t = kaeri_i18n::t::export_device_other;
    vec![
        builtin("kobo", "Kobo".into(), (1264, 1680), Panel::Grey, &[Kepub, Cbz, Epub], false),
        builtin("kindle", "Kindle".into(), (1072, 1448), Panel::Grey, &[FixedEpub, Epub], true),
        builtin("other", t(), (1072, 1448), Panel::Grey, &[Cbz, FixedEpub, Epub], false),
        builtin(
            "eink-6",
            kaeri_i18n::t::export_device_eink_6(),
            (1072, 1448),
            Panel::Grey,
            &[Cbz, FixedEpub, Epub],
            false,
        ),
        builtin(
            "eink-7",
            kaeri_i18n::t::export_device_eink_7(),
            (1264, 1680),
            Panel::Grey,
            &[Cbz, FixedEpub, Epub],
            false,
        ),
        builtin(
            "colour-eink",
            kaeri_i18n::t::export_device_colour_eink(),
            (1264, 1680),
            Panel::Kaleido3,
            &[Cbz, FixedEpub, Epub],
            false,
        ),
        builtin(
            "tablet",
            kaeri_i18n::t::export_device_tablet(),
            (1600, 2560),
            Panel::Lcd,
            &[Cbz, FixedEpub, Epub],
            false,
        ),
    ]
}

fn format_named(name: &str) -> Option<ExportFormat> {
    Some(match name.trim().to_ascii_lowercase().as_str() {
        "fixed-epub" => ExportFormat::FixedEpub,
        "kepub" => ExportFormat::Kepub,
        "epub" => ExportFormat::Epub,
        "cbz" => ExportFormat::Cbz,
        _ => return None,
    })
}

/// Built-in profiles, then the person's own. A user profile with no usable size is left out.
pub fn all(reading: &ReadingConfig) -> Vec<Profile> {
    let mut out = builtin_profiles();
    for (id, own) in &reading.export_profiles {
        if own.width < 100 || own.height < 100 || own.width > 8000 || own.height > 8000 {
            continue;
        }
        let mut formats: Vec<ExportFormat> =
            own.formats.iter().filter_map(|f| format_named(f)).collect();
        if formats.is_empty() {
            formats.push(ExportFormat::Cbz);
        }
        let profile = Profile {
            id: id.clone(),
            name: if own.name.trim().is_empty() { id.clone() } else { own.name.clone() },
            width: own.width,
            height: own.height,
            panel: if own.colour { Panel::UnknownColour } else { Panel::Grey },
            formats,
            split: false,
            max_file_bytes: Some(FAT32_FILE_BYTES),
        };
        match out.iter_mut().find(|p| p.id == *id) {
            Some(existing) => *existing = profile,
            None => out.push(profile),
        }
    }
    out
}

pub fn find(reading: &ReadingConfig, id: &str) -> Option<Profile> {
    all(reading).into_iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kaeri_core::config::ExportProfileConfig;

    #[test]
    fn a_target_says_which_formats_it_is_offered_for_pages_and_for_novels() {
        use ExportFormat::{Cbz, Epub, FixedEpub, Kepub};
        let reading = ReadingConfig::default();
        let kobo = find(&reading, "kobo").unwrap().target();
        assert_eq!(kobo.manga_formats, [Kepub, Cbz]);
        assert_eq!(kobo.novel_formats, [Kepub, Epub]);
        let kindle = find(&reading, "kindle").unwrap().target();
        assert_eq!(kindle.manga_formats, [FixedEpub]);
        assert_eq!(kindle.novel_formats, [Epub]);
        // Nothing offered is something a send refuses.
        for profile in all(&reading) {
            for text in [false, true] {
                for format in profile.accepts(text) {
                    assert!(profile.formats.contains(&format));
                }
            }
        }
    }

    #[test]
    fn built_in_classes_and_the_persons_own() {
        let mut reading = ReadingConfig::default();
        let ids: Vec<String> = all(&reading).into_iter().map(|p| p.id).collect();
        assert!(ids.iter().any(|i| i == "kobo") && ids.iter().any(|i| i == "tablet"));
        reading.export_profiles.insert(
            "attic-reader".into(),
            ExportProfileConfig {
                name: "Attic reader".into(),
                width: 758,
                height: 1024,
                colour: false,
                formats: vec!["epub".into(), "nonsense".into()],
            },
        );
        reading.export_profiles.insert(
            "tiny".into(),
            ExportProfileConfig { width: 10, height: 10, ..Default::default() },
        );
        let own = find(&reading, "attic-reader").unwrap();
        assert_eq!((own.width, own.height, own.formats), (758, 1024, vec![ExportFormat::Epub]));
        assert!(find(&reading, "tiny").is_none(), "no usable size");
    }

    #[test]
    fn only_the_kindle_splits() {
        let splits: Vec<String> =
            builtin_profiles().into_iter().filter(|p| p.split).map(|p| p.id).collect();
        assert_eq!(splits, ["kindle"]);
    }
}

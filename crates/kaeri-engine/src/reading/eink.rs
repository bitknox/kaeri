//! Whether this screen is e-ink, which panel it has, and the refresh rule it paints by.
//!
//! The vendor family comes from the device's identity, never from which refresh hook resolved:
//! on the vendor's own devices a second vendor's call resolves and does nothing. The identity
//! list below is written from public device facts (manufacturer and brand strings, the vendor's
//! own system property).

use std::collections::BTreeMap;

use kaeri_model::{
    eink::{
        EinkMatch, EinkPanel, EinkPolicy, EinkProfile, GallerySpeed, NovelSupport, ScreenFacts,
        VendorFamily,
    },
    reader::ReaderSettingKey,
};

/// The person's answers to "Is this an e-ink screen?" and "Is this a colour screen?", as kept in
/// `reading.eink`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EinkAnswers {
    /// `None`: auto.
    pub eink: Option<bool>,
    /// `None`: auto.
    pub colour: Option<bool>,
}

impl EinkAnswers {
    pub fn from_config(eink: &BTreeMap<String, toml::Value>) -> Self {
        let word = |key: &str| eink.get(key).and_then(toml::Value::as_str).unwrap_or("auto");
        Self {
            eink: match word("mode") {
                "on" => Some(true),
                "off" => Some(false),
                _ => None,
            },
            colour: match word("colour_panel") {
                "colour" => Some(true),
                "greys" => Some(false),
                _ => None,
            },
        }
    }
}

fn has(text: &str, any: &[&str]) -> bool {
    let text = text.to_ascii_lowercase();
    any.iter().any(|word| text.contains(word))
}

/// What the device's identity says: the family, and whether it is known e-ink.
fn identity(facts: &ScreenFacts) -> Option<VendorFamily> {
    let who = format!("{} {} {}", facts.manufacturer, facts.brand, facts.product);
    if facts.onyx_name.is_some() || has(&who, &["onyx"]) {
        Some(VendorFamily::Onyx)
    } else if has(&who, &["bigme", "boyue", "likebook", "meebook"]) {
        Some(VendorFamily::Rockchip)
    } else if has(&who, &["pocketbook", "tolino"]) {
        Some(VendorFamily::Ntx)
    } else {
        None
    }
}

const fn hook_name(family: VendorFamily) -> Option<&'static str> {
    match family {
        VendorFamily::Onyx => Some("onyx"),
        VendorFamily::Rockchip => Some("rockchip"),
        VendorFamily::Ntx => Some("ntx"),
        VendorFamily::Other => None,
    }
}

/// The hook that sets the family's own update modes for a view, where one is known.
const fn modes_hook_name(family: VendorFamily) -> Option<&'static str> {
    match family {
        VendorFamily::Onyx => Some("onyx-modes"),
        VendorFamily::Rockchip | VendorFamily::Ntx | VendorFamily::Other => None,
    }
}

/// Makes the profile from what Android said, the person's answers and the Refresh settings.
pub fn detect(
    facts: &ScreenFacts,
    answers: EinkAnswers,
    device: &BTreeMap<String, toml::Value>,
) -> EinkProfile {
    let known = identity(facts);
    let says_eink =
        facts.eink_support.as_deref().is_some_and(|v| matches!(v, "1" | "true" | "yes"));
    let rockchip_hook = facts.hooks.iter().any(|h| h == "rockchip");
    let slow = facts.refresh_hz > 0.0 && facts.refresh_hz < 50.0;
    let mut eink = if known.is_some() || says_eink {
        EinkMatch::Sure
    } else if slow || rockchip_hook {
        EinkMatch::Unsure
    } else {
        EinkMatch::No
    };
    let family = known.unwrap_or(if rockchip_hook {
        VendorFamily::Rockchip
    } else {
        VendorFamily::Other
    });
    let galy = has(&facts.model, &["galy"]);
    let (mut panel, mut colour_sure) = match eink {
        EinkMatch::No => (EinkPanel::Lcd, true),
        _ if family == VendorFamily::Onyx => {
            (if facts.wide_colour_gamut { EinkPanel::Kaleido3 } else { EinkPanel::Grey }, true)
        }
        _ if galy => (EinkPanel::Gallery3, true),
        _ if facts.wide_colour_gamut => (EinkPanel::UnknownColour, false),
        _ => (EinkPanel::Grey, true),
    };
    match answers.eink {
        Some(true) => {
            eink = EinkMatch::Sure;
            if panel == EinkPanel::Lcd {
                panel = EinkPanel::Grey;
            }
        }
        Some(false) => {
            eink = EinkMatch::No;
            panel = EinkPanel::Lcd;
        }
        None => {}
    }
    if panel != EinkPanel::Lcd {
        match answers.colour {
            Some(true) if !panel.colour() => panel = EinkPanel::UnknownColour,
            Some(false) => panel = EinkPanel::Grey,
            _ => {}
        }
        if answers.colour.is_some() {
            colour_sure = true;
        }
    }
    let resolved = |name: Option<&str>| {
        eink != EinkMatch::No && name.is_some_and(|h| facts.hooks.iter().any(|x| x == h))
    };
    let refresh_hook = resolved(hook_name(family));
    let panel_modes = resolved(modes_hook_name(family));
    EinkProfile {
        eink,
        panel,
        ppi: facts.dpi,
        family,
        refresh_hook,
        panel_modes,
        page_buttons: facts.page_keys,
        colour_sure,
        novels: facts.webview.map_or(NovelSupport::Full, NovelSupport::of_webview),
        policy: (eink != EinkMatch::No).then(|| policy(panel, device)),
    }
}

/// The refresh rule for `panel` with this device's Refresh settings over the defaults, which ask
/// for no full refresh.
pub fn policy(panel: EinkPanel, device: &BTreeMap<String, toml::Value>) -> EinkPolicy {
    let mut policy = EinkPolicy::defaults(panel);
    let number = |key: ReaderSettingKey| {
        device
            .get(key.name())
            .and_then(|v| v.as_integer().or_else(|| v.as_float().map(|f| f as i64)))
    };
    if let Some(on) =
        device.get(ReaderSettingKey::RefreshImage.name()).and_then(toml::Value::as_bool)
    {
        policy.image_full = on;
    }
    if let Some(every) = number(ReaderSettingKey::RefreshText) {
        policy.text_every = every.clamp(0, 50) as u8;
        // Dark pages ghost sooner: two thirds of the cadence, six pages becoming four.
        policy.text_every_dark = (every.clamp(0, 50) as u8).saturating_mul(2).div_ceil(3);
    }
    if let Some(every) = number(ReaderSettingKey::RefreshList) {
        policy.list_every = every.clamp(0, 50) as u8;
    }
    // On Gallery 3, Screen mode is its colour speed.
    policy.gallery_speed =
        match device.get(ReaderSettingKey::ScreenMode.name()).and_then(toml::Value::as_str) {
            Some("fast") if panel == EinkPanel::Gallery3 => GallerySpeed::Fast,
            Some("best") if panel == EinkPanel::Gallery3 => GallerySpeed::Best,
            _ => GallerySpeed::Standard,
        };
    policy
}

/// The pipeline's panel for a profile, when the screen is e-ink; `None` draws as LCD.
pub fn pages_panel(profile: Option<&EinkProfile>) -> kaeri_pages::Panel {
    match profile.filter(|p| p.eink != EinkMatch::No).map(|p| p.panel) {
        Some(EinkPanel::Grey) => kaeri_pages::Panel::Grey,
        Some(EinkPanel::Kaleido3) => kaeri_pages::Panel::Kaleido3,
        Some(EinkPanel::Gallery3) => kaeri_pages::Panel::Gallery3,
        Some(EinkPanel::UnknownColour) => kaeri_pages::Panel::UnknownColour,
        Some(EinkPanel::Lcd) | None => kaeri_pages::Panel::Lcd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(manufacturer: &str, model: &str) -> ScreenFacts {
        ScreenFacts {
            manufacturer: manufacturer.into(),
            brand: manufacturer.into(),
            model: model.into(),
            device: String::new(),
            product: String::new(),
            hardware: String::new(),
            sdk: 33,
            width_px: 1264,
            height_px: 1680,
            dpi: 300,
            refresh_hz: 60.0,
            eink_support: None,
            onyx_name: None,
            wide_colour_gamut: false,
            hooks: Vec::new(),
            page_keys: false,
            webview: Some(120),
        }
    }

    #[test]
    fn the_family_comes_from_identity_and_colour_from_the_gamut() {
        let none = BTreeMap::new();
        let mut boox = facts("ONYX", "Reader 7");
        boox.hooks = vec!["onyx".into(), "ntx".into()];
        boox.page_keys = true;
        let p = detect(&boox, EinkAnswers::default(), &none);
        assert_eq!(
            (p.eink, p.panel, p.family),
            (EinkMatch::Sure, EinkPanel::Grey, VendorFamily::Onyx)
        );
        assert!(p.refresh_hook && p.page_buttons && p.colour_sure);
        assert!(!p.panel_modes, "the mode calls did not resolve");
        assert_eq!(p.policy, Some(EinkPolicy::defaults(EinkPanel::Grey)));
        boox.hooks.push("onyx-modes".into());
        assert!(detect(&boox, EinkAnswers::default(), &none).panel_modes);
        boox.wide_colour_gamut = true;
        assert_eq!(detect(&boox, EinkAnswers::default(), &none).panel, EinkPanel::Kaleido3);

        // The NTX call resolving says nothing about the family, nor do the Onyx mode calls.
        let mut other = facts("Example", "Pad");
        other.hooks = vec!["ntx".into(), "onyx-modes".into()];
        let p = detect(&other, EinkAnswers::default(), &none);
        assert_eq!((p.eink, p.panel, p.policy), (EinkMatch::No, EinkPanel::Lcd, None));
        assert!(!p.panel_modes);
        let mut ntx = facts("Tolino", "Vision");
        ntx.hooks = vec!["ntx".into(), "onyx-modes".into()];
        let p = detect(&ntx, EinkAnswers::default(), &none);
        assert!(p.refresh_hook && !p.panel_modes);

        let galy = detect(&facts("Bigme", "Galy"), EinkAnswers::default(), &none);
        assert_eq!((galy.panel, galy.family), (EinkPanel::Gallery3, VendorFamily::Rockchip));
    }

    #[test]
    fn a_slow_screen_is_unsure_and_answers_settle_it() {
        let none = BTreeMap::new();
        let mut slow = facts("Example", "Note");
        slow.refresh_hz = 30.0;
        slow.wide_colour_gamut = true;
        let p = detect(&slow, EinkAnswers::default(), &none);
        assert_eq!(
            (p.eink, p.panel, p.colour_sure),
            (EinkMatch::Unsure, EinkPanel::UnknownColour, false)
        );
        let yes = EinkAnswers { eink: Some(true), colour: Some(false) };
        let p = detect(&slow, yes, &none);
        assert_eq!((p.eink, p.panel, p.colour_sure), (EinkMatch::Sure, EinkPanel::Grey, true));
        let no = EinkAnswers { eink: Some(false), colour: None };
        assert_eq!(detect(&slow, no, &none).panel, EinkPanel::Lcd);
        let mut old = facts("Example", "Note");
        old.webview = Some(90);
        assert_eq!(
            detect(&old, EinkAnswers::default(), &none).novels,
            NovelSupport::KnownFlaws
        );
    }

    #[test]
    fn refresh_settings_shape_the_policy() {
        let mut device = BTreeMap::new();
        device.insert("refresh_text".to_owned(), toml::Value::Integer(9));
        device.insert("refresh_image".to_owned(), toml::Value::Boolean(false));
        device.insert("screen_mode".to_owned(), toml::Value::String("fast".into()));
        let p = policy(EinkPanel::Gallery3, &device);
        assert_eq!((p.text_every, p.text_every_dark, p.image_full), (9, 6, false));
        assert_eq!(p.gallery_speed, GallerySpeed::Fast);
        // Settings a person saved keep their values: a full on picture pages and lists.
        let mut saved = BTreeMap::new();
        saved.insert("refresh_image".to_owned(), toml::Value::Boolean(true));
        saved.insert("refresh_list".to_owned(), toml::Value::Integer(3));
        saved.insert("screen_mode".to_owned(), toml::Value::String("regal".into()));
        let p = policy(EinkPanel::Kaleido3, &saved);
        assert_eq!((p.image_full, p.text_every, p.list_every), (true, 0, 3));
        assert_eq!(p.gallery_speed, GallerySpeed::Standard);
        // Nothing saved: no full refresh at all.
        assert!(!policy(EinkPanel::Kaleido3, &BTreeMap::new()).any_full());
        let mut cfg = BTreeMap::new();
        cfg.insert("mode".to_owned(), toml::Value::String("on".into()));
        assert_eq!(
            EinkAnswers::from_config(&cfg),
            EinkAnswers { eink: Some(true), colour: None }
        );
    }
}

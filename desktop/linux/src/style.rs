//! The app's look on libadwaita: dark whatever the system says, the Obi & Silk stylesheet over
//! libadwaita's, and Zen Kaku Gothic New for show titles.

use std::path::PathBuf;

use gtk::pango::prelude::FontMapExt;

const CSS: &str = include_str!("style.css");
/// GTK's CSS has no media queries yet: high contrast is a second sheet, put in while it is on.
const HIGH_CONTRAST: &str = include_str!("style-hc.css");

/// Put the look in place for `display`: once, before the first window.
pub fn install(display: &gtk::gdk::Display) {
    adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
    let css = gtk::CssProvider::new();
    css.load_from_string(CSS);
    gtk::style_context_add_provider_for_display(
        display,
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
    let contrast = gtk::CssProvider::new();
    contrast.load_from_string(HIGH_CONTRAST);
    let styles = adw::StyleManager::default();
    let apply = {
        let display = display.clone();
        move |styles: &adw::StyleManager| {
            if styles.is_high_contrast() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &contrast,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1,
                );
            } else {
                gtk::style_context_remove_provider_for_display(&display, &contrast);
            }
        }
    };
    apply(&styles);
    styles.connect_high_contrast_notify(apply);
    register_fonts();
}

/// The quieter text colour, for Pango markup that CSS can't reach: the lifted haze under high
/// contrast, as `--haze` in style-hc.css.
pub fn haze() -> &'static str {
    if adw::StyleManager::default().is_high_contrast() {
        "#B8BDD6" // i18n-skip
    } else {
        "#A3A9C4" // i18n-skip
    }
}

/// Calls `redraw` when High Contrast turns on or off, while `owner` lives: for text whose
/// colour is in Pango markup ([`haze`]), which CSS doesn't restyle.
pub fn on_contrast_change(
    owner: &impl gtk::prelude::IsA<gtk::Widget>,
    redraw: impl Fn() + 'static,
) {
    use gtk::prelude::{ObjectExt, WidgetExt};
    let manager = adw::StyleManager::default();
    let id = manager.connect_high_contrast_notify(move |_| redraw());
    let id = std::cell::Cell::new(Some(id));
    owner.connect_destroy(move |_| {
        if let Some(id) = id.take() {
            adw::StyleManager::default().disconnect(id);
        }
    });
}

/// Zen Kaku Gothic New for the show titles. An installed copy is found by fontconfig on its own;
/// a build run from the repository adds the copy in its assets.
fn register_fonts() {
    let map = pangocairo::FontMap::default();
    for file in font_files() {
        if file.exists() && map.add_font_file(&file).is_err() {
            tracing::warn!(file = %file.display(), "a font did not load");
        }
    }
}

/// The folder holding the bundled faces: libass looks there for the one a subtitle names.
pub fn fonts_dir() -> PathBuf {
    std::env::var_os("KAERI_FONTS").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/mpv/fonts")
    })
}

fn font_files() -> Vec<PathBuf> {
    let dir = fonts_dir();
    ["ZenKakuGothicNew-Bold.ttf", "ZenKakuGothicNew-Regular.ttf"]
        .into_iter()
        .map(|name| dir.join(name))
        .collect()
}

/// The app's icon at each size the hicolor theme takes, from `assets/icon`.
const ICONS: [(u32, &[u8]); 7] = [
    (16, include_bytes!("../../../assets/icon/16x16.png")),
    (32, include_bytes!("../../../assets/icon/32x32.png")),
    (48, include_bytes!("../../../assets/icon/48x48.png")),
    (64, include_bytes!("../../../assets/icon/64x64.png")),
    (128, include_bytes!("../../../assets/icon/128x128.png")),
    (256, include_bytes!("../../../assets/icon/256x256.png")),
    (512, include_bytes!("../../../assets/icon/512x512.png")),
];

/// The app's icon, for the About dialog and the window list, when no installed copy is in the
/// icon theme: written under `cache` as a hicolor tree and added to the theme's search path.
/// The mini player's icon, which the Adwaita theme has none of: a frame with a small one in its
/// corner.
const MINI_PLAYER: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill="#222" d="M2.5 2A1.5 1.5 0 0 0 1 3.5v8A1.5 1.5 0 0 0 2.5 13H6v-1.5H2.5v-8h11V7H15V3.5A1.5 1.5 0 0 0 13.5 2z"/><rect fill="#222" x="7.5" y="8.5" width="7.5" height="5.5" rx="1"/></svg>"##; // i18n-skip

/// Two people side by side, for watching together: the player's people button.
const PARTY: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><g fill="#222"><circle cx="4.5" cy="4.5" r="2.25"/><circle cx="11.5" cy="4.5" r="2.25"/><path d="M1.5 14v-2.5a3 3 0 0 1 6 0V14zM8.5 14v-2.5a3 3 0 0 1 6 0V14z"/></g></svg>"##; // i18n-skip

/// A screen with the cast waves in its corner: a TV with Chromecast built in, in the picker. GTK
/// recolours a symbolic icon by its fill alone and fills every path, so the frame and the waves
/// are drawn as filled outlines of lines 1.2 wide, not as strokes.
const CAST: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="#222"><path d="M.9 5V3.5A1.6 1.6 0 0 1 2.5 1.9H13.5A1.6 1.6 0 0 1 15.1 3.5V11.5A1.6 1.6 0 0 1 13.5 13.1H9.5A.6 .6 0 0 1 9.5 11.9H13.5A.4 .4 0 0 0 13.9 11.5V3.5A.4 .4 0 0 0 13.5 3.1H2.5A.4 .4 0 0 0 2.1 3.5V5A.6 .6 0 0 1 .9 5z"/><path d="M1.5 10.4A3.1 3.1 0 0 1 4.6 13.5A.6 .6 0 0 1 3.4 13.5A1.9 1.9 0 0 0 1.5 11.6A.6 .6 0 0 1 1.5 10.4zM1.5 7.9A5.6 5.6 0 0 1 7.1 13.5A.6 .6 0 0 1 5.9 13.5A4.4 4.4 0 0 0 1.5 9.1A.6 .6 0 0 1 1.5 7.9z"/><circle cx="1.5" cy="13.5" r=".6"/></svg>"##; // i18n-skip

/// The same screen while a TV is held: its frame stays and the screen inside it fills, clear of
/// the frame and of the outer wave by the line's width, as the cast glyph on Android draws it.
const CAST_CONNECTED: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="#222"><path d="M.9 5V3.5A1.6 1.6 0 0 1 2.5 1.9H13.5A1.6 1.6 0 0 1 15.1 3.5V11.5A1.6 1.6 0 0 1 13.5 13.1H9.5A.6 .6 0 0 1 9.5 11.9H13.5A.4 .4 0 0 0 13.9 11.5V3.5A.4 .4 0 0 0 13.5 3.1H2.5A.4 .4 0 0 0 2.1 3.5V5A.6 .6 0 0 1 .9 5z"/><path d="M1.5 10.4A3.1 3.1 0 0 1 4.6 13.5A.6 .6 0 0 1 3.4 13.5A1.9 1.9 0 0 0 1.5 11.6A.6 .6 0 0 1 1.5 10.4zM1.5 7.9A5.6 5.6 0 0 1 7.1 13.5A.6 .6 0 0 1 5.9 13.5A4.4 4.4 0 0 0 1.5 9.1A.6 .6 0 0 1 1.5 7.9z"/><circle cx="1.5" cy="13.5" r=".6"/><path d="M3.3 4.3H12.7V10.7H7.7A6.8 6.8 0 0 0 3.3 6.94z"/></svg>"##; // i18n-skip

/// A die showing five: Surprise me and Another, as on every other platform.
const DICE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16"><path fill="#222" fill-rule="evenodd" d="M4 1.5h8A2.5 2.5 0 0 1 14.5 4v8a2.5 2.5 0 0 1-2.5 2.5H4A2.5 2.5 0 0 1 1.5 12V4A2.5 2.5 0 0 1 4 1.5zm0 1.25A1.25 1.25 0 0 0 2.75 4v8A1.25 1.25 0 0 0 4 13.25h8A1.25 1.25 0 0 0 13.25 12V4A1.25 1.25 0 0 0 12 2.75z"/><g fill="#222"><circle cx="5.25" cy="5.25" r="1.1"/><circle cx="10.75" cy="5.25" r="1.1"/><circle cx="8" cy="8" r="1.1"/><circle cx="5.25" cy="10.75" r="1.1"/><circle cx="10.75" cy="10.75" r="1.1"/></g></svg>"##; // i18n-skip

/// The reader's Chapters button: lines of a contents page, the last one short, as the reader board
/// draws it. Each line is the filled outline of a 1.5 wide line with round ends.
const CHAPTERS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="#222"><path d="M2 3.25H14A.75 .75 0 0 1 14 4.75H2A.75 .75 0 0 1 2 3.25zM2 7.25H14A.75 .75 0 0 1 14 8.75H2A.75 .75 0 0 1 2 7.25zM2 11.25H9A.75 .75 0 0 1 9 12.75H2A.75 .75 0 0 1 2 11.25z"/></svg>"##; // i18n-skip

/// A large A and a small a: the reader's Aa, its settings for the page and the text. The letters
/// are the filled outlines of 1.4 wide lines with round ends and joins; the a's bowl is a ring.
const TEXT_LOOK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="#222"><path d="M.837 13.274L4.587 2.274A.7 .7 0 0 1 5.913 2.274L9.663 13.274A.7 .7 0 0 1 8.337 13.726L5.25 4.669L2.163 13.726A.7 .7 0 0 1 .837 13.274zM2.9 8.9H7.6A.7 .7 0 0 1 7.6 10.3H2.9A.7 .7 0 0 1 2.9 8.9zM9 11A3 3 0 0 1 15 11A3 3 0 0 1 9 11zM10.4 11A1.6 1.6 0 0 0 13.6 11A1.6 1.6 0 0 0 10.4 11zM13.6 8.4A.7 .7 0 0 1 15 8.4V13.5A.7 .7 0 0 1 13.6 13.5z"/></svg>"##; // i18n-skip

/// An open book in outline: the foot of a cover plate for a work with no cover. The covers and
/// the spine are the filled outline of a 1.4 wide line with round joins, the two pages cut out
/// of it as holes wound the other way.
const BOOK: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16" fill="#222"><path d="M8 3.316C7.216 2.769 6.304 2.418 5.26 2.221C4.22 2.026 3.041 1.981 1.713 2.051A.7 .7 0 0 0 1.05 2.75V12.25A.7 .7 0 0 0 1.787 12.949C3.174 12.876 4.322 12.939 5.273 13.141C6.22 13.342 6.967 13.678 7.568 14.15A.7 .7 0 0 0 8 14.3A.7 .7 0 0 0 8.432 14.15C9.033 13.678 9.78 13.342 10.727 13.141C11.678 12.939 12.826 12.876 14.213 12.949A.7 .7 0 0 0 14.95 12.25V2.75A.7 .7 0 0 0 14.287 2.051C12.959 1.981 11.78 2.026 10.74 2.221C9.696 2.418 8.784 2.769 8 3.316zM7.3 12.348C6.647 12.035 5.919 11.82 5.114 11.687C4.31 11.555 3.424 11.504 2.45 11.524V3.424C3.568 3.4 4.512 3.478 5.312 3.662C6.109 3.846 6.759 4.136 7.3 4.537zM8.7 4.537C9.241 4.136 9.891 3.846 10.688 3.662C11.488 3.478 12.432 3.4 13.55 3.424V11.524C12.576 11.504 11.69 11.555 10.886 11.687C10.081 11.82 9.353 12.035 8.7 12.348z"/></svg>"##; // i18n-skip

/// The icons the app draws itself: the mini player's, the people button's, the cast screen's
/// (outline and held), the die, the reader's Aa and its Chapters, and the book. GTK recolours a
/// symbolic icon by forcing the fill of every shape and ignores strokes, so each one is drawn in
/// filled shapes only, a line as the outline of its stroke.
const ACTION_ICONS: [(&str, &str); 8] = [
    ("kaeri-mini-player-symbolic", MINI_PLAYER),
    ("kaeri-party-symbolic", PARTY),
    ("kaeri-cast-symbolic", CAST),
    ("kaeri-cast-connected-symbolic", CAST_CONNECTED),
    ("kaeri-dice-symbolic", DICE),
    ("kaeri-text-look-symbolic", TEXT_LOOK),
    ("kaeri-chapters-symbolic", CHAPTERS),
    ("kaeri-book-symbolic", BOOK),
];

/// Writes [`ACTION_ICONS`] into the cache's own theme folder and adds it to the theme's path.
pub fn action_icons(display: &gtk::gdk::Display, cache: &std::path::Path) {
    let root = cache.join("icons");
    let dir = root.join("hicolor/scalable/actions");
    let written = ACTION_ICONS.iter().all(|(name, svg)| {
        let file = dir.join(format!("{name}.svg"));
        std::fs::read_to_string(&file).is_ok_and(|s| s == *svg)
            || std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, svg)).is_ok()
    });
    if written {
        gtk::IconTheme::for_display(display).add_search_path(&root);
    }
}

pub fn app_icon(display: &gtk::gdk::Display, name: &str, cache: &std::path::Path) {
    let theme = gtk::IconTheme::for_display(display);
    if theme.has_icon(name) {
        return;
    }
    let root = cache.join("icons");
    let written = ICONS.iter().all(|(size, png)| {
        let dir = root.join(format!("hicolor/{size}x{size}/apps"));
        let file = dir.join(format!("{name}.png"));
        std::fs::metadata(&file).is_ok_and(|m| m.len() == png.len() as u64)
            || std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(&file, png)).is_ok()
    });
    if written {
        theme.add_search_path(&root);
    } else {
        tracing::warn!(path = %root.display(), "the app icon could not be written");
    }
}

#[cfg(test)]
mod tests {
    use super::ACTION_ICONS;

    #[test]
    fn every_drawn_icon_is_filled_shapes_only() {
        for (name, svg) in ACTION_ICONS {
            assert!(!svg.contains("stroke"), "{name} draws a stroke, which GTK fills");
            assert!(!svg.contains("fill=\"none\""), "{name} leaves a shape unfilled");
        }
    }
}

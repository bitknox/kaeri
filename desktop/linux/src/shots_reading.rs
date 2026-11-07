//! The reading shots: each Read place, a series' page, the reader in its three forms and Send
//! to e-reader, on the reading fixture scene (`--scene reading`). The light ones are the reader's
//! own light looks, as the app's chrome is dark only.

use std::rc::Rc;

use adw::prelude::*;
use kaeri_core::books::WorkId;
use kaeri_model::reader::ReaderSettingKey;

use crate::{pages::Dest, window::Shell};

/// Every reading shot, in the order a run takes them.
pub const NAMES: [&str; 21] = [
    "read",
    "read-updates",
    "read-library",
    "read-library-filters",
    "read-search",
    "read-search-filters",
    "read-downloads",
    "read-sources",
    "read-series",
    "read-series-source",
    "read-paged",
    "read-paged-light",
    "read-paged-crop",
    "read-paged-single",
    "read-paged-width",
    "read-paged-settings",
    "read-strip",
    "read-strip-light",
    "read-novel",
    "read-novel-light",
    "read-send",
];

const MANGA: &str = "a:900301";
const NOVEL: &str = "a:900302";
const STRIP: &str = "a:900303";

fn read(shell: &Rc<Shell>, work: &str, chapter: &str) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.read",
        Some(&(work, chapter).to_variant()),
    );
}

fn series(shell: &Rc<Shell>, work: &str) {
    let _ = WidgetExt::activate_action(
        &shell.ctx.window,
        "win.open-series",
        Some(&work.to_variant()),
    );
}

/// Put the window in the state `shot` shows, and say how long to let it settle, when it is one.
pub fn stage(shell: &Rc<Shell>, shot: &str) -> Option<f64> {
    Some(match shot {
        "read" => {
            shell.go(Dest::Read);
            3.0
        }
        "read-updates" => {
            shell.go(Dest::Updates);
            3.0
        }
        "read-library" => {
            shell.go(Dest::ReadLibrary);
            3.0
        }
        "read-library-filters" => {
            shell.go(Dest::ReadLibrary);
            crate::reading::library::shot_filters(shell);
            3.0
        }
        "read-search" => {
            shell.go(Dest::ReadSearch);
            3.0
        }
        "read-search-filters" => {
            shell.go(Dest::ReadSearch);
            crate::reading::search::shot_filters(shell);
            3.5
        }
        "read-downloads" => {
            shell.go(Dest::ReadDownloads);
            3.0
        }
        "read-sources" => {
            shell.go(Dest::ReadSources);
            3.0
        }
        "read-series" => {
            series(shell, MANGA);
            3.5
        }
        "read-series-source" => {
            series(shell, MANGA);
            crate::reading::series::shot_source_picker(shell);
            3.5
        }
        "read-paged" => {
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-light" => {
            shell.reader.shot_setting(ReaderSettingKey::Background, "white");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-crop" => {
            shell.reader.shot_setting(ReaderSettingKey::CropBorders, "on");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-single" => {
            shell.reader.shot_setting(ReaderSettingKey::PageLayout, "single");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-width" => {
            shell.reader.shot_setting(ReaderSettingKey::PageLayout, "single");
            shell.reader.shot_setting(ReaderSettingKey::Fit, "width");
            read(shell, MANGA, "13");
            4.0
        }
        "read-paged-settings" => {
            read(shell, MANGA, "13");
            let reader = Rc::clone(&shell.reader);
            gtk::glib::timeout_add_local_once(
                std::time::Duration::from_millis(2500),
                move || {
                    reader.pad(kaeri_model::input::ReaderPad::Settings);
                },
            );
            4.5
        }
        "read-strip" => {
            read(shell, STRIP, "2");
            4.0
        }
        "read-strip-light" => {
            shell.reader.shot_setting(ReaderSettingKey::Background, "white");
            read(shell, STRIP, "2");
            4.0
        }
        "read-novel" => {
            read(shell, NOVEL, "3");
            7.0
        }
        "read-novel-light" => {
            shell.reader.shot_setting(ReaderSettingKey::Look, "light");
            read(shell, NOVEL, "3");
            7.0
        }
        "read-send" => {
            series(shell, MANGA);
            crate::reading::export::open(&shell.ctx, &WorkId(MANGA.to_owned()));
            3.0
        }
        _ => return None,
    })
}

/// Leave what `shot` opened.
pub fn unstage(shell: &Rc<Shell>, shot: &str) {
    if !NAMES.contains(&shot) {
        return;
    }
    if let Some(dialog) = shell.ctx.window.visible_dialog() {
        dialog.force_close();
    }
    shell.reader.close();
    // Settings go back to the defaults the next shot expects.
    shell.reader.shot_setting(ReaderSettingKey::Background, "theme");
    shell.reader.shot_setting(ReaderSettingKey::CropBorders, "off");
    shell.reader.shot_setting(ReaderSettingKey::PageLayout, "auto");
    shell.reader.shot_setting(ReaderSettingKey::Fit, "screen");
    shell.reader.shot_setting(ReaderSettingKey::Look, "system");
    shell.go(Dest::Read);
}

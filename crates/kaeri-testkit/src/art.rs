//! Cover, banner and still art for the fixture titles, for a front end that shows images.
//!
//! The native app's design draws each invented title as a stand-in: a traveller under a crescent
//! moon, a tower library above the clouds, a station platform under a full moon, a steel
//! gardener's flower, a running track at summer's end. These are those drawings rendered to PNG,
//! a 460 × 690 poster and a 1600 × 900 wide one each, and the second season's drawn in another
//! tone. The opt-in catalogue's eight titles ([`crate::fixtures::catalogue`]) are drawn the same
//! way, a lighthouse, a koi pond, paper lanterns and so on, and Home's world's 36 as the Home
//! design board draws them. The SVG each PNG was rendered from is
//! under `fixtures/art/src/`. The people the viewer follows ([`crate::fixtures::following`]) have
//! a 128 × 128 picture each, drawn as that board draws them, at `avatar/<name>.png`. [`FakeAniList::serve_art`](crate::FakeAniList::serve_art) serves them at the paths the
//! fixtures' own image URLs name.

use crate::fixtures::{AIRING, AIRING_NO_MAL, CATALOGUE, FINISHED, MOVIE, ONA, SEQUEL};

/// One title's drawing, as a poster and wide.
struct Art {
    poster: &'static [u8],
    wide: &'static [u8],
}

macro_rules! art {
    ($name:literal) => {
        Art {
            poster: include_bytes!(concat!("../fixtures/art/", $name, "-poster.png")),
            wide: include_bytes!(concat!("../fixtures/art/", $name, "-wide.png")),
        }
    };
}

const STARDUST: Art = art!("stardust");
const STARDUST_SECOND: Art = art!("stardust-2");
const LIBRARY: Art = art!("library");
const PLATFORM: Art = art!("platform");
const GARDENER: Art = art!("gardener");
const RELAY: Art = art!("relay");

/// The catalogue's drawings, in [`CATALOGUE`]'s order.
const CATALOGUE_ART: [Art; 8] = [
    art!("lighthouse"),
    art!("koi"),
    art!("lanterns"),
    art!("train"),
    art!("fox"),
    art!("radio"),
    art!("greenhouse"),
    art!("rowing"),
];

/// Home's world's drawings ([`crate::home_world`]), in its ids' order from 950001: the board's
/// fireworks, kites and washing lines in its palettes, and the drawings above turned and
/// recoloured, each title its own.
const WORLD_ART: [Art; 36] = [
    art!("hanabi"),
    art!("hotaru"),
    art!("kamome"),
    art!("sora"),
    art!("niji"),
    art!("akari"),
    art!("minato"),
    art!("shiro"),
    art!("kaze"),
    art!("tsubame"),
    art!("ginga"),
    art!("kishi-2"),
    art!("shokudo-2"),
    art!("kujira"),
    art!("kasa"),
    art!("mori"),
    art!("koto"),
    art!("kishi"),
    art!("shokudo"),
    art!("ryokan"),
    art!("garasu"),
    art!("tegami"),
    art!("densha"),
    art!("kumo"),
    art!("hoshi"),
    art!("fune"),
    art!("yume"),
    art!("neko"),
    art!("mizu"),
    art!("haru"),
    art!("yoru"),
    art!("ame"),
    art!("kissa"),
    art!("kame"),
    art!("nami"),
    art!("tanpopo"),
];

/// The first of [`crate::fixtures::books`]' ids, and how many there are.
const BOOKS_FIRST: u32 = 900_301;
const BOOKS: u32 = 80;

fn of(id: u32) -> Option<&'static Art> {
    if let Some(n) = id.checked_sub(crate::home_world::FIRST)
        && let Some(art) = WORLD_ART.get(n as usize)
    {
        return Some(art);
    }
    // The books borrow the catalogue's and the world's drawings, one each until they run out,
    // then round again.
    if let Some(n) = id.checked_sub(BOOKS_FIRST).filter(|n| *n < BOOKS) {
        let n = n as usize % (CATALOGUE_ART.len() + WORLD_ART.len());
        return CATALOGUE_ART.get(n).or_else(|| WORLD_ART.get(n - CATALOGUE_ART.len()));
    }
    let id = kaeri_core::ids::AnilistId::new(id);
    Some(match id {
        FINISHED => &STARDUST,
        SEQUEL => &STARDUST_SECOND,
        AIRING => &LIBRARY,
        MOVIE => &PLATFORM,
        ONA => &GARDENER,
        AIRING_NO_MAL => &RELAY,
        _ => return CATALOGUE.iter().position(|c| *c == id).map(|n| &CATALOGUE_ART[n]),
    })
}

/// The people with a picture, as [`crate::fixtures::following`] names them, and their pictures.
const AVATARS: [(&str, &[u8]); 10] = [
    ("ines", include_bytes!("../fixtures/art/avatar-ines.png")),
    ("lumi", include_bytes!("../fixtures/art/avatar-lumi.png")),
    ("dario", include_bytes!("../fixtures/art/avatar-dario.png")),
    ("rowanpaper", include_bytes!("../fixtures/art/avatar-rowanpaper.png")),
    ("oskar", include_bytes!("../fixtures/art/avatar-oskar.png")),
    ("teovale", include_bytes!("../fixtures/art/avatar-teovale.png")),
    ("petrakite", include_bytes!("../fixtures/art/avatar-petrakite.png")),
    ("kofi", include_bytes!("../fixtures/art/avatar-kofi.png")),
    ("yunami", include_bytes!("../fixtures/art/avatar-yunami.png")),
    ("halden", include_bytes!("../fixtures/art/avatar-halden.png")),
];

/// The PNG for an image path under [`crate::fixtures::ART_HOST`], or `None` for a path the
/// fixtures do not name: `cover/900001-xl.jpg` and `cover/900001.jpg` are the poster,
/// `banner/900001.jpg` and an episode's still, `ep/900001/3.jpg`, the wide drawing, and
/// `avatar/Ines.png` a person's picture.
pub fn png(path: &str) -> Option<&'static [u8]> {
    let mut parts = path.trim_start_matches('/').split('/');
    let kind = parts.next()?;
    let id = parts.next()?;
    if kind == "avatar" {
        let name = id.strip_suffix(".png")?.to_lowercase();
        return AVATARS.iter().find(|(n, _)| *n == name).map(|(_, png)| *png);
    }
    let id: u32 = id.split(['-', '.']).next()?.parse().ok()?;
    let art = of(id)?;
    match kind {
        "cover" => Some(art.poster),
        "banner" | "ep" => Some(art.wide),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

    #[test]
    fn every_image_the_fixtures_name_has_art() {
        for path in
            ["cover/900001-xl.jpg", "cover/900006.jpg", "banner/900004.jpg", "ep/900001/7.jpg"]
        {
            assert!(png(path).is_some_and(|bytes| bytes.starts_with(PNG)), "{path}");
        }
        for id in CATALOGUE {
            for kind in ["cover", "banner"] {
                let path = format!("{kind}/{id}.jpg");
                assert!(png(&path).is_some_and(|bytes| bytes.starts_with(PNG)), "{path}");
            }
        }
        for id in crate::home_world::ids() {
            for kind in ["cover", "banner"] {
                let path = format!("{kind}/{id}.jpg");
                assert!(png(&path).is_some_and(|bytes| bytes.starts_with(PNG)), "{path}");
            }
        }
        let covers: std::collections::HashSet<&[u8]> = crate::home_world::ids()
            .iter()
            .filter_map(|id| png(&format!("cover/{id}.jpg")))
            .collect();
        assert_eq!(covers.len(), 36, "each of the world's titles is drawn its own way");
        assert_ne!(png("cover/900001.jpg"), png("cover/900004.jpg"), "the sequel is its own");
        assert_ne!(
            png("cover/900001.jpg"),
            png("banner/900001.jpg"),
            "a poster is not a banner"
        );
    }

    #[test]
    fn a_path_the_fixtures_do_not_name_has_none() {
        assert!(png("cover/900999.jpg").is_none());
        assert!(png("cover/950037.jpg").is_none());
        assert!(png("avatar/900001.jpg").is_none());
        assert!(png("avatar/SannaQ.png").is_none(), "SannaQ has no picture");
        assert!(png("avatar/Ines.png").is_some_and(|bytes| bytes.starts_with(PNG)));
        assert!(png("cover").is_none());
    }
}

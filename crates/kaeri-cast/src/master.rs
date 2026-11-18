//! An HLS master playlist's audio renditions, and the master rewritten to keep the one chosen.
//!
//! A master can carry its sound as renditions beside the picture (`EXT-X-MEDIA:TYPE=AUDIO`),
//! one per language: the original and a dub in one stream. mpv lists each as a track and plays
//! the one the app picks. A receiver plays the one its own player picks, `DEFAULT=YES` or the
//! TV's language, and a sender can't name another in its `LOAD`: the receiver numbers a
//! manifest's tracks itself, after the load, and whether it switches them later depends on the
//! player its firmware runs. So the TV is handed the master with the chosen rendition alone in
//! each audio group, marked the default: every receiver plays the one rendition there is.

use kaeri_playback::tracks::language_matches;

/// One `EXT-X-MEDIA:TYPE=AUDIO` of a master.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendition {
    /// Its `GROUP-ID`, which a variant names with `AUDIO`.
    pub group: String,
    pub language: Option<String>,
    pub name: Option<String>,
    pub default: bool,
    /// Its `CHANNELS`' count, when it says.
    pub channels: Option<u16>,
}

/// The audio renditions of a master, in its order.
pub fn audio_renditions(text: &str) -> Vec<Rendition> {
    text.lines().filter_map(|line| rendition(line.trim())).collect()
}

fn rendition(line: &str) -> Option<Rendition> {
    let attrs = line.strip_prefix("#EXT-X-MEDIA:")?;
    let pairs = attributes(attrs);
    let get = |name: &str| {
        pairs
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.trim_matches('"').to_owned())
    };
    if !get("TYPE")?.eq_ignore_ascii_case("AUDIO") {
        return None;
    }
    Some(Rendition {
        group: get("GROUP-ID").unwrap_or_default(),
        language: get("LANGUAGE").filter(|l| !l.is_empty()),
        name: get("NAME").filter(|n| !n.is_empty()),
        default: get("DEFAULT").is_some_and(|d| d.eq_ignore_ascii_case("YES")),
        channels: get("CHANNELS")
            .and_then(|c| c.split('/').next().and_then(|n| n.trim().parse().ok())),
    })
}

/// A tag's attributes as written, each value with its quotes.
fn attributes(attrs: &str) -> Vec<(String, String)> {
    let mut pairs = Vec::new();
    let mut rest = attrs.trim();
    while !rest.is_empty() {
        let Some((key, after)) = rest.split_once('=') else { break };
        let end = if let Some(quoted) = after.strip_prefix('"') {
            quoted.find('"').map_or(after.len(), |e| e + 2)
        } else {
            after.find(',').unwrap_or(after.len())
        };
        pairs.push((key.trim().to_owned(), after[..end].to_owned()));
        rest = after[end..].trim_start_matches(',').trim_start();
    }
    pairs
}

fn written(pairs: &[(String, String)]) -> String {
    pairs.iter().map(|(key, value)| format!("{key}={value}")).collect::<Vec<_>>().join(",")
}

fn set(pairs: &mut Vec<(String, String)>, name: &str, value: &str) {
    match pairs.iter_mut().find(|(key, _)| key.eq_ignore_ascii_case(name)) {
        Some(pair) => pair.1 = value.to_owned(),
        None => pairs.push((name.to_owned(), value.to_owned())),
    }
}

/// Whether `rendition` is the one asked for: its name and language, as the probe read them
/// from the source's master. The master the TV gets may be the stream proxy's copy, or another
/// group of the same master, so it is found by what it is, not by its place.
fn same(rendition: &Rendition, language: Option<&str>, name: Option<&str>) -> bool {
    let same_language = match (rendition.language.as_deref(), language) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b) || language_matches(a, b),
        (None, None) => true,
        _ => false,
    };
    same_language && rendition.name.as_deref() == name
}

fn in_language(rendition: &Rendition, language: Option<&str>) -> bool {
    match (rendition.language.as_deref(), language) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b) || language_matches(a, b),
        _ => false,
    }
}

/// `text` with the audio rendition in `language` named `name` the only one of each audio
/// group, marked `DEFAULT=YES` and `AUTOSELECT=YES`. In a group without that name, the first
/// in the language stands in. A variant whose group has neither is left out, so the picture
/// never switches to a variant with other sound; a variant with no audio group stays.
///
/// `None` when there is nothing to choose or nothing matches: then the master goes as it is.
pub fn keep_audio(text: &str, language: Option<&str>, name: Option<&str>) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let found: Vec<(usize, Rendition)> = lines
        .iter()
        .enumerate()
        .filter_map(|(at, line)| rendition(line.trim()).map(|r| (at, r)))
        .collect();
    if found.len() < 2 {
        return None;
    }
    let mut groups: Vec<&str> = Vec::new();
    for (_, rendition) in &found {
        if !groups.contains(&rendition.group.as_str()) {
            groups.push(&rendition.group);
        }
    }
    // The line each group keeps, for the groups that have the one asked for.
    let kept: Vec<(&str, usize)> = groups
        .iter()
        .filter_map(|group| {
            let members = || found.iter().filter(|(_, r)| r.group == *group);
            members()
                .find(|(_, r)| same(r, language, name))
                .or_else(|| members().find(|(_, r)| in_language(r, language)))
                .map(|(at, _)| (*group, *at))
        })
        .collect();
    if kept.is_empty() {
        return None;
    }
    let kept_group = |group: &str| kept.iter().any(|(g, _)| *g == group);

    let mut out = String::with_capacity(text.len());
    let mut variants = 0;
    let mut skip_uri = false;
    for (at, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if skip_uri {
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                skip_uri = false;
            }
            continue;
        }
        if found.iter().any(|(a, _)| *a == at) {
            if kept.iter().any(|(_, a)| *a == at) {
                let attrs = trimmed.strip_prefix("#EXT-X-MEDIA:").unwrap_or_default();
                let mut pairs = attributes(attrs);
                set(&mut pairs, "DEFAULT", "YES");
                set(&mut pairs, "AUTOSELECT", "YES");
                out.push_str("#EXT-X-MEDIA:");
                out.push_str(&written(&pairs));
                out.push('\n');
            }
            continue;
        }
        if let Some(attrs) = trimmed.strip_prefix("#EXT-X-STREAM-INF:") {
            let group = attributes(attrs)
                .into_iter()
                .find(|(key, _)| key.eq_ignore_ascii_case("AUDIO"))
                .map(|(_, value)| value.trim_matches('"').to_owned());
            if group.as_deref().is_some_and(|g| !kept_group(g)) {
                skip_uri = true;
                continue;
            }
            variants += 1;
        }
        out.push_str(line);
        out.push('\n');
    }
    (variants > 0).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A master with Japanese and an English dub as renditions, in two groups for two codecs,
    /// on made-up hosts.
    const MASTER: &str = "#EXTM3U
#EXT-X-INDEPENDENT-SEGMENTS
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aac\",LANGUAGE=\"ja\",NAME=\"Japanese\",DEFAULT=YES,AUTOSELECT=YES,URI=\"audio/ja/aac.m3u8\"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aac\",LANGUAGE=\"en\",NAME=\"English\",DEFAULT=NO,AUTOSELECT=YES,CHANNELS=\"6\",URI=\"audio/en/aac.m3u8\"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"ac3\",LANGUAGE=\"ja\",NAME=\"Japanese\",DEFAULT=YES,URI=\"https://audio.example/ja/ac3.m3u8\"
#EXT-X-MEDIA:TYPE=SUBTITLES,GROUP-ID=\"subs\",LANGUAGE=\"en\",NAME=\"English\",URI=\"subs/en.m3u8\"
#EXT-X-STREAM-INF:BANDWIDTH=2000000,RESOLUTION=1280x720,CODECS=\"avc1.64001f,mp4a.40.2\",AUDIO=\"aac\",SUBTITLES=\"subs\"
video/720.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=5000000,RESOLUTION=1920x1080,CODECS=\"avc1.640028,ac-3\",AUDIO=\"ac3\"
video/1080.m3u8
#EXT-X-I-FRAME-STREAM-INF:BANDWIDTH=90000,URI=\"video/iframes.m3u8\"
";

    #[test]
    fn renditions_are_read_with_their_group_language_and_name() {
        let found = audio_renditions(MASTER);
        assert_eq!(found.len(), 3);
        assert_eq!(
            found[1],
            Rendition {
                group: "aac".into(),
                language: Some("en".into()),
                name: Some("English".into()),
                default: false,
                channels: Some(6),
            }
        );
        assert!(found[0].default);
    }

    #[test]
    fn the_dub_is_the_only_audio_left_and_a_group_without_it_goes_with_its_variants() {
        let kept = keep_audio(MASTER, Some("en"), Some("English")).unwrap();
        let audio = audio_renditions(&kept);
        assert_eq!(audio.len(), 1, "{kept}");
        assert_eq!(audio[0].language.as_deref(), Some("en"));
        assert!(audio[0].default);
        assert!(kept.contains("AUTOSELECT=YES,CHANNELS=\"6\",URI=\"audio/en/aac.m3u8\""));
        // The variant whose group has no English goes, its address with it.
        assert!(kept.contains("video/720.m3u8"));
        assert!(!kept.contains("video/1080.m3u8"), "{kept}");
        assert!(!kept.contains("AUDIO=\"ac3\""));
        // What isn't audio stays as it was.
        assert!(kept.contains("TYPE=SUBTITLES"));
        assert!(kept.contains("#EXT-X-I-FRAME-STREAM-INF"));
        assert!(kept.contains("#EXT-X-INDEPENDENT-SEGMENTS"));
    }

    #[test]
    fn the_original_is_kept_in_every_group_that_has_it() {
        let kept = keep_audio(MASTER, Some("jpn"), Some("Japanese")).unwrap();
        let audio = audio_renditions(&kept);
        assert_eq!(audio.len(), 2, "{kept}");
        assert!(audio.iter().all(|r| r.language.as_deref() == Some("ja") && r.default));
        assert!(kept.contains("video/720.m3u8") && kept.contains("video/1080.m3u8"));
    }

    #[test]
    fn a_name_that_differs_still_finds_the_language() {
        let kept = keep_audio(MASTER, Some("en-US"), Some("English 5.1")).unwrap();
        let audio = audio_renditions(&kept);
        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].name.as_deref(), Some("English"));
    }

    #[test]
    fn nothing_to_choose_or_nothing_matching_leaves_the_master_alone() {
        assert_eq!(keep_audio(MASTER, Some("fr"), Some("French")), None);
        let one = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",LANGUAGE=\"ja\",NAME=\"Japanese\",URI=\"a.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1,AUDIO=\"a\"\nv.m3u8\n";
        assert_eq!(keep_audio(one, Some("ja"), Some("Japanese")), None);
        let media = "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4.0,\nseg0.ts\n";
        assert_eq!(keep_audio(media, Some("ja"), None), None);
    }

    #[test]
    fn renditions_without_a_language_are_told_apart_by_name() {
        let master = "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Main\",DEFAULT=YES,URI=\"main.m3u8\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",NAME=\"Dub\",URI=\"dub.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1,AUDIO=\"a\"\nv.m3u8\n";
        let kept = keep_audio(master, None, Some("Dub")).unwrap();
        let audio = audio_renditions(&kept);
        assert_eq!(audio.len(), 1);
        assert_eq!(audio[0].name.as_deref(), Some("Dub"));
        assert!(audio[0].default);
    }
}

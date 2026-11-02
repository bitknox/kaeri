//! WebVTT as sources write it, set right for the players that read it: the subtitles a plugin
//! supplies, which are fetched and cached before a file opens, and the ones an HLS playlist
//! lists, which the stream proxy serves.

/// A WebVTT file as the source meant it, or `None` when it needs nothing.
///
/// Some sources convert their subtitles to WebVTT from a format that allows more than WebVTT
/// does, and two things come out broken:
///
/// - **A cue with a blank line in it**, an on-screen panel of text laid out as paragraphs. A
///   blank line ends a WebVTT cue, so the paragraphs after it are left as blocks with no timing.
///   The format says to skip such a block. ffmpeg, which mpv reads WebVTT with, stops at it
///   instead, and every cue after it is lost: an episode's subtitles end a few minutes in. Each
///   such block goes back onto the cue before it, after a line holding only a no-break space,
///   which is not blank to a parser and shows as the gap the source had.
/// - **A shape drawn by command**, `m 370 219 l 370 339 595 339`, the box behind that panel. WebVTT
///   has no drawings, so it shows the commands as text. They are dropped, and a cue that was
///   nothing else goes with them.
pub(crate) fn mend_webvtt(text: &str) -> Option<String> {
    let text = text.replace("\r\n", "\n");
    let mut blocks: Vec<String> = Vec::new();
    let mut mended = false;
    for block in text.split("\n\n").map(|b| b.trim_start_matches('\n')) {
        if block.trim().is_empty() {
            continue;
        }
        let mut lines = block.lines();
        let timed = lines.next().is_some_and(|l| l.contains("-->"))
            || lines.next().is_some_and(|l| l.contains("-->"));
        let own_block = ["NOTE", "STYLE", "REGION"].iter().any(|word| block.starts_with(word));
        let kept: Vec<&str> = block.lines().filter(|line| !is_drawing(line)).collect();
        mended |= kept.len() != block.lines().count();
        let block = kept.join("\n");
        match blocks.last_mut() {
            Some(previous) if !timed && !own_block => {
                if !block.trim().is_empty() {
                    previous.push_str("\n\u{a0}\n");
                    previous.push_str(&block);
                }
                mended = true;
            }
            _ => blocks.push(block),
        }
    }
    // A cue whose only text was a drawing is its timing alone now.
    blocks.retain(|block| !block.lines().last().is_some_and(|l| l.contains("-->")));
    mended.then(|| blocks.join("\n\n") + "\n")
}

/// Whether a cue line is a vector drawing — `m`, then drawing commands and coordinates only —
/// once its tags are set aside.
fn is_drawing(line: &str) -> bool {
    let mut text = String::new();
    let mut in_tag = false;
    for c in line.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let mut words = text.split_whitespace();
    if words.next() != Some("m") {
        return false;
    }
    let rest: Vec<&str> = words.collect();
    let number = |w: &str| w.parse::<f64>().is_ok();
    rest.iter().filter(|w| number(w)).count() >= 2
        && rest
            .iter()
            .all(|w| number(w) || matches!(*w, "m" | "n" | "l" | "b" | "s" | "p" | "c"))
}

/// A WebVTT file whose mending changes it: a cue holding a blank line, then the blocks after it.
#[cfg(test)]
pub(crate) const BROKEN: &str = "WEBVTT\n\n00:01.000 --> 00:02.000\nHello.\n\n\
                   q1\n00:03.000 --> 00:06.000\n<b>Quest</b>\n\nRescue the prisoner,\nquickly.\n\n\
                   00:03.000 --> 00:06.000\n<b>m 0 0 l 200 0 200 71 0 71</b>\n\n\
                   00:04.000 --> 00:05.000\nmore to do\nm 370 219 l 370 339 595 339\n\n\
                   NOTE kept as it is\n\n00:07.000 --> 00:08.000\nGoodbye.\n";

/// What [`BROKEN`] comes out as.
#[cfg(test)]
pub(crate) const MENDED: &str = "WEBVTT\n\n00:01.000 --> 00:02.000\nHello.\n\n\
             q1\n00:03.000 --> 00:06.000\n<b>Quest</b>\n\u{a0}\nRescue the prisoner,\nquickly.\n\n\
             00:04.000 --> 00:05.000\nmore to do\n\n\
             NOTE kept as it is\n\n00:07.000 --> 00:08.000\nGoodbye.\n";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cue_with_a_blank_line_keeps_its_paragraphs_and_the_cues_after_it() {
        let mended = mend_webvtt(BROKEN).expect("the file is mended");
        assert_eq!(mended, MENDED);

        let clean = "WEBVTT\r\n\r\n00:01.000 --> 00:02.000\r\nHello.\r\nmy 2 cents\r\n";
        assert_eq!(mend_webvtt(clean), None, "a sound file is left as it came");
    }
}

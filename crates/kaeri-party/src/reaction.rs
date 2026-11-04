//! Reactions: Syncplay has no message for them, so they travel as chat lines in a fixed, readable
//! form, `*laughs* 12:04`, with the room's position at the press. Another Syncplay app shows the
//! line as text; this client shows it as a glyph at that moment. Someone typing the line by hand
//! makes a reaction, which is harmless.

use std::fmt::Write as _;

/// One of the four reactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reaction {
    Laugh,
    Gasp,
    Clap,
    Cry,
}

impl Reaction {
    pub const ALL: [Reaction; 4] = [Self::Laugh, Self::Gasp, Self::Clap, Self::Cry];

    /// The word the line carries, an English one so every client reads the same.
    pub fn word(self) -> &'static str {
        match self {
            Self::Laugh => "laughs",
            Self::Gasp => "gasps",
            Self::Clap => "claps",
            Self::Cry => "cries",
        }
    }

    fn from_word(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.word() == word)
    }
}

/// The line for `reaction` at `at` seconds into the episode: `*laughs* 12:04`, or `*claps*
/// 1:02:03` from the first hour on.
pub fn encode(reaction: Reaction, at: f64) -> String {
    format!("*{}* {}", reaction.word(), clock(at))
}

/// A line read as a reaction, with its position in seconds. `None` for any other line, including
/// one with more after the time.
pub fn decode(line: &str) -> Option<(Reaction, f64)> {
    let rest = line.trim().strip_prefix('*')?;
    let (word, time) = rest.split_once('*')?;
    let reaction = Reaction::from_word(word)?;
    Some((reaction, parse_clock(time.strip_prefix(' ')?)?))
}

/// `m:ss`, or `h:mm:ss`. A position that is not a finite, non-negative number reads as 0:00.
pub fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 { seconds as u64 } else { 0 };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    let mut out = String::new();
    if h > 0 {
        let _ = write!(out, "{h}:{m:02}:{s:02}");
    } else {
        let _ = write!(out, "{m}:{s:02}");
    }
    out
}

fn parse_clock(text: &str) -> Option<f64> {
    let digits = |part: &str, width: Option<usize>| -> Option<u64> {
        let ok = !part.is_empty()
            && part.len() <= 6
            && part.bytes().all(|b| b.is_ascii_digit())
            && width.is_none_or(|w| part.len() == w);
        ok.then(|| part.parse().ok()).flatten()
    };
    let parts: Vec<&str> = text.split(':').collect();
    let total = match parts.as_slice() {
        [m, s] => {
            let (m, s) = (digits(m, None)?, digits(s, Some(2))?);
            (s < 60).then_some(m * 60 + s)?
        }
        [h, m, s] => {
            let (h, m, s) = (digits(h, None)?, digits(m, Some(2))?, digits(s, Some(2))?);
            (m < 60 && s < 60).then_some(h * 3600 + m * 60 + s)?
        }
        _ => return None,
    };
    Some(total as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reaction_is_written_with_its_word_and_the_position() {
        assert_eq!(encode(Reaction::Laugh, 724.9), "*laughs* 12:04");
        assert_eq!(encode(Reaction::Gasp, 0.0), "*gasps* 0:00");
        assert_eq!(encode(Reaction::Clap, 3723.0), "*claps* 1:02:03");
        assert_eq!(encode(Reaction::Cry, -4.0), "*cries* 0:00");
        assert_eq!(encode(Reaction::Cry, f64::NAN), "*cries* 0:00");
    }

    #[test]
    fn every_reaction_reads_back_at_its_second() {
        for reaction in Reaction::ALL {
            for at in [0.0, 59.0, 60.0, 724.0, 3599.0, 3600.0, 7325.0] {
                assert_eq!(decode(&encode(reaction, at)), Some((reaction, at)));
            }
        }
    }

    #[test]
    fn other_lines_are_not_reactions() {
        for line in [
            "",
            "laughs 12:04",
            "*laughs*",
            "*laughs* soon",
            "*laughs* 12:4",
            "*laughs* 12:60",
            "*laughs* 1:2:03",
            "*laughs* 12:04 lol",
            "*dances* 12:04",
            "*laughs*12:04",
            "lol *laughs* 12:04",
            "*laughs* -1:04",
        ] {
            assert_eq!(decode(line), None, "{line:?}");
        }
        assert_eq!(decode("  *claps* 3:24  "), Some((Reaction::Clap, 204.0)));
    }
}

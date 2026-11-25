//! How far the subtitles rise while the player's controls are up: as far as they need to clear
//! the controls, and no further. Subtitles that already sit clear of them stay where they are.
//!
//! The apps move the subtitles with mpv's `sub-pos`. libass applies it to bottom-aligned lines
//! with no `\pos`, `\move`, `\clip`, `\org` or drawing in them, so signs stay put, and moves such a
//! line up by `sub-pos`'s share of the distance from the top of its frame to the line's resting
//! bottom edge (`ass_render.c`, the subtitle case of the vertical placement). Where the line rests
//! and what its frame is depend on how mpv lays the track out (`sd_ass.c`, `configure_ass`):
//!
//! - Plain text (SRT, WebVTT and the like) and stripped ASS take mpv's own style, laid out over
//!   the window (`sub-use-margins`): the line rests `sub-margin-y` above the window's bottom, 34
//!   at a 720-line window, scaled with the window and with `sub-scale`.
//! - ASS kept as written is laid out in the picture: a line rests its style's MarginV of PlayResY
//!   above the picture's bottom, so a letterbox bar below the picture adds to the room.
//! - Restyled ASS (`sub-ass-override=force`) keeps its margins but is laid out over the window, as
//!   plain text is: a line rests MarginV of the picture fitted to the window above its bottom.
//!
//! The controls want the line's bottom edge as far above the window's bottom as plain subtitles
//! rise to while they are up: the controls' height plus mpv's own margin. The lift is what it
//! takes to get there from where the viewer's Raise puts the line, never more than the controls'
//! whole height, which is what every track was lifted by before. A track whose placement cannot be
//! known, a picture track or an ASS header that does not parse, still takes the whole height.

use crate::subtitle_style::{SubtitleStyle, SystemCaptionStyle, subtitle_scale};
use kaeri_core::config::StyledSubtitles;

/// mpv's `sub-margin-y`, which Kaeri leaves at mpv's default, at a 720-line window.
const PLAIN_MARGIN: f64 = 34.0 / 720.0;

/// The most the controls lift the subtitles: half the picture.
const MOST: f64 = 50.0;

/// The video's place in the player's frame: mpv's `osd-dimensions`, in its pixels.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Record))]
pub struct SubtitleFrame {
    /// The frame, `w` and `h`.
    pub width: f64,
    pub height: f64,
    /// The bars around the picture: `mt`, `mb`, `ml` and `mr`.
    pub top: f64,
    pub bottom: f64,
    pub left: f64,
    pub right: f64,
}

/// Where the subtitle track's bottom lines rest, as far as it can be known.
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "ffi", derive(uniffi::Enum))]
pub enum SubtitlePlacement {
    /// Not known: no subtitles, a picture track (PGS, VobSub), or an ASS header that does not say.
    #[default]
    Unknown,
    /// Plain text, which mpv draws in its own style.
    Plain,
    /// ASS: the lowest margin of its bottom-aligned styles, in its script's `PlayResY`.
    Styled { margin: f64, play_height: f64 },
}

/// The text codecs mpv converts to its own style (`lavc_conv.c`), by mpv's codec names.
const PLAIN_CODECS: &[&str] = &[
    "subrip",
    "srt",
    "webvtt",
    "mov_text",
    "text",
    "microdvd",
    "mpl2",
    "vplayer",
    "pjs",
    "jacosub",
    "realtext",
    "sami",
    "subviewer",
    "subviewer1",
    "stl",
];

/// The placement of a subtitle track from mpv's `current-tracks/sub/codec` and, for ASS, its
/// `sub-ass-extradata`: the script's header with its styles.
pub fn subtitle_placement(codec: Option<&str>, header: Option<&str>) -> SubtitlePlacement {
    match codec {
        Some(codec) if PLAIN_CODECS.contains(&codec) => SubtitlePlacement::Plain,
        Some("ass") => header.and_then(styled_placement).unwrap_or_default(),
        _ => SubtitlePlacement::Unknown,
    }
}

/// libass's style columns when a script has no `Format:` line for them (`ass.c`).
const ASS_STYLE_FORMAT: &str = "Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
    OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, \
    BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding";
const SSA_STYLE_FORMAT: &str = "Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
    TertiaryColour, BackColour, Bold, Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, \
    MarginR, MarginV, AlphaLevel, Encoding";

/// The lowest margin of the header's bottom-aligned styles, and its `PlayResY` as libass settles
/// it. `None` when no style sits at the bottom.
fn styled_placement(header: &str) -> Option<SubtitlePlacement> {
    enum Section {
        Info,
        Styles { ssa: bool },
        Other,
    }
    let mut section = Section::Other;
    let (mut play_width, mut play_height) = (0_i64, 0_i64);
    let mut format: Option<Vec<String>> = None;
    let mut lowest: Option<i64> = None;
    for line in header.lines().map(str::trim) {
        if line.starts_with('[') {
            let name = line.to_ascii_lowercase();
            section = match name.as_str() {
                "[script info]" => Section::Info,
                "[v4+ styles]" => Section::Styles { ssa: false },
                "[v4 styles]" => Section::Styles { ssa: true },
                _ => Section::Other,
            };
            format = None;
            continue;
        }
        match section {
            Section::Info => {
                if let Some(value) = line.strip_prefix("PlayResX:") {
                    play_width = leading_int(value);
                } else if let Some(value) = line.strip_prefix("PlayResY:") {
                    play_height = leading_int(value);
                }
            }
            Section::Styles { ssa } => {
                if let Some(columns) = line.strip_prefix("Format:") {
                    format = Some(columns.split(',').map(|c| c.trim().to_owned()).collect());
                } else if let Some(values) = line.strip_prefix("Style:") {
                    let columns = format.clone().unwrap_or_else(|| {
                        let default = if ssa { SSA_STYLE_FORMAT } else { ASS_STYLE_FORMAT };
                        default.split(',').map(|c| c.trim().to_owned()).collect()
                    });
                    let values: Vec<&str> = values.splitn(columns.len(), ',').collect();
                    let field = |name: &str| {
                        columns
                            .iter()
                            .position(|c| c.eq_ignore_ascii_case(name))
                            .and_then(|i| values.get(i))
                            .map(|v| leading_int(v))
                    };
                    let (Some(alignment), Some(margin)) =
                        (field("Alignment"), field("MarginV"))
                    else {
                        continue;
                    };
                    // Bottom: 1 to 3 in both numberings, and SSA's 8, which VSFilter put there.
                    let bottom = (1..=3).contains(&alignment) || (ssa && alignment == 8);
                    if bottom {
                        lowest = Some(lowest.map_or(margin, |low| low.min(margin)));
                    }
                }
            }
            Section::Other => {}
        }
    }
    let margin = lowest?;
    // libass's defaults for a missing PlayRes (`ass_lazy_track_init`).
    let play_height = match (play_width, play_height) {
        (_, h) if h > 0 => h,
        (w, _) if w <= 0 => 288,
        (1280, _) => 1024,
        (w, _) => ((w - 1) - (w - 1) / 4).max(1),
    };
    Some(SubtitlePlacement::Styled { margin: margin as f64, play_height: play_height as f64 })
}

/// The integer a header value starts with, as libass reads one: 0 when there is none.
fn leading_int(value: &str) -> i64 {
    let value = value.trim_start();
    let end = value
        .char_indices()
        .find(|&(i, c)| !(c.is_ascii_digit() || (i == 0 && (c == '-' || c == '+'))))
        .map_or(value.len(), |(i, _)| i);
    value[..end].parse().unwrap_or(0)
}

/// How far the controls lift the subtitles, percent, on top of the viewer's Raise: what
/// `mpv_subtitle_options` takes as `controls_lift`. `controls` is the share of the frame's height
/// the controls take, zero while they are away.
pub fn subtitle_lift(
    style: &SubtitleStyle,
    system: Option<&SystemCaptionStyle>,
    controls: f64,
    frame: Option<&SubtitleFrame>,
    placement: &SubtitlePlacement,
) -> f64 {
    if controls.is_nan() || controls <= 0.0 {
        return 0.0;
    }
    let full = (controls * 100.0).min(MOST);
    let Some(frame) = frame.filter(|f| f.width > 0.0 && f.height > 0.0) else {
        return full;
    };
    let styled = match placement {
        SubtitlePlacement::Plain => None,
        SubtitlePlacement::Styled { margin, play_height } if *play_height > 0.0 => {
            Some(margin / play_height)
        }
        SubtitlePlacement::Styled { .. } | SubtitlePlacement::Unknown => return full,
    };
    // Shares of the frame's height: the room below the line's bottom edge at rest, and how far
    // libass moves it for the whole of `sub-pos`.
    let (rest, travel) = match (styled, style.styled) {
        (None, _) | (Some(_), StyledSubtitles::Strip) => {
            let rest = PLAIN_MARGIN * subtitle_scale(style, system);
            (rest, 1.0 - rest)
        }
        (Some(margin), StyledSubtitles::Keep) => {
            let (top, bottom) = (frame.top / frame.height, frame.bottom / frame.height);
            let picture = 1.0 - top - bottom;
            (bottom + picture * margin, picture * (1.0 - margin))
        }
        (Some(margin), StyledSubtitles::Restyle) => {
            // The picture fitted to the window (`fit_height` in `ass_render_api.c`).
            let width = frame.width - frame.left - frame.right;
            let height = frame.height - frame.top - frame.bottom;
            let fitted = if width <= 0.0 || width * frame.height <= height * frame.width {
                1.0
            } else {
                height * frame.width / width / frame.height
            };
            let rest = fitted * margin;
            (rest, 1.0 - rest)
        }
    };
    if travel.is_nan() || travel <= 0.0 {
        return full;
    }
    let needed = (controls + PLAIN_MARGIN - rest) / travel * 100.0 - f64::from(style.raise);
    // Up to the next tenth, so rounding never leaves the line under the controls.
    let needed = ((needed * 10.0 - 1e-6).ceil() / 10.0).max(0.0);
    needed.min(full)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1280×720 window with a 16:9 picture filling it.
    fn filled() -> SubtitleFrame {
        SubtitleFrame { width: 1280.0, height: 720.0, ..SubtitleFrame::default() }
    }

    /// The same window with a 2.39:1 picture in it, letterboxed.
    fn letterboxed() -> SubtitleFrame {
        let bar = (720.0 - 1280.0 / 2.39) / 2.0;
        SubtitleFrame { top: bar, bottom: bar, ..filled() }
    }

    /// Controls 90 px tall at 720.
    const CONTROLS: f64 = 90.0 / 720.0;

    fn header(play_res_y: u32, styles: &[(&str, u32, i32)]) -> String {
        let mut text = format!(
            "[Script Info]\nScriptType: v4.00+\nPlayResX: {}\nPlayResY: {play_res_y}\n\n\
             [V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
             OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, \
             Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, \
             Encoding\n",
            play_res_y * 16 / 9
        );
        for (name, alignment, margin) in styles {
            text.push_str(&format!(
                "Style: {name},Sans,48,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,\
                 100,100,0,0,1,2,1,{alignment},20,20,{margin},1\n"
            ));
        }
        text.push_str("\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n");
        text
    }

    fn styled(play_res_y: u32, styles: &[(&str, u32, i32)]) -> SubtitlePlacement {
        subtitle_placement(Some("ass"), Some(&header(play_res_y, styles)))
    }

    fn lift(placement: &SubtitlePlacement, frame: SubtitleFrame) -> f64 {
        subtitle_lift(&SubtitleStyle::default(), None, CONTROLS, Some(&frame), placement)
    }

    /// The line's bottom edge above the window's bottom, in px at 720, for `sub-pos` at
    /// `100 - percent`, by libass's sums for a line laid out in the picture.
    fn styled_bottom(frame: SubtitleFrame, margin: f64, play_height: f64, percent: f64) -> f64 {
        let content = frame.height - frame.top - frame.bottom;
        let rest = frame.bottom + content * margin / play_height;
        let travel = content * (1.0 - margin / play_height);
        rest + travel * percent / 100.0
    }

    #[test]
    fn a_style_placed_high_is_not_lifted() {
        // MarginV 200 of 1080 rests 133 px up at 720: above 90 px of controls and mpv's 34.
        let high = styled(1080, &[("Default", 2, 200), ("Top", 8, 20)]);
        assert_eq!(high, SubtitlePlacement::Styled { margin: 200.0, play_height: 1080.0 });
        assert_eq!(lift(&high, filled()), 0.0);
    }

    #[test]
    fn a_style_placed_low_is_lifted_only_by_the_difference() {
        // MarginV 90 of 1080 rests 60 px up at 720.
        let placement = styled(1080, &[("Default", 2, 90), ("Signs", 8, 0)]);
        let lifted = lift(&placement, filled());
        let full = CONTROLS * 100.0;
        assert!(lifted > 0.0 && lifted < full, "{lifted} is part of {full}");
        // It lands on the line plain subtitles rise to, give or take the rounding to 0.1.
        let bottom = styled_bottom(filled(), 90.0, 1080.0, lifted);
        let line = 90.0 + 34.0;
        assert!((bottom - line).abs() < 1.0, "{bottom} px against {line}");
    }

    #[test]
    fn the_lowest_bottom_style_decides_and_top_ones_do_not_count() {
        let placement = styled(720, &[("Default", 2, 150), ("Low", 1, 10), ("Sign", 8, 0)]);
        assert_eq!(placement, SubtitlePlacement::Styled { margin: 10.0, play_height: 720.0 });
    }

    #[test]
    fn a_letterboxed_picture_gives_its_bar_to_the_room() {
        // MarginV 20 of 288 rests 50 px up when the picture fills the window, and 129 px up
        // over a 92 px letterbox bar.
        let placement = styled(288, &[("Default", 2, 20)]);
        assert!(lift(&placement, filled()) > 0.0);
        assert_eq!(lift(&placement, letterboxed()), 0.0);
    }

    #[test]
    fn plain_subtitles_rise_as_before() {
        let plain = subtitle_placement(Some("subrip"), None);
        assert_eq!(plain, SubtitlePlacement::Plain);
        assert_eq!(subtitle_placement(Some("webvtt"), None), SubtitlePlacement::Plain);
        // Laid out over the window, so a letterbox bar is no room for them.
        for frame in [filled(), letterboxed()] {
            assert_eq!(lift(&plain, frame), CONTROLS * 100.0);
        }
    }

    #[test]
    fn stripped_ass_is_plain() {
        let placement = styled(1080, &[("Default", 2, 200)]);
        let style =
            SubtitleStyle { styled: StyledSubtitles::Strip, ..SubtitleStyle::default() };
        let lifted = subtitle_lift(&style, None, CONTROLS, Some(&filled()), &placement);
        assert_eq!(lifted, CONTROLS * 100.0);
    }

    #[test]
    fn restyled_ass_keeps_its_margin_over_the_window() {
        // Over the window, so the letterbox bar is no room, but the margin still is.
        let placement = styled(1080, &[("Default", 2, 200)]);
        let style =
            SubtitleStyle { styled: StyledSubtitles::Restyle, ..SubtitleStyle::default() };
        let lift = |frame| subtitle_lift(&style, None, CONTROLS, Some(&frame), &placement);
        assert_eq!(lift(filled()), 0.0);
        let low = styled(288, &[("Default", 2, 10)]);
        let letterboxed = subtitle_lift(&style, None, CONTROLS, Some(&letterboxed()), &low);
        assert!(letterboxed > 0.0);
    }

    #[test]
    fn unknown_placement_takes_the_whole_height() {
        let full = CONTROLS * 100.0;
        for placement in [
            subtitle_placement(Some("hdmv_pgs_subtitle"), None),
            subtitle_placement(Some("ass"), None),
            subtitle_placement(Some("ass"), Some("not a header")),
            // No bottom style: what would move is not known.
            styled(1080, &[("Top", 8, 20)]),
            subtitle_placement(None, None),
        ] {
            assert_eq!(placement, SubtitlePlacement::Unknown);
            assert_eq!(lift(&placement, filled()), full);
        }
        // Nor without the frame's size.
        let high = styled(1080, &[("Default", 2, 200)]);
        assert_eq!(subtitle_lift(&SubtitleStyle::default(), None, CONTROLS, None, &high), full);
        let empty = SubtitleFrame::default();
        assert_eq!(
            subtitle_lift(&SubtitleStyle::default(), None, CONTROLS, Some(&empty), &high),
            full
        );
    }

    #[test]
    fn controls_away_lift_nothing() {
        let low = styled(288, &[("Default", 2, 0)]);
        assert_eq!(
            subtitle_lift(&SubtitleStyle::default(), None, 0.0, Some(&filled()), &low),
            0.0
        );
    }

    #[test]
    fn a_raise_that_clears_the_controls_leaves_nothing_to_lift() {
        let plain = SubtitlePlacement::Plain;
        let raised = SubtitleStyle { raise: 20, ..SubtitleStyle::default() };
        assert_eq!(subtitle_lift(&raised, None, CONTROLS, Some(&filled()), &plain), 0.0);
        // A smaller one counts toward the lift.
        let some = SubtitleStyle { raise: 6, ..SubtitleStyle::default() };
        let lifted = subtitle_lift(&some, None, CONTROLS, Some(&filled()), &plain);
        assert!(lifted > 0.0 && lifted < CONTROLS * 100.0, "{lifted}");
    }

    #[test]
    fn larger_plain_subtitles_sit_higher_and_need_less() {
        // mpv scales the plain margin with sub-scale.
        let plain = SubtitlePlacement::Plain;
        let large = SubtitleStyle { size: 200, ..SubtitleStyle::default() };
        let lifted = subtitle_lift(&large, None, CONTROLS, Some(&filled()), &plain);
        assert!(lifted < CONTROLS * 100.0, "{lifted}");
    }

    #[test]
    fn the_lift_stays_within_half_the_picture() {
        let plain = SubtitlePlacement::Plain;
        assert_eq!(
            subtitle_lift(&SubtitleStyle::default(), None, 0.9, Some(&filled()), &plain),
            MOST
        );
    }

    #[test]
    fn ssa_headers_and_missing_play_res_follow_libass() {
        // SSA numbers alignment 1 to 3 for the bottom too, and 8 is VSFilter's bottom right.
        let ssa = "[Script Info]\nScriptType: v4.00\n\n[V4 Styles]\nFormat: Name, Fontname, \
                   Fontsize, PrimaryColour, SecondaryColour, TertiaryColour, BackColour, Bold, \
                   Italic, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, \
                   AlphaLevel, Encoding\nStyle: Default,Sans,20,16777215,65535,65535,0,0,0,1,2,\
                   0,6,30,30,90,0,0\nStyle: Corner,Sans,20,16777215,65535,65535,0,0,0,1,2,0,8,\
                   30,30,12,0,0\n";
        // No PlayResX or PlayResY: libass takes 384×288.
        assert_eq!(
            subtitle_placement(Some("ass"), Some(ssa)),
            SubtitlePlacement::Styled { margin: 12.0, play_height: 288.0 }
        );
        // PlayResX alone: three quarters of it.
        let wide = header(0, &[("Default", 2, 30)]).replace("PlayResX: 0", "PlayResX: 1920");
        let wide = wide.replace("PlayResY: 0\n", "");
        assert_eq!(
            subtitle_placement(Some("ass"), Some(&wide)),
            SubtitlePlacement::Styled { margin: 30.0, play_height: 1440.0 }
        );
    }

    #[test]
    fn a_header_with_windows_line_ends_and_a_short_format_parses() {
        let text = "[Script Info]\r\nPlayResY: 360\r\n\r\n[V4+ Styles]\r\n\
                    Format: Name, Alignment, MarginV\r\nStyle: Default, 2, 25\r\n";
        assert_eq!(
            subtitle_placement(Some("ass"), Some(text)),
            SubtitlePlacement::Styled { margin: 25.0, play_height: 360.0 }
        );
    }
}

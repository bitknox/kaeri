using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// The subtitles above the bottom bar while the controls show, and back down with them, as the
/// Mac and Linux players have them: mpv's <c>sub-pos</c>, which libass applies to the dialogue at
/// the bottom of ASS subtitles and leaves signs where they are. The move runs with the controls'
/// fade, rising over fade-in (200 ms, decelerating) and settling over fade-out (300 ms,
/// accelerating); with animation effects off it is at once.
/// </summary>
public sealed class SubtitleLift(Func<TimeSpan> clock)
{
    /// <summary>mpv's own place for them, at the bottom.</summary>
    public const double Bottom = 100;

    double from = Bottom, to = Bottom;
    TimeSpan start, length;
    double? sent;

    /// <summary>
    /// Where <c>sub-pos</c> puts the subtitles clear of a bar <paramref name="bar"/> tall over a
    /// picture <paramref name="height"/> tall, in tenths of a percent; never more than half way up.
    /// </summary>
    public static double Position(double bar, double height) =>
        height > 0 ? Math.Round(Bottom - Math.Clamp(bar / height, 0, 0.5) * 100, 1) : Bottom;

    /// <summary>
    /// Where <c>sub-pos</c> puts the subtitles only as far up as they need to clear the bar: the
    /// engine's <c>subtitle_lift</c> for mpv's <paramref name="frame"/> (<c>osd-dimensions</c>) and
    /// the track's <paramref name="placement"/>, on top of the style's own raise, which
    /// <c>SubtitleStyling.Position</c> takes off again. Subtitles that already sit clear stay put;
    /// without a style, a frame or a placement, the whole bar, as <see cref="Position(double, double)"/>.
    /// </summary>
    public static double Position(double bar, double height, SubtitleStyle? style, SystemCaptionStyle? system,
        SubtitleFrame? frame, SubtitlePlacement? placement)
    {
        if (style is null || height <= 0 || bar <= 0)
            return Position(bar, height);
        var lift = KaeriFfiMethods.SubtitleLift(style, system, bar / height, frame, placement ?? new SubtitlePlacement.Unknown());
        return Math.Round(Bottom - lift, 1);
    }

    /// <summary>Where they are headed.</summary>
    public double Target => to;

    /// <summary>Still on the way there.</summary>
    public bool Moving => from != to && clock() - start < length;

    /// <summary>
    /// Head for <paramref name="target"/> over <paramref name="time"/>, from wherever the move
    /// so far has them; zero is at once.
    /// </summary>
    public void Head(double target, TimeSpan time)
    {
        if (target == to)
            return;
        from = At();
        to = target;
        start = clock();
        length = time;
    }

    /// <summary>Where they are now: eased out on the way up, eased in on the way down.</summary>
    public double At()
    {
        if (length <= TimeSpan.Zero || from == to)
            return to;
        var t = Math.Clamp((clock() - start) / length, 0, 1);
        var eased = to < from ? 1 - Math.Pow(1 - t, 3) : Math.Pow(t, 3);
        return from + (to - from) * eased;
    }

    /// <summary>The value to set now, or null when mpv already has it.</summary>
    public double? Next()
    {
        var now = Math.Round(At(), 1);
        if (now == sent)
            return null;
        sent = now;
        return now;
    }

    /// <summary>
    /// The player closed: mpv keeps <c>sub-pos</c> from file to file, so the next playback is
    /// told again, at once, rather than eased from wherever this one left it.
    /// </summary>
    public void Forget()
    {
        from = to = Bottom;
        length = TimeSpan.Zero;
        sent = null;
    }
}

using System.Diagnostics;
using Kaeri.App;
using Kaeri.Engine.Ffi;
using Kaeri.Engine.Model;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;

namespace Kaeri.Windows.Player;

// The subtitles above the bottom bar while the controls show, as on the Mac and Linux.
public sealed partial class PlayerView
{
    static readonly Stopwatch liftClock = Stopwatch.StartNew();
    readonly SubtitleLift lift = new(() => liftClock.Elapsed);
    DispatcherQueueTimer? lifting;
    /// <summary>The playback the engine's ops are for now, which a <c>sub-pos</c> of ours joins.</summary>
    ulong playback;

    /// <summary>
    /// The subtitles up above the bottom bar while the controls show, down to mpv's own place
    /// when they go: with the controls' fade, or at once for a new size or a new playback.
    /// </summary>
    void LiftSubtitles(bool animated = true)
    {
        var up = Showing && chromeShown;
        var target = up && BottomBar.ActualHeight > 0
            ? SubtitleLift.Position(BottomBar.ActualHeight, VideoPanel.ActualHeight, ctx?.Known.Settings?.SubtitleStyle,
                SubtitleStyling.System, subtitleFrame, subtitlePlacement)
            : SubtitleLift.Bottom;
        // A move across the picture, so with animation effects off it is at once; the fade stays.
        var time = !animated || !Motion.Enabled ? TimeSpan.Zero : up ? Motion.FadeInTime : Motion.FadeOutTime;
        lift.Head(target, time);
        SetSubtitlePosition();
        if (!lift.Moving)
            return;
        if (lifting is null)
        {
            lifting = DispatcherQueue.GetForCurrentThread().CreateTimer();
            lifting.Interval = TimeSpan.FromMilliseconds(30);
            lifting.Tick += (_, _) =>
            {
                SetSubtitlePosition();
                if (!lift.Moving)
                {
                    SetSubtitlePosition();
                    lifting.Stop();
                }
            };
        }
        lifting.Start();
    }

    /// <summary>Tells mpv where the lift has the subtitles now, when that has changed.</summary>
    void SetSubtitlePosition()
    {
        if (player is null || !Showing)
            return;
        ulong now;
        lock (loads)
            now = playback;
        if (now == 0 || lift.Next() is not { } position)
            return;
        position = SubtitleStyling.Position(position, ctx?.Known.Settings?.SubtitleStyle);
        player.Apply(now, [new PlayerOp.Set("sub-pos", new MpvValue.Double(position))]);
    }

    SubtitleStyle? styled;

    /// <summary>
    /// Hands mpv the engine's subtitle options for the style in effect and the system's captions.
    /// With <paramref name="onlyIfChanged"/> it does nothing when the style is the one mpv has.
    /// </summary>
    internal void ApplySubtitleStyle(bool onlyIfChanged = false)
    {
        if (player is null || ctx?.Known.Settings?.SubtitleStyle is not { } style)
            return;
        ulong now;
        lock (loads)
            now = playback;
        if (now == 0 || (onlyIfChanged && style == styled))
            return;
        styled = style;
        player.Apply(now, SubtitleStyling.Ops(style));
        // The raise is part of sub-pos, which the lift owns.
        lift.Forget();
        LiftSubtitles(animated: false);
    }

    /// <summary>The player closed: the next one starts from mpv's own place, told again.</summary>
    void DropSubtitleLift()
    {
        lifting?.Stop();
        lift.Forget();
        subtitleFrame = null;
        subtitlePlacement = null;
    }

    void OnBottomBarSized(object sender, SizeChangedEventArgs e) => LiftSubtitles(animated: false);

    /// <summary>mpv's <c>osd-dimensions</c> and where the subtitle track's lines rest, as last said.</summary>
    SubtitleFrame? subtitleFrame;
    SubtitlePlacement? subtitlePlacement;

    /// <summary>
    /// A new subtitle track, size or full screen, as mpv reports it: the lift is worked out again,
    /// at once.
    /// </summary>
    void SubtitlesPlaced(FfiPlayerState state)
    {
        if (Equals(state.SubtitleFrame, subtitleFrame) && Equals(state.SubtitlePlacement, subtitlePlacement))
            return;
        subtitleFrame = state.SubtitleFrame;
        subtitlePlacement = state.SubtitlePlacement;
        LiftSubtitles(animated: false);
    }
}

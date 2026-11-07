using Kaeri.App;
using Kaeri.Engine.Model;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;

namespace Kaeri.Windows;

/// <summary>
/// The hover card's trailer, one at a time, as on the Mac: the pointer resting on a cover cues its
/// trailer 300 ms before the card opens, loading it held (silent and unseen) in the pool's dock;
/// the pointer leaving the cover before then lets it go; the card that opens takes it under its
/// art and shows it once it has grown and a frame plays. The rules are <see cref="HoverCue"/>'s.
/// </summary>
static class HoverTrailer
{
    static readonly HoverCue cue = new();
    static TrailerStage? stage;
    /// <summary>The title a queued cue is for, until the open card closes.</summary>
    static (Entry Entry, string Subtitles)? queued;

    /// <summary>The pointer has rested on <paramref name="entry"/>'s cover, and its trailer may play.</summary>
    public static void Cue(Entry entry, string subtitles)
    {
        var step = cue.Cue(entry.Id);
        if (step is HoverTrailerStep.Start or HoverTrailerStep.Replace)
            Begin(entry, subtitles);
        else if (cue.Queued == entry.Id)
            queued = (entry, subtitles);
    }

    /// <summary>The pointer left title <paramref name="id"/>'s cover before its card opened.</summary>
    public static void Uncue(uint id)
    {
        if (cue.Uncue(id) == HoverTrailerStep.Stop)
            Drop();
    }

    /// <summary>A scroll: the cued cover moves from under the pointer.</summary>
    public static void Scrolled()
    {
        if (cue.Scrolled() == HoverTrailerStep.Stop)
            Drop();
    }

    /// <summary>
    /// <paramref name="entry"/>'s card opened, with <paramref name="video"/> (an area of
    /// <paramref name="w"/> by <paramref name="h"/>) for the trailer under <paramref name="art"/>:
    /// the trailer cued for it, or one begun now, still held; none when <paramref name="plays"/> is
    /// false or nothing could start.
    /// </summary>
    public static TrailerStage? Open(Entry entry, bool plays, string subtitles, Panel video, UIElement art, double w, double h)
    {
        queued = null;
        switch (cue.Open(entry.Id, stage is { Active: true }, plays))
        {
            case HoverTrailerStep.Stop:
                Drop();
                return null;
            case HoverTrailerStep.Start or HoverTrailerStep.Replace:
                Begin(entry, subtitles);
                break;
            case HoverTrailerStep.None:
                return null;
        }
        if (stage is null)
        {
            cue.Reset();
            return null;
        }
        stage.Adopt(video, art, w, h);
        return stage;
    }

    /// <summary>
    /// The card that took <paramref name="taken"/> closed: its trailer goes back at once, as the card
    /// goes over it, and a cue that waited for it starts.
    /// </summary>
    public static void Closed(TrailerStage? taken)
    {
        if (taken is null)
            return;
        if (taken != stage)
        {
            taken.Stop(fade: false);
            return;
        }
        var step = cue.Closed();
        Drop();
        var next = queued;
        queued = null;
        if (step is HoverTrailerStep.Start or HoverTrailerStep.Replace && next is { } wait && wait.Entry.Id == cue.Id)
            Begin(wait.Entry, wait.Subtitles);
        else if (cue.Id is not null)
            cue.Reset();
    }

    /// <summary>Every trailer is let go of: nothing stays cued or waits.</summary>
    public static void StopAll()
    {
        queued = null;
        cue.Reset();
        Drop();
    }

    static void Begin(Entry entry, string subtitles)
    {
        Drop();
        if (TrailerPool.Dock() is not { } dock)
            return;
        var next = new TrailerStage(dock, EmbedStyle.Banner, art: null, subtitles);
        next.Size(HoverPreview.Width, HoverPreview.Width * 9 / 16);
        next.Hold();
        if (next.Start(entry))
            stage = next;
    }

    static void Drop()
    {
        var gone = stage;
        stage = null;
        gone?.Stop(fade: false);
    }
}

using Kaeri.Engine.Core;
using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>
/// What the starting screen says and offers, from the engine's steps and the clock: no controls,
/// so every rule of the design is tested here. The player draws a <see cref="StartFrame"/>.
/// Times are the time since the screen opened, passed in by the caller.
/// </summary>
public enum StartAction { Cancel, FromBeginning, ChooseSource, Details, Skip, ShowDownloads, ChooseAnother, TryAgain, FixMatch, AddStore, Close }

/// <summary>How a button is drawn: flat, the tonal pill, or the amber of a failure's primary.</summary>
public enum StartLook { Quiet, Tonal, Primary }

public sealed record StartButton(StartAction Action, string Label, StartLook Look);

/// <summary>The mark in front of a Details row.</summary>
public enum StartMark { Done, Empty, Now, Rest }

public sealed record StartRow(StartMark Mark, string Name, string Note, string Time, bool Skip);

/// <summary>What the screen shows now.</summary>
public sealed record StartFrame(
    string Step,
    string Key,
    string Lead,
    long? Seconds,
    IReadOnlyList<StartButton> Buttons,
    (string Title, string Reason)? Failure,
    uint? Percent,
    IReadOnlyList<StartRow> Rows,
    Translation? Translation,
    bool DetailsOpen)
{
    /// <summary>The detail line as one string: the facts, then the seconds.</summary>
    public string Detail => (Lead.Length == 0, Seconds) switch
    {
        (true, null) => "",
        (true, { } s) => T.WindowsStartSeconds(s),
        (false, null) => Lead,
        (false, { } s) => $"{Lead}{StartScreen.Join}{T.WindowsStartSeconds(s)}",
    };
}

/// <summary>One start, from the play being staged to its first frame or its end.</summary>
public sealed class StartScreen(uint id, string episode, double? resume)
{
    /// <summary>The shortest a step line's text stays before a newer one takes its place.</summary>
    public static readonly TimeSpan Hold = TimeSpan.FromMilliseconds(600);
    /// <summary>When the detail line appears, and Play from the beginning.</summary>
    public static readonly TimeSpan DetailAt = TimeSpan.FromSeconds(1);
    /// <summary>When the seconds join the detail line, and Choose source and Details join the row.</summary>
    public static readonly TimeSpan ToolsAt = TimeSpan.FromSeconds(3);
    /// <summary>How long one source or stream is tried before Skip is offered for it.</summary>
    public static readonly TimeSpan SkipAfter = TimeSpan.FromSeconds(8);
    /// <summary>When the step line says plainly that it is still waiting.</summary>
    public static readonly TimeSpan SlowAt = TimeSpan.FromSeconds(20);

    internal const string Join = " · "; // i18n-skip

    sealed record Asking(string Name, uint Place, uint Total, bool Pinned);
    sealed record Seen(string Name, SourceMiss? Miss, TimeSpan? Took);
    sealed record Held(string Text, string Key, TimeSpan Since);

    public uint Id { get; } = id;
    public string Episode { get; } = episode;
    bool fromBeginning;
    Held shown = new(T.StartLookingUpTitle, "lookup", TimeSpan.Zero);
    (string Text, string Key)? waiting;
    TimeSpan rungAt;
    TimeSpan? lookedUpAt;
    Asking? asking;
    readonly List<Seen> seen = [];
    List<string> resting = [];
    (string Name, SourceMiss Miss)? lastMiss;
    uint? peers;
    ulong? rate;
    // Some(None) is "finding peers" with no source named.
    (bool Active, string? Name) findingPeers;
    (string? Source, Translation Translation)? found;
    (uint Attempt, uint Total)? stream;
    uint? percent;
    bool offline;
    (StartFailure Failure, bool Open)? failure;
    Held? beforeOffline;
    bool announcedSlow;
    bool allNoMatch = true;

    public bool DetailsOpen { get; set; }
    public bool Failed => failure is not null;
    public bool Offline => offline;

    public long Number => long.TryParse(Episode, out var n) ? n : 0;

    /// <summary>The viewer asked to begin at 0: the Resuming fact and its button go.</summary>
    public void BeginFromStart() => fromBeginning = true;

    /// <summary>Where the episode was left, when it resumes there: a PlaybackStaged can tell it after the screen opened.</summary>
    public double? ResumeAt { get; set; } = resume;

    double? ResumePoint => fromBeginning ? null : ResumeAt;

    /// <summary>
    /// Say <paramref name="text"/> on the step line at <paramref name="at"/>: at once when the one
    /// showing has had its time, else it waits, and a newer one during the wait replaces it.
    /// </summary>
    void Say(string text, string key, TimeSpan at)
    {
        if (shown.Text == text && shown.Key == key)
        {
            waiting = null;
            return;
        }
        var held = at - shown.Since;
        if (held < TimeSpan.Zero)
            held = TimeSpan.Zero;
        // A percentage changes in place, with no hold.
        if (held >= Hold || key == shown.Key)
        {
            shown = new Held(text, key, key == shown.Key ? shown.Since : at);
            waiting = null;
        }
        else
            waiting = (text, key);
    }

    /// <summary>The clock moved: a waiting text whose turn has come takes its place. True when the step line changed.</summary>
    public bool Tick(TimeSpan at)
    {
        if (waiting is { } w && at - shown.Since >= Hold)
        {
            shown = new Held(w.Text, w.Key, at);
            waiting = null;
            return true;
        }
        return false;
    }

    /// <summary>A step the engine said, with the line it brings.</summary>
    public void Step(StartStep step, Msg? line, TimeSpan at)
    {
        if (failure is not null)
            return;
        var text = line?.Text();
        switch (step)
        {
            case StartStep.GettingSourcesReady:
                lookedUpAt ??= at;
                break;
            case StartStep.AskingSource a:
                lookedUpAt ??= at;
                CloseAsking(at, null);
                asking = new Asking(a.Name, a.Place, a.Total, a.Pinned);
                rungAt = at;
                findingPeers = default;
                break;
            case StartStep.SourceMissed m:
                CloseAsking(at, m.Miss);
                if (m.Miss != SourceMiss.NoMatch)
                    allNoMatch = false;
                lastMiss = (m.Name, m.Miss);
                break;
            case StartStep.SourcesResting r:
                resting = [.. r.Names];
                break;
            case StartStep.FindingPeers p:
                findingPeers = (true, p.Name);
                break;
            case StartStep.Found f:
                CloseAsking(at, null);
                found = (f.Source, f.Translation);
                lastMiss = null;
                findingPeers = default;
                rungAt = at;
                break;
            case StartStep.OpeningStream o:
                stream = (o.Attempt, o.Total);
                rungAt = at;
                break;
            case StartStep.TryingAnotherStream t:
                stream = (t.Next, t.Total);
                rungAt = at;
                percent = null;
                break;
            case StartStep.Swarm s:
                peers = s.Peers;
                rate = s.BytesPerSecond;
                break;
            case StartStep.Speed sp:
                rate = sp.BytesPerSecond;
                break;
            case StartStep.Buffering b:
                percent = b.Percent;
                break;
        }
        if (offline)
            return;
        var (said, key) = step switch
        {
            StartStep.Buffering b => (T.PlayerStartBufferingPercent(Wording.Percent(b.Percent)), "buffering"),
            StartStep.LookingUpTitle => (text ?? "", "lookup"),
            StartStep.GettingSourcesReady => (text ?? "", "sources"),
            StartStep.AskingSource => (text ?? "", "asking"),
            StartStep.FindingPeers => (text ?? "", "peers"),
            StartStep.Found => (text ?? "", "opening"),
            StartStep.TryingAnotherStream => (text ?? "", "another"),
            _ => ("", ""),
        };
        if (said.Length > 0)
            Say(said, key, at);
    }

    void CloseAsking(TimeSpan at, SourceMiss? miss)
    {
        if (asking is { } a)
        {
            seen.Add(new Seen(a.Name, miss, at - rungAt));
            asking = null;
        }
        else if (miss is { } m && seen.Count > 0 && seen[^1].Miss is null)
            seen[^1] = seen[^1] with { Miss = m };
    }

    /// <summary>The device went offline or came back.</summary>
    public void Connectivity(bool nowOffline, TimeSpan at)
    {
        if (nowOffline && !offline)
        {
            beforeOffline = shown;
            offline = true;
            waiting = null;
            shown = new Held(T.PlayerStartOffline, "offline", at);
        }
        else if (!nowOffline && offline)
        {
            offline = false;
            if (beforeOffline is { } held)
            {
                shown = held with { Since = at };
                beforeOffline = null;
            }
        }
    }

    /// <summary>The start ended without playing.</summary>
    public void Fail(StartFailure why)
    {
        failure = (why, true);
        waiting = null;
    }

    (string Text, string Key) StepText(TimeSpan at)
    {
        if (offline)
            return (shown.Text, shown.Key);
        if (at >= SlowAt && shown.Key == "asking" && asking is { } a)
            return (T.PlayerStartStillWaiting(a.Name), "slow");
        return (shown.Text, shown.Key);
    }

    string Lead()
    {
        if (offline)
            return T.PlayerStartOfflineDetail(Episode);
        List<string> facts = [];
        switch (shown.Key)
        {
            case "asking" or "slow":
                if (lastMiss is { } miss)
                    facts.Add(MissText(miss.Name, miss.Miss, Episode));
                else if (asking is { Pinned: true })
                    facts.Add(T.PlayerStartPinned);
                if (asking is { Total: > 1 } a)
                    facts.Add(T.PlayerStartPlace(a.Place, a.Total));
                break;
            case "peers":
                if (peers is { } p)
                    facts.Add(T.PlayerStartPeers(p));
                else if (findingPeers.Name is { } from)
                    facts.Add(T.PlayerStartFrom(from));
                break;
            case "buffering":
                if (peers is { } bp)
                    facts.Add(T.PlayerStartPeers(bp));
                if (rate is > 0)
                    facts.Add(T.PlayerStartSpeed(Wording.Bytes(rate.Value)));
                break;
            case "another":
                facts.Add(T.PlayerStartNoVideo);
                if (stream is { } s)
                    facts.Add(T.PlayerStartStreamPlace(s.Attempt, s.Total));
                break;
            case "opening":
                if (found is { Source: { } name })
                    facts.Add(T.PlayerStartFrom(name));
                if (peers is { } op)
                    facts.Add(T.PlayerStartPeers(op));
                if (rate is > 0)
                    facts.Add(T.PlayerStartSpeed(Wording.Bytes(rate.Value)));
                break;
        }
        if (facts.Count == 0 && ResumePoint is { } position)
            facts.Add(T.PlayerStartResuming(Wording.Clock(position)));
        return string.Join(Join, facts);
    }

    /// <summary>Whether another source or stream is left to skip to, and what Skip would say.</summary>
    StartButton? Skippable()
    {
        switch (shown.Key)
        {
            case "asking" or "slow":
                return asking is { } a && a.Place < a.Total
                    ? new StartButton(StartAction.Skip, T.PlayerStartSkipSource(a.Name), StartLook.Tonal)
                    : null;
            case "opening" or "another" or "buffering":
                return stream is { } s && s.Attempt < s.Total
                    ? new StartButton(StartAction.Skip, T.PlayerStartSkipStream, StartLook.Tonal)
                    : null;
            default:
                return null;
        }
    }

    List<StartButton> Buttons(TimeSpan at)
    {
        if (failure is { } f)
        {
            StartButton again = new(StartAction.TryAgain, T.ActionTryAgain, StartLook.Tonal);
            StartButton details = new(StartAction.Details, T.PlayerStartDetails, StartLook.Quiet);
            StartButton close = new(StartAction.Close, T.ActionClose, StartLook.Quiet);
            return f.Failure.Why switch
            {
                NoSourceWhy.NotAired => [new(StartAction.Close, T.ActionClose, StartLook.Tonal)],
                NoSourceWhy.NoSources => [new(StartAction.AddStore, T.StoresAddTitle, StartLook.Primary), again, close],
                NoSourceWhy.NotFound when allNoMatch && seen.Count > 0 =>
                [
                    new(StartAction.FixMatch, T.PlayerStartFixMatch, StartLook.Primary),
                    new(StartAction.ChooseAnother, T.PlayerChooseAnotherSource, StartLook.Tonal),
                    details,
                    close,
                ],
                _ =>
                [
                    new(StartAction.ChooseAnother, T.PlayerChooseAnotherSource, StartLook.Primary),
                    again,
                    details,
                    close,
                ],
            };
        }
        List<StartButton> row = [new(StartAction.Cancel, T.ActionCancel, StartLook.Quiet)];
        if (offline)
        {
            row.Add(new(StartAction.ShowDownloads, T.StateOfflineShowDownloads, StartLook.Tonal));
            return row;
        }
        if (at >= DetailAt && ResumePoint is not null)
            row.Add(new(StartAction.FromBeginning, T.PlayerStartFromBeginning, StartLook.Tonal));
        if (at >= ToolsAt)
        {
            row.Add(new(StartAction.ChooseSource, T.PlayerChooseSource, StartLook.Tonal));
            row.Add(new(StartAction.Details, T.PlayerStartDetails, StartLook.Quiet));
        }
        if (at - rungAt >= SkipAfter && Skippable() is { } skip)
            row.Add(skip);
        return row;
    }

    List<StartRow> Rows(TimeSpan at)
    {
        List<StartRow> rows = [];
        if (lookedUpAt is { } done)
            rows.Add(new(StartMark.Done, T.WindowsStartRowLookedUp, "", SecondsText(done), false));
        foreach (var s in seen)
            rows.Add(new(
                s.Miss is null ? StartMark.Done : StartMark.Empty,
                s.Name,
                s.Miss is { } m ? MissNote(m, Episode) : "",
                s.Took is { } t ? SecondsText(t) : "",
                false));
        if (asking is { } a && failure is null)
            rows.Add(new(
                StartMark.Now,
                a.Name,
                T.WindowsStartRowAsking,
                SecondsText(at - rungAt),
                a.Place < a.Total && at - rungAt >= SkipAfter));
        foreach (var name in resting)
            rows.Add(new(StartMark.Rest, name, T.PlayerStartResting, "", false));
        return rows;
    }

    /// <summary>Everything the screen shows at <paramref name="at"/>.</summary>
    public StartFrame Frame(TimeSpan at)
    {
        var (step, key) = StepText(at);
        (string, string)? failed = failure is { } f
            ? (T.PlayerFailedTitle(Number), f.Failure.Text.Text())
            : null;
        var lead = failed is not null || (at < DetailAt && !offline) ? "" : Lead();
        long? seconds = at >= ToolsAt && failed is null && !offline ? (long)at.TotalSeconds : null;
        return new StartFrame(
            step,
            key,
            lead,
            seconds,
            Buttons(at),
            failed,
            shown.Key == "buffering" ? percent : null,
            Rows(at),
            found?.Translation,
            DetailsOpen);
    }

    /// <summary>The moment, once, to tell a screen reader the start is slow.</summary>
    public string? SlowAnnouncement(TimeSpan at)
    {
        if (announcedSlow || at < SkipAfter || failure is not null || offline)
            return null;
        announcedSlow = true;
        return T.WindowsStartStillStarting;
    }

    static string SecondsText(TimeSpan took) => T.WindowsStartSeconds((long)Math.Round(took.TotalSeconds, MidpointRounding.AwayFromZero));

    static string MissText(string name, SourceMiss miss, string episode) => miss switch
    {
        SourceMiss.NoMatch => T.PlayerStartMissNoMatch(name),
        SourceMiss.NoEpisode => T.PlayerStartMissNoEpisode(name, episode),
        SourceMiss.Failed => T.PlayerStartMissFailed(name),
        SourceMiss.TimedOut => T.PlayerStartMissTimedOut(name),
        _ => T.PlayerStartMissSkipped(name),
    };

    /// <summary>A Details row's note for a source that missed: the sentence without the name in front.</summary>
    static string MissNote(SourceMiss miss, string episode) => miss switch
    {
        SourceMiss.NoMatch => T.WindowsStartRowNoMatch,
        SourceMiss.NoEpisode => T.WindowsStartRowNoEpisode(episode),
        SourceMiss.Failed => T.WindowsStartRowFailed,
        SourceMiss.TimedOut => T.WindowsStartRowTimedOut,
        _ => T.WindowsStartRowSkipped,
    };
}

/// <summary>The ground under the starting screen: 55% of the dark, or 80% over art that is bright.</summary>
public static class StartGround
{
    /// <summary>Art brighter than this, by <see cref="Luminance"/>, sits under the deeper ground.</summary>
    public const double Bright = 0.15;

    public const double Normal = 0.55;
    public const double Deep = 0.80;

    /// <summary>The ground's opacity for art of this luminance.</summary>
    public static double For(double luminance) => luminance > Bright ? Deep : Normal;

    /// <summary>
    /// The mean relative luminance of the art's centre band: the middle 60% of the width and half
    /// of the height, from 8-bit B, G, R, A pixels, in linear sRGB as the board measured it.
    /// </summary>
    public static double Luminance(ReadOnlySpan<byte> bgra, int width, int height)
    {
        if (width <= 0 || height <= 0)
            return 0;
        var stride = width * 4;
        var step = Math.Max(1, width / 64);
        static double Linear(byte v)
        {
            var c = v / 255.0;
            return c <= 0.04045 ? c / 12.92 : Math.Pow((c + 0.055) / 1.055, 2.4);
        }
        double sum = 0;
        var n = 0;
        for (var y = height / 4; y < height * 3 / 4; y += step)
            for (var x = width * 2 / 10; x < width * 8 / 10; x += step)
            {
                var i = y * stride + x * 4;
                if (i + 2 >= bgra.Length)
                    continue;
                sum += 0.2126 * Linear(bgra[i + 2]) + 0.7152 * Linear(bgra[i + 1]) + 0.0722 * Linear(bgra[i]);
                n++;
            }
        return n == 0 ? 0 : sum / n;
    }
}

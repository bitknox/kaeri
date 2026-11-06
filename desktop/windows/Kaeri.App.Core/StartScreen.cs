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

/// <summary>
/// A button of the starting screen. <paramref name="Shown"/> is false for a slot kept as a
/// placeholder: not shown, no click or focus, and its room stays so the buttons after it don't move.
/// </summary>
public sealed record StartButton(StartAction Action, string Label, StartLook Look, bool Shown = true);

/// <summary>The mark in front of a Details row.</summary>
public enum StartMark { Done, Empty, Now, Rest }

/// <summary>A Details row, and what its Skip says when it carries one.</summary>
public sealed record StartRow(StartMark Mark, string Name, string Note, string Time, string? Skip);

/// <summary>Where a start is. Waiting is offline: the engine asks no source until it is back.</summary>
public enum StartPhase { Lookup, Sources, Asking, Peers, Opening, Another, Buffering, Waiting }

/// <summary>What the screen shows now.</summary>
public sealed record StartFrame(
    string Step,
    StartPhase Phase,
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
        (true, { } s) => T.PlayerStartSeconds(s),
        (false, null) => Lead,
        (false, { } s) => $"{Lead}{StartScreen.Join}{T.PlayerStartSeconds(s)}",
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
    /// <summary>How long one source or stream is tried before Skip is offered for it, and how long a start waits before a screen reader hears it is still starting.</summary>
    public static readonly TimeSpan SkipAfter = TimeSpan.FromSeconds(8);
    /// <summary>When the step line says plainly that it is still waiting.</summary>
    public static readonly TimeSpan SlowAt = TimeSpan.FromSeconds(20);

    internal const string Join = " · "; // i18n-skip

    sealed record Asking(string Name, uint Place, uint Total, bool Pinned);
    sealed record Seen(string Name, SourceMiss? Miss, TimeSpan? Took);
    sealed record Held(string Text, StartPhase Phase, TimeSpan Since);

    public uint Id { get; } = id;
    public string Episode { get; } = episode;
    bool fromBeginning;
    Held shown = new(T.StartLookingUpTitle, StartPhase.Lookup, TimeSpan.Zero);
    (string Text, StartPhase Phase)? waiting;
    /// <summary>Since when the start has been under way: the screen opening, or the end of a wait for a connection.</summary>
    TimeSpan activeSince;
    /// <summary>Since when the rung (the source asked, the stream opened) has been tried.</summary>
    TimeSpan rungAt;
    TimeSpan? lookedUpAt;
    Asking? asking;
    readonly List<Seen> seen = [];
    List<string> resting = [];
    (string Name, SourceMiss Miss)? lastMiss;
    uint? peers;
    ulong? rate;
    // Active with no name is "finding peers" with no source named.
    (bool Active, string? Name) findingPeers;
    (string? Source, Translation Translation)? found;
    (uint Attempt, uint Total)? stream;
    bool noVideo;
    uint? percent;
    StartFailure? failure;
    bool announcedSlow;

    public bool DetailsOpen { get; set; }
    public bool Failed => failure is not null;

    /// <summary>Whether the start is waiting for a connection.</summary>
    public bool Offline => shown.Phase == StartPhase.Waiting;

    /// <summary>Where the episode was left, when it resumes there: a PlaybackStaged can tell it after the screen opened.</summary>
    public double? ResumeAt { get; set; } = resume;

    /// <summary>Whether the start resumes: the room of Play from the beginning is kept from the start only then.</summary>
    public bool Resumes => ResumeAt is not null;

    /// <summary>The viewer asked to begin at 0: the detail line says so and the button is spent.</summary>
    public void BeginFromStart() => fromBeginning = true;

    static TimeSpan Since(TimeSpan at, TimeSpan from) => at > from ? at - from : TimeSpan.Zero;

    /// <summary>
    /// Say <paramref name="text"/> on the step line at <paramref name="at"/>: at once when the one
    /// showing has had its time, else it waits, and a newer one during the wait replaces it.
    /// </summary>
    void Say(string text, StartPhase phase, TimeSpan at)
    {
        if (shown.Text == text)
        {
            shown = shown with { Phase = phase };
            waiting = null;
            return;
        }
        // A percentage changes in place, with no hold.
        var inPlace = phase == StartPhase.Buffering && shown.Phase == StartPhase.Buffering;
        if (Since(at, shown.Since) >= Hold || inPlace)
        {
            shown = new Held(text, phase, inPlace ? shown.Since : at);
            waiting = null;
        }
        else
            waiting = (text, phase);
    }

    /// <summary>The clock moved: a waiting text whose turn has come takes its place. True when the step line changed.</summary>
    public bool Tick(TimeSpan at)
    {
        if (waiting is { } w && Since(at, shown.Since) >= Hold)
        {
            shown = new Held(w.Text, w.Phase, at);
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
        if (step is StartStep.WaitingForConnection)
        {
            // Said once per wait; the engine asks no source until it is back.
            waiting = null;
            shown = new Held(T.PlayerStartOffline, StartPhase.Waiting, at);
            return;
        }
        if (Offline)
        {
            // Back online: the start is under way from now.
            activeSince = at;
            rungAt = at;
        }
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
                noVideo = true;
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
        (string Text, StartPhase Phase)? said = step switch
        {
            StartStep.Buffering b => (T.PlayerStartBufferingPercent(Wording.Percent(b.Percent)), StartPhase.Buffering),
            // A direct stream has data coming: it buffers, with no number to give.
            StartStep.Speed when shown.Phase == StartPhase.Opening => (T.StartBuffering, StartPhase.Buffering),
            StartStep.LookingUpTitle => (text ?? "", StartPhase.Lookup),
            StartStep.GettingSourcesReady => (text ?? "", StartPhase.Sources),
            StartStep.AskingSource => (text ?? "", StartPhase.Asking),
            StartStep.FindingPeers => (text ?? "", StartPhase.Peers),
            StartStep.Found => (text ?? "", StartPhase.Opening),
            StartStep.TryingAnotherStream => (text ?? "", StartPhase.Another),
            _ => null,
        };
        if (said is { Text.Length: > 0 } s2)
            Say(s2.Text, s2.Phase, at);
    }

    void CloseAsking(TimeSpan at, SourceMiss? miss)
    {
        if (asking is { } a)
        {
            seen.Add(new Seen(a.Name, miss, Since(at, rungAt)));
            asking = null;
        }
        else if (miss is { } m && seen.Count > 0 && seen[^1].Miss is null)
            seen[^1] = seen[^1] with { Miss = m };
    }

    /// <summary>The start ended without playing.</summary>
    public void Fail(StartFailure why)
    {
        failure = why;
        waiting = null;
    }

    /// <summary>The source the wait is on, once one has been asked.</summary>
    string? WaitingFor()
    {
        var asked = asking?.Name;
        return shown.Phase switch
        {
            StartPhase.Asking => asked,
            StartPhase.Peers => (findingPeers.Active ? findingPeers.Name : null) ?? asked,
            StartPhase.Opening or StartPhase.Another or StartPhase.Buffering => found?.Source ?? asked,
            _ => null,
        };
    }

    /// <summary>The step line at <paramref name="at"/>: the engine's, and in any phase past 20 s, said plainly.</summary>
    string StepText(TimeSpan at) =>
        Since(at, activeSince) >= SlowAt && WaitingFor() is { } name ? T.PlayerStartStillWaiting(name) : shown.Text;

    /// <summary>The lead of the detail line: where the episode starts, then the facts of the step.</summary>
    string Lead()
    {
        if (Offline)
            return T.PlayerStartOfflineDetail(Episode);
        List<string> facts = [];
        if (fromBeginning)
            facts.Add(T.PlayerStartBeginning);
        else if (ResumeAt is { } position)
            facts.Add(T.PlayerStartResuming(Wording.Clock(position)));
        switch (shown.Phase)
        {
            case StartPhase.Asking when asking is { } a:
                if (a.Pinned)
                    facts.Add(T.PlayerStartPinned);
                if (lastMiss is { } miss)
                    facts.Add(MissText(miss.Name, miss.Miss, Episode));
                if (a.Total > 1)
                    facts.Add(T.PlayerStartPlace(a.Place, a.Total));
                break;
            case StartPhase.Peers:
                if (findingPeers is { Active: true, Name: { } from })
                    facts.Add(T.PlayerStartFrom(from));
                break;
            case StartPhase.Opening or StartPhase.Another or StartPhase.Buffering:
                if (noVideo)
                    facts.Add(T.PlayerStartNoVideo);
                if (peers is { } p)
                {
                    facts.Add(T.PlayerStartPeers(p));
                    if (rate is > 0)
                        facts.Add(T.PlayerStartSpeed(Speed(rate.Value)));
                }
                else if (found is { Source: { } name } && !noVideo)
                    facts.Add(T.PlayerStartFrom(name));
                if (noVideo && stream is { Total: > 1 } s)
                    facts.Add(T.PlayerStartStreamPlace(s.Attempt, s.Total));
                break;
        }
        return string.Join(Join, facts);
    }

    /// <summary>
    /// What Skip would say now, when there is another source to skip to. Only while the sources
    /// are asked: once a stream is opening the engine has handed it to the player, whose session
    /// lets a skip go, so the button would do nothing (audit D4; the engine could take it on).
    /// </summary>
    string? SkipLabel(TimeSpan at)
    {
        if (Since(at, rungAt) < SkipAfter)
            return null;
        return shown.Phase is StartPhase.Asking or StartPhase.Peers && asking is { } a && a.Place < a.Total
            ? T.PlayerStartSkipSource(a.Name)
            : null;
    }

    List<StartButton> Buttons(TimeSpan at)
    {
        if (failure is { } f)
            return FailureButtons(f);
        List<StartButton> row = [new(StartAction.Cancel, T.ActionCancel, StartLook.Quiet)];
        if (Offline)
        {
            row.Add(new(StartAction.ShowDownloads, T.StateOfflineShowDownloads, StartLook.Tonal));
            return row;
        }
        // Once pressed, its slot stays as a hidden placeholder: nothing after it moves.
        if (at >= DetailAt && ResumeAt is not null)
            row.Add(new(StartAction.FromBeginning, T.PlayerStartFromBeginning, StartLook.Tonal, Shown: !fromBeginning));
        if (at >= ToolsAt)
        {
            row.Add(new(StartAction.ChooseSource, T.PlayerChooseSource, StartLook.Tonal));
            row.Add(new(StartAction.Details, T.PlayerStartDetails, StartLook.Quiet));
        }
        if (SkipLabel(at) is { } skip)
            row.Add(new(StartAction.Skip, skip, StartLook.Tonal));
        return row;
    }

    /// <summary>The buttons of a failure: the primary first, by what went wrong.</summary>
    static List<StartButton> FailureButtons(StartFailure f)
    {
        StartButton Again(StartLook look) => new(StartAction.TryAgain, T.ActionTryAgain, look);
        StartButton Another(StartLook look) => new(StartAction.ChooseAnother, T.PlayerChooseAnotherSource, look);
        StartButton details = new(StartAction.Details, T.PlayerStartDetails, StartLook.Quiet);
        StartButton close = new(StartAction.Close, T.ActionClose, StartLook.Quiet);
        return f.Why switch
        {
            NoSourceWhy.NotAired => [new(StartAction.Close, T.ActionClose, StartLook.Tonal)],
            NoSourceWhy.NoSources => [new(StartAction.AddStore, T.StoresAddTitle, StartLook.Primary), close],
            NoSourceWhy.NoConnection =>
                [Again(StartLook.Primary), new(StartAction.ShowDownloads, T.StateOfflineShowDownloads, StartLook.Tonal), close],
            _ => f.Text switch
            {
                Msg.NoticeReason { Reason: Cause.TitleNoMatch } =>
                    [new(StartAction.FixMatch, T.PlayerStartFixMatch, StartLook.Primary), Another(StartLook.Tonal), details, close],
                Msg.NoticeReason { Reason: Cause.SourcesTimedOut } =>
                    [Again(StartLook.Primary), Another(StartLook.Tonal), details, close],
                _ => [Another(StartLook.Primary), Again(StartLook.Tonal), details, close],
            },
        };
    }

    List<StartRow> Rows(TimeSpan at)
    {
        List<StartRow> rows = [];
        if (lookedUpAt is { } done)
            rows.Add(new(StartMark.Done, T.PlayerStartLookedUp, "", SecondsText(done), null));
        foreach (var s in seen)
            rows.Add(new(
                s.Miss is null ? StartMark.Done : StartMark.Empty,
                s.Name,
                s.Miss is { } m ? MissNote(m, Episode) : "",
                s.Took is { } t ? SecondsText(t) : "",
                null));
        if (asking is { } a && failure is null)
            rows.Add(new(
                StartMark.Now,
                a.Name,
                T.PlayerStartRowAsking,
                SecondsText(Since(at, rungAt)),
                a.Place < a.Total && Since(at, rungAt) >= SkipAfter ? T.PlayerStartSkipSource(a.Name) : null));
        foreach (var name in resting)
            rows.Add(new(StartMark.Rest, name, T.PlayerStartResting, "", null));
        return rows;
    }

    /// <summary>Everything the screen shows at <paramref name="at"/>.</summary>
    public StartFrame Frame(TimeSpan at)
    {
        (string, string)? failed = null;
        if (failure is { } f)
        {
            var reason = FailureReason(f.Text);
            failed = (T.PlayerFailedTitleText(Episode), Names(f.Asked) is { } names ? $"{reason} {T.PlayerStartAsked(names)}" : reason);
        }
        var lead = failed is not null || (at < DetailAt && !Offline) ? "" : Lead();
        long? seconds = at >= ToolsAt && failed is null && !Offline ? (long)at.TotalSeconds : null;
        return new StartFrame(
            StepText(at),
            shown.Phase,
            lead,
            seconds,
            Buttons(at),
            failed,
            shown.Phase == StartPhase.Buffering ? percent : null,
            Rows(at),
            found?.Translation,
            DetailsOpen);
    }

    /// <summary>The moment, once, to tell a screen reader the start is slow.</summary>
    public string? SlowAnnouncement(TimeSpan at)
    {
        if (announcedSlow || Since(at, activeSince) < SkipAfter || failure is not null || Offline)
            return null;
        announcedSlow = true;
        return T.PlayerStartStillStarting;
    }

    /// <summary>
    /// The failure's reason in plain words: the cause's first sentence, without what each source
    /// answered (the Details rows have that) and without the hint after it (the buttons give it).
    /// Built from the cause, not cut out of its text.
    /// </summary>
    public static string FailureReason(Msg text)
    {
        var full = text switch
        {
            Msg.NoticeReason { Reason: Cause.NoSourceHasEpisode c } => new Msg.NoticeReason(c with { Detail = "" }).Text(),
            Msg.NoticeReason { Reason: Cause.TitleNoMatch c } => new Msg.NoticeReason(c with { Detail = "" }).Text(),
            Msg.NoticeReason { Reason: Cause.ReleaseFailed c } => new Msg.NoticeReason(c with { Detail = "" }).Text(),
            _ => text.Text(),
        };
        return FirstSentence(full);
    }

    /// <summary>The first sentence of <paramref name="text"/>, ending in its stop: a full stop counts when a space or the end follows it, so "7.5" is not cut.</summary>
    static string FirstSentence(string text)
    {
        var end = text.Length;
        for (var i = 0; i < text.Length; i++)
        {
            var c = text[i];
            var wide = c is '。' or '！' or '？';
            var stop = c is '.' or '!' or '?' && (i + 1 == text.Length || char.IsWhiteSpace(text[i + 1]));
            if (wide || stop)
            {
                end = i + 1;
                break;
            }
        }
        var sentence = text[..end].Trim().TrimEnd(':', '：').TrimEnd();
        if (sentence.Length == 0 || sentence[^1] is not ('.' or '!' or '?' or '。' or '！' or '？'))
            sentence += ".";
        return sentence;
    }

    /// <summary>Names as a sentence lists them: "Alpha", "Alpha and Brook", "Alpha, Brook and Harbor".</summary>
    public static string? Names(IReadOnlyList<string> list) => list.Count switch
    {
        0 => null,
        1 => list[0],
        _ => T.NamesPair(list.Take(list.Count - 1).Skip(1).Aggregate(list[0], T.NamesList), list[^1]),
    };

    /// <summary>A rate as the detail line gives it, with one decimal from megabytes up: "3.4 MB".</summary>
    public static string Speed(ulong bytesPerSecond)
    {
        double value = bytesPerSecond;
        return value switch
        {
            >= 1e9 => Wording.Size(new Size(value / 1e9, SizeUnit.Gigabyte, 1)),
            >= 1e6 => Wording.Size(new Size(value / 1e6, SizeUnit.Megabyte, 1)),
            >= 1e3 => Wording.Size(new Size(Math.Round(value / 1e3), SizeUnit.Kilobyte, 0)),
            _ => Wording.Size(new Size(value, SizeUnit.Byte, 0)),
        };
    }

    static string SecondsText(TimeSpan took) => T.PlayerStartSeconds((long)Math.Round(took.TotalSeconds, MidpointRounding.AwayFromZero));

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
        SourceMiss.NoMatch => T.PlayerStartRowNoMatch,
        SourceMiss.NoEpisode => T.PlayerStartRowNoEpisode(episode),
        SourceMiss.Failed => T.PlayerStartRowFailed,
        SourceMiss.TimedOut => T.PlayerStartRowTimedOut,
        _ => T.PlayerStartRowSkipped,
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

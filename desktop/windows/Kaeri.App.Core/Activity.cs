using Kaeri.Engine.Model;

namespace Kaeri.App;

/// <summary>One job, from the <c>JobStarted</c> that names it to the <c>JobDone</c> that ends it.</summary>
public sealed record Job(ulong Id, string Activity, DateTimeOffset Started)
{
    /// <summary>What it is held up on, when that is not the work itself.</summary>
    public string? Note { get; set; }

    /// <summary>Until when, when the note's wait is known.</summary>
    public DateTimeOffset? WaitUntil { get; set; }

    /// <summary>What the title bar says for it: the note when there is one, else the activity.</summary>
    public string Words => Note ?? Activity.FirstUpper();
}

/// <summary>
/// The jobs running, as the engine names them. Work earns a place on screen only once it has
/// run 600 ms, the terminal's rule and the Mac's: most loads answer well inside it, and a status
/// that blinked on every click would move the title bar for nothing.
/// </summary>
public sealed class Jobs
{
    /// <summary>How long work runs before it shows.</summary>
    public static readonly TimeSpan Delay = TimeSpan.FromMilliseconds(600);

    readonly List<Job> running = [];

    /// <summary>Every job running, oldest first.</summary>
    public IReadOnlyList<Job> Running => running;

    /// <summary>The jobs that have run at least <see cref="Delay"/> by <paramref name="now"/>, newest first.</summary>
    public IReadOnlyList<Job> Visible(DateTimeOffset now) =>
        [.. running.Where(j => now - j.Started >= Delay).Reverse()];

    /// <summary>Folds one event in; true when it changed what runs.</summary>
    public bool Take(Event e, DateTimeOffset now)
    {
        switch (e)
        {
            case Event.JobStarted started:
                running.RemoveAll(j => j.Id == started.Job);
                running.Add(new Job(started.Job, started.Activity, now));
                return true;
            case Event.JobNote note when running.FirstOrDefault(j => j.Id == note.Job) is { } job:
                job.Note = note.Note.Text().FirstUpper();
                job.WaitUntil = note.Wait is { } wait ? now + wait : null;
                return true;
            case Event.JobDone done:
                return running.RemoveAll(j => j.Id == done.V1) > 0;
        }
        return false;
    }

    /// <summary>The engine is gone: no job will end.</summary>
    public void Clear() => running.Clear();
}

/// <summary>A notice as Messages keeps it: when it was said, its words, and whether it was an alert.</summary>
public sealed record Said(DateTimeOffset At, string Text, bool Alert);

/// <summary>
/// Every notice this run, newest first, the last 200: the TUI's recent messages and the Mac's
/// Messages, so a notice that timed out can still be read.
/// </summary>
public sealed class NoticeLog
{
    public const int Kept = 200;

    readonly List<Said> said = [];

    /// <summary>Newest first.</summary>
    public IReadOnlyList<Said> All => said;

    /// <summary>Raised with each notice kept.</summary>
    public event Action<Said>? Added;

    public void Add(string text, bool alert, DateTimeOffset at)
    {
        if (text.Length == 0)
            return;
        var item = new Said(at, text, alert);
        said.Insert(0, item);
        if (said.Count > Kept)
            said.RemoveRange(Kept, said.Count - Kept);
        Added?.Invoke(item);
    }
}

/// <summary>Where sync with the trackers stands, as the title bar's status shows it: a dot and its words.</summary>
public enum SyncDot
{
    /// <summary>No tracker signed in: a hollow haze dot.</summary>
    SignedOut,
    /// <summary>Synced: water.</summary>
    Synced,
    /// <summary>Changes waiting to go out, or a sync running: haze.</summary>
    Waiting,
    /// <summary>A pull failed or a sign-in expired: vermilion.</summary>
    Problem,
}

public sealed record SyncSummary(string Text, SyncDot Dot, string? Tracker);

public static class Summary
{
    /// <summary>
    /// The status's words, as the Mac's sync capsule has them: AniList first, the tracker the
    /// catalogue comes from, or any other signed in; then offline, expired, waiting, failed or synced.
    /// </summary>
    public static SyncSummary Of(IReadOnlyDictionary<string, SyncState> trackers, bool offline)
    {
        var signedIn = trackers.Values.Where(s => s.Connected).ToList();
        if ((signedIn.FirstOrDefault(s => s.Tracker == "anilist") ?? signedIn.OrderBy(s => s.Tracker, StringComparer.Ordinal).FirstOrDefault()) is not { } main)
            return new(T.AccountsStateSignedOut, SyncDot.SignedOut, null);
        var name = Engine.Ffi.KaeriFfiMethods.TrackerName(main.Tracker);
        var pending = signedIn.Sum(s => (long)s.Outbox);
        if (offline)
            return new(pending > 0 ? T.AccountsChangesWaiting(pending) : T.StoresStatusOffline, SyncDot.Waiting, main.Tracker);
        if (main.NeedsReauth)
            return new(T.TitlebarSignInAgain(name), SyncDot.Problem, main.Tracker);
        if (pending > 0)
            return new(T.AccountsChangesWaiting(pending), SyncDot.Waiting, main.Tracker);
        if (main.PullError is not null)
            return new(T.TitlebarNotSynced(name), SyncDot.Problem, main.Tracker);
        return new(T.TitlebarSynced(name), SyncDot.Synced, main.Tracker);
    }

    /// <summary>"4s", "1m 12s": how long a job has run.</summary>
    public static string Elapsed(TimeSpan run)
    {
        var seconds = (long)Math.Max(0, run.TotalSeconds);
        return seconds < 60 ? T.DurationSeconds(seconds) : T.DurationMinutesSeconds(seconds / 60, (seconds % 60).ToString(System.Globalization.CultureInfo.CurrentCulture));
    }

    /// <summary>The words with their first letter in upper case, as notices and jobs are shown.</summary>
    public static string FirstUpper(this string text) =>
        text.Length == 0 || char.IsUpper(text[0]) ? text : char.ToUpper(text[0], System.Globalization.CultureInfo.CurrentCulture) + text[1..];
}
